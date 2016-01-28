//! An interpreter for the rust-installer package format.  Responsible
//! for installing from a directory or tarball to an installation
//! prefix, represented by a `Components` instance.

extern crate tar;
extern crate flate2;

use component::components::*;
use component::transaction::*;

use errors::*;
use utils;
use temp;

use std::path::{Path, PathBuf};
use std::collections::HashSet;
use std::io::Read;
use std::fs::File;

/// The current metadata revision used by rust-installer
pub const INSTALLER_VERSION: &'static str = "3";
pub const VERSION_FILE: &'static str = "rust-installer-version";

pub trait Package {
    fn contains(&self, component: &str, short_name: Option<&str>) -> bool;
    fn install<'a>(&self,
                   target: &Components,
                   component: &str,
                   short_name: Option<&str>,
                   tx: Transaction<'a>)
                   -> Result<Transaction<'a>>;
}

pub struct DirectoryPackage {
    path: PathBuf,
    components: HashSet<String>,
}

impl DirectoryPackage {
    pub fn new(path: PathBuf) -> Result<Self> {
        try!(validate_installer_version(&path));

        let content = try!(utils::read_file("package components", &path.join("components")));
        let components = content.lines().map(|l| l.to_owned()).collect();
        Ok(DirectoryPackage {
            path: path,
            components: components,
        })
    }
}

fn validate_installer_version(path: &Path) -> Result<()> {
    let file = try!(utils::read_file("installer version", &path.join(VERSION_FILE)));
    let v = file.trim();
    if v == INSTALLER_VERSION {
        Ok(())
    } else {
        Err(Error::InstallerVersion(v.to_owned()))
    }
}

impl Package for DirectoryPackage {
    fn contains(&self, component: &str, short_name: Option<&str>) -> bool {
        self.components.contains(component) ||
        if let Some(n) = short_name {
            self.components.contains(n)
        } else {
            false
        }
    }
    fn install<'a>(&self,
                   target: &Components,
                   name: &str,
                   short_name: Option<&str>,
                   tx: Transaction<'a>)
                   -> Result<Transaction<'a>> {
        let actual_name = if self.components.contains(name) {
            name
        } else if let Some(n) = short_name {
            n
        } else {
            name
        };

        let root = self.path.join(actual_name);

        let manifest = try!(utils::read_file("package manifest", &root.join("manifest.in")));
        let mut builder = target.add(name, tx);

        for l in manifest.lines() {
            let part = try!(ComponentPart::decode(l)
                                .ok_or_else(|| Error::CorruptComponent(name.to_owned())));

            let path = part.1;
            let src_path = root.join(&path);

            match &*part.0 {
                "file" => try!(builder.copy_file(path.clone(), &src_path)),
                "dir" => try!(builder.copy_dir(path.clone(), &src_path)),
                _ => return Err(Error::CorruptComponent(name.to_owned())),
            }

            try!(set_file_perms(&target.prefix().path().join(path), &src_path));
        }

        let (_, tx) = try!(builder.finish());

        Ok(tx)
    }
}

// On Unix we need to set up the file permissions correctly so
// binaries are executable and directories readable.
#[cfg(unix)]
fn set_file_perms(dest_path: &Path, src_path: &Path) -> Result<()> {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use walkdir::WalkDir;

    // By convention, anything in the bin/ directory of the package is a binary
    let is_bin = if let Some(p) = src_path.parent() {
        p.ends_with("bin")
    } else {
        false
    };

    let is_dir = utils::is_directory(dest_path);

    if is_dir {
        // Walk the directory setting everything
        for entry in WalkDir::new(dest_path) {
            let entry = try!(entry.map_err(|e| Error::WalkDirForPermissions(e)));
            let meta = try!(entry.metadata().map_err(|e| Error::WalkDirForPermissions(e)));
            if meta.is_dir() {
                let mut perm = meta.permissions();
                perm.set_mode(0o755);
                try!(fs::set_permissions(entry.path(), perm).map_err(|e| Error::SetPermissions(e)));
            } else {
                let mut perm = meta.permissions();
                perm.set_mode(0o644);
                try!(fs::set_permissions(entry.path(), perm).map_err(|e| Error::SetPermissions(e)));
            }
        }
    } else if is_bin {
        let mut perm = try!(fs::metadata(dest_path).map_err(|e| Error::SetPermissions(e)))
                           .permissions();
        perm.set_mode(0o755);
        try!(fs::set_permissions(dest_path, perm).map_err(|e| Error::SetPermissions(e)));
    } else {
        let mut perm = try!(fs::metadata(dest_path).map_err(|e| Error::SetPermissions(e)))
                           .permissions();
        perm.set_mode(0o644);
        try!(fs::set_permissions(dest_path, perm).map_err(|e| Error::SetPermissions(e)));
    }

    Ok(())
}

#[cfg(windows)]
fn set_file_perms(_dest_path: &Path, _src_path: &Path) -> Result<()> {
    Ok(())
}

pub struct TarPackage<'a>(DirectoryPackage, temp::Dir<'a>);

impl<'a> TarPackage<'a> {
    pub fn new<R: Read>(stream: R, temp_cfg: &'a temp::Cfg) -> Result<Self> {
        let temp_dir = try!(temp_cfg.new_directory());

        let mut archive = tar::Archive::new(stream);
        try!(archive.unpack(&*temp_dir).map_err(Error::ExtractingPackage));

        Ok(TarPackage(try!(DirectoryPackage::new(temp_dir.to_owned())), temp_dir))
    }
}

impl<'a> Package for TarPackage<'a> {
    fn contains(&self, component: &str, short_name: Option<&str>) -> bool {
        self.0.contains(component, short_name)
    }
    fn install<'b>(&self,
                   target: &Components,
                   component: &str,
                   short_name: Option<&str>,
                   tx: Transaction<'b>)
                   -> Result<Transaction<'b>> {
        self.0.install(target, component, short_name, tx)
    }
}

pub struct TarGzPackage<'a>(TarPackage<'a>);

impl<'a> TarGzPackage<'a> {
    pub fn new<R: Read>(stream: R, temp_cfg: &'a temp::Cfg) -> Result<Self> {
        let stream = try!(flate2::read::GzDecoder::new(stream).map_err(Error::ExtractingPackage));

        Ok(TarGzPackage(try!(TarPackage::new(stream, temp_cfg))))
    }
    pub fn new_file(path: &Path, temp_cfg: &'a temp::Cfg) -> Result<Self> {
        let file = try!(File::open(path).map_err(Error::ExtractingPackage));
        Self::new(file, temp_cfg)
    }
}

impl<'a> Package for TarGzPackage<'a> {
    fn contains(&self, component: &str, short_name: Option<&str>) -> bool {
        self.0.contains(component, short_name)
    }
    fn install<'b>(&self,
                   target: &Components,
                   component: &str,
                   short_name: Option<&str>,
                   tx: Transaction<'b>)
                   -> Result<Transaction<'b>> {
        self.0.install(target, component, short_name, tx)
    }
}
