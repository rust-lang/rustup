/// The representation of the installed toolchain and its components.
/// `Components` and `DirectoryPackage` are the two sides of the
/// installation / uninstallation process.

use rustup_utils::utils;
use prefix::InstallPrefix;
use errors::*;

use component::transaction::Transaction;
use component::package::{INSTALLER_VERSION, VERSION_FILE};

use std::path::{Path, PathBuf};
use std::fs::File;

const COMPONENTS_FILE: &'static str = "components";

#[derive(Clone, Debug)]
pub struct Components {
    prefix: InstallPrefix,
}

impl Components {
    pub fn open(prefix: InstallPrefix) -> Result<Self> {
        let c = Components { prefix: prefix };

        // Validate that the metadata uses a format we know
        if let Some(v) = try!(c.read_version()) {
            if v != INSTALLER_VERSION {
                return Err(ErrorKind::BadInstalledMetadataVersion(v).into());
            }
        }

        Ok(c)
    }
    fn rel_components_file(&self) -> PathBuf {
        self.prefix.rel_manifest_file(COMPONENTS_FILE)
    }
    fn rel_component_manifest(&self, name: &str) -> PathBuf {
        self.prefix.rel_manifest_file(&format!("manifest-{}", name))
    }
    fn read_version(&self) -> Result<Option<String>> {
        let p = self.prefix.manifest_file(VERSION_FILE);
        if utils::is_file(&p) {
            Ok(Some(try!(utils::read_file(VERSION_FILE, &p)).trim().to_string()))
        } else {
            Ok(None)
        }
    }
    fn write_version(&self, tx: &mut Transaction) -> Result<()> {
        try!(tx.modify_file(self.prefix.rel_manifest_file(VERSION_FILE)));
        try!(utils::write_file(VERSION_FILE,
                               &self.prefix.manifest_file(VERSION_FILE),
                               INSTALLER_VERSION));

        Ok(())
    }
    pub fn list(&self) -> Result<Vec<Component>> {
        let path = self.prefix.abs_path(self.rel_components_file());
        if !utils::is_file(&path) {
            return Ok(Vec::new());
        }
        let content = try!(utils::read_file("components", &path));
        Ok(content.lines()
                  .map(|s| {
                      Component {
                          components: self.clone(),
                          name: s.to_owned(),
                      }
                  })
                  .collect())
    }
    pub fn add<'a>(&self, name: &str, tx: Transaction<'a>) -> ComponentBuilder<'a> {
        ComponentBuilder {
            components: self.clone(),
            name: name.to_owned(),
            parts: Vec::new(),
            tx: tx,
        }
    }
    pub fn find(&self, name: &str) -> Result<Option<Component>> {
        let result = try!(self.list());
        Ok(result.into_iter().filter(|c| (c.name() == name)).next())
    }
    pub fn prefix(&self) -> InstallPrefix {
        self.prefix.clone()
    }
}

pub struct ComponentBuilder<'a> {
    components: Components,
    name: String,
    parts: Vec<ComponentPart>,
    tx: Transaction<'a>
}

impl<'a> ComponentBuilder<'a> {
    pub fn add_file(&mut self, path: PathBuf) -> Result<File> {
        self.parts.push(ComponentPart("file".to_owned(), path.clone()));
        self.tx.add_file(&self.name, path)
    }
    pub fn copy_file(&mut self, path: PathBuf, src: &Path) -> Result<()> {
        self.parts.push(ComponentPart("file".to_owned(), path.clone()));
        self.tx.copy_file(&self.name, path, src)
    }
    pub fn copy_dir(&mut self, path: PathBuf, src: &Path) -> Result<()> {
        self.parts.push(ComponentPart("dir".to_owned(), path.clone()));
        self.tx.copy_dir(&self.name, path, src)
    }

    pub fn finish(mut self) -> Result<Transaction<'a>> {
        // Write component manifest
        let path = self.components.rel_component_manifest(&self.name);
        let abs_path = self.components.prefix.abs_path(&path);
        let mut file = try!(self.tx.add_file(&self.name, path));
        for part in self.parts {
            // FIXME: This writes relative paths to the component manifest,
            // but rust-installer writes absolute paths.
            try!(utils::write_line("component", &mut file, &abs_path, &part.encode()));
        }

        // Add component to components file
        let path = self.components.rel_components_file();
        let abs_path = self.components.prefix.abs_path(&path);
        try!(self.tx.modify_file(path));
        try!(utils::append_file("components", &abs_path, &self.name));

        // Drop in the version file for future use
        try!(self.components.write_version(&mut self.tx));

        Ok(self.tx)
    }
}

#[derive(Debug)]
pub struct ComponentPart(pub String, pub PathBuf);

impl ComponentPart {
    pub fn encode(&self) -> String {
        format!("{}:{}", &self.0, &self.1.to_string_lossy())
    }
    pub fn decode(line: &str) -> Option<Self> {
        line.find(":")
            .map(|pos| ComponentPart(line[0..pos].to_owned(),
                                     PathBuf::from(&line[(pos + 1)..])))
    }
}

#[derive(Clone, Debug)]
pub struct Component {
    components: Components,
    name: String,
}

impl Component {
    pub fn manifest_name(&self) -> String {
        format!("manifest-{}", &self.name)
    }
    pub fn manifest_file(&self) -> PathBuf {
        self.components.prefix.manifest_file(&self.manifest_name())
    }
    pub fn rel_manifest_file(&self) -> PathBuf {
        self.components.prefix.rel_manifest_file(&self.manifest_name())
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn parts(&self) -> Result<Vec<ComponentPart>> {
        let mut result = Vec::new();
        for line in try!(utils::read_file("component", &self.manifest_file())).lines() {
            result.push(try!(ComponentPart::decode(line)
                                 .ok_or_else(|| ErrorKind::CorruptComponent(self.name.clone()))));
        }
        Ok(result)
    }
    pub fn uninstall<'a>(&self, mut tx: Transaction<'a>) -> Result<Transaction<'a>> {
        // Update components file
        let path = self.components.rel_components_file();
        let abs_path = self.components.prefix.abs_path(&path);
        let temp = try!(tx.temp().new_file());
        try!(utils::filter_file("components", &abs_path, &temp, |l| (l != self.name)));
        try!(tx.modify_file(path));
        try!(utils::rename_file("components", &temp, &abs_path));

        // TODO: If this is the last component remove the components file
        // and the version file.

        // Track visited directories
        use std::collections::HashSet;
        use std::collections::hash_set::IntoIter;
        use std::fs::read_dir;

        // dirs will contain the set of longest disjoint directory paths seen
        // ancestors help in filtering seen paths and constructing dirs
        // All seen paths must be relative to avoid surprises
        struct PruneSet {
            dirs: HashSet<PathBuf>,
            ancestors: HashSet<PathBuf>,
            prefix: PathBuf,
        }

        impl PruneSet {
            fn seen(&mut self, mut path: PathBuf) {
                if !path.is_relative() || !path.pop() {
                    return;
                }
                if self.dirs.contains(&path) || self.ancestors.contains(&path) {
                    return;
                }
                self.dirs.insert(path.clone());
                while path.pop() {
                    if path.file_name().is_none() {
                        break;
                    }
                    if self.dirs.contains(&path) {
                        self.dirs.remove(&path);
                    }
                    if self.ancestors.contains(&path) {
                        break;
                    }
                    self.ancestors.insert(path.clone());
                }
            }
        }

        struct PruneIter {
            iter: IntoIter<PathBuf>,
            path_buf: Option<PathBuf>,
            prefix: PathBuf,
        }

        impl IntoIterator for PruneSet {
            type Item = PathBuf;
            type IntoIter = PruneIter;

            fn into_iter(self) -> Self::IntoIter {
                PruneIter {
                    iter: self.dirs.into_iter(),
                    path_buf: None,
                    prefix: self.prefix,
                }
            }
        }

        // Returns only empty directories
        impl Iterator for PruneIter {
            type Item = PathBuf;

            fn next(&mut self) -> Option<Self::Item> {
                self.path_buf = match self.path_buf {
                    None => self.iter.next(),
                    Some(_) => {
                        let mut path_buf = self.path_buf.take().unwrap();
                        match path_buf.file_name() {
                            Some(_) => if path_buf.pop() { Some(path_buf) } else { None },
                            None => self.iter.next(),
                        }
                    },
                };
                if self.path_buf.is_none() {
                    return None;
                }
                let full_path = self.prefix.join(self.path_buf.as_ref().unwrap());
                let empty = match read_dir(full_path) {
                    Ok(dir) => dir.count() == 0,
                    Err(_) => false,
                };
                if empty {
                    self.path_buf.clone()
                } else {
                    // No dir above can be empty, go to next path in dirs
                    self.path_buf = None;
                    self.next()
                }
            }
        }

        // Remove parts
        let mut pset = PruneSet {
            dirs: HashSet::new(),
            ancestors: HashSet::new(),
            prefix: self.components.prefix.abs_path(""),
        };
        for part in try!(self.parts()).into_iter().rev() {
            match &*part.0 {
                "file" => try!(tx.remove_file(&self.name, part.1.clone())),
                "dir" => try!(tx.remove_dir(&self.name, part.1.clone())),
                _ => return Err(ErrorKind::CorruptComponent(self.name.clone()).into()),
            }
            pset.seen(part.1);
        }
        for empty_dir in pset {
            try!(tx.remove_dir(&self.name, empty_dir));
        }

        // Remove component manifest
        try!(tx.remove_file(&self.name, self.rel_manifest_file()));

        Ok(tx)
    }
}
