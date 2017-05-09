//! An interpreter for the rust-installer package format.  Responsible
//! for installing from a directory or tarball to an installation
//! prefix, represented by a `Components` instance.

extern crate tar;
extern crate flate2;

use component::components::*;
use component::transaction::*;

use errors::*;
use rustup_utils::utils;
use temp;

use std::path::{Path, PathBuf};
use std::collections::HashSet;
use std::fmt;
use std::io::Read;
use std::fs::File;

/// The current metadata revision used by rust-installer
pub const INSTALLER_VERSION: &'static str = "3";
pub const VERSION_FILE: &'static str = "rust-installer-version";

pub trait Package: fmt::Debug {
    fn contains(&self, component: &str, short_name: Option<&str>) -> bool;
    fn install<'a>(&self,
                   target: &Components,
                   component: &str,
                   short_name: Option<&str>,
                   tx: Transaction<'a>)
                   -> Result<Transaction<'a>>;
    fn components(&self) -> Vec<String>;
}

#[derive(Debug)]
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
        Err(ErrorKind::BadInstallerVersion(v.to_owned()).into())
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
                            .ok_or_else(|| ErrorKind::CorruptComponent(name.to_owned())));

            let path = part.1;
            let src_path = root.join(&path);

            match &*part.0 {
                "file" => try!(builder.copy_file(path.clone(), &src_path)),
                "dir" => try!(builder.copy_dir(path.clone(), &src_path)),
                _ => return Err(ErrorKind::CorruptComponent(name.to_owned()).into()),
            }

            try!(set_file_perms(&target.prefix().path().join(path), &src_path));
        }

        let tx = try!(builder.finish());

        Ok(tx)
    }

    fn components(&self) -> Vec<String> {
        self.components.iter().cloned().collect()
    }
}

// On Unix we need to set up the file permissions correctly so
// binaries are executable and directories readable. This shouldn't be
// necessary: the source files *should* have the right permissions,
// but due to rust-lang/rust#25479 they don't.
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
            let entry = try!(entry.chain_err(|| ErrorKind::ComponentDirPermissionsFailed));
            let meta = try!(entry.metadata().chain_err(|| ErrorKind::ComponentDirPermissionsFailed));
            if meta.is_dir() {
                let mut perm = meta.permissions();
                perm.set_mode(0o755);
                try!(fs::set_permissions(entry.path(), perm).chain_err(|| ErrorKind::ComponentFilePermissionsFailed));
            } else {
                let mut perm = meta.permissions();
                perm.set_mode(0o644);
                try!(fs::set_permissions(entry.path(), perm).chain_err(|| ErrorKind::ComponentFilePermissionsFailed));
            }
        }
    } else if is_bin {
        let mut perm = try!(fs::metadata(dest_path).chain_err(|| ErrorKind::ComponentFilePermissionsFailed))
                           .permissions();
        perm.set_mode(0o755);
        try!(fs::set_permissions(dest_path, perm).chain_err(|| ErrorKind::ComponentFilePermissionsFailed));
    } else {
        let mut perm = try!(fs::metadata(dest_path).chain_err(|| ErrorKind::ComponentFilePermissionsFailed))
                           .permissions();
        perm.set_mode(0o644);
        try!(fs::set_permissions(dest_path, perm).chain_err(|| ErrorKind::ComponentFilePermissionsFailed));
    }

    Ok(())
}

#[cfg(windows)]
fn set_file_perms(_dest_path: &Path, _src_path: &Path) -> Result<()> {
    Ok(())
}

#[derive(Debug)]
pub struct TarPackage<'a>(DirectoryPackage, temp::Dir<'a>);

impl<'a> TarPackage<'a> {
    pub fn new<R: Read>(stream: R, temp_cfg: &'a temp::Cfg) -> Result<Self> {
        let temp_dir = try!(temp_cfg.new_directory());
        let mut archive = tar::Archive::new(stream);
        // The rust-installer packages unpack to a directory called
        // $pkgname-$version-$target. Skip that directory when
        // unpacking.
        try!(unpack_without_first_dir(&mut archive, &*temp_dir));

        Ok(TarPackage(try!(DirectoryPackage::new(temp_dir.to_owned())), temp_dir))
    }
}

fn unpack_without_first_dir<R: Read>(archive: &mut tar::Archive<R>, path: &Path) -> Result<()> {
    let entries = try!(archive.entries().chain_err(|| ErrorKind::ExtractingPackage));
    for entry in entries {
        let mut entry = try!(entry.chain_err(|| ErrorKind::ExtractingPackage));
        let relpath = {
            let path = entry.path();
            let path = try!(path.chain_err(|| ErrorKind::ExtractingPackage));
            path.into_owned()
        };
        let mut components = relpath.components();
        // Throw away the first path component
        components.next();
        let full_path = path.join(&components.as_path());

        // Create the full path to the entry if it does not exist already
        match full_path.parent() {
            Some(parent) if !parent.exists() =>
                try!(::std::fs::create_dir_all(&parent).chain_err(|| ErrorKind::ExtractingPackage)),
            _ => (),
        };

        try!(entry.unpack(&full_path).chain_err(|| ErrorKind::ExtractingPackage));
    }

    Ok(())
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
    fn components(&self) -> Vec<String> {
        self.0.components()
    }
}

#[derive(Debug)]
pub struct TarGzPackage<'a>(TarPackage<'a>);

impl<'a> TarGzPackage<'a> {
    pub fn new<R: Read>(stream: R, temp_cfg: &'a temp::Cfg) -> Result<Self> {
        let stream = try!(flate2::read::GzDecoder::new(stream).chain_err(|| ErrorKind::ExtractingPackage));

        Ok(TarGzPackage(try!(TarPackage::new(stream, temp_cfg))))
    }
    pub fn new_file(path: &Path, temp_cfg: &'a temp::Cfg) -> Result<Self> {
        let file = try!(File::open(path).chain_err(|| ErrorKind::ExtractingPackage));
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
    fn components(&self) -> Vec<String> {
        self.0.components()
    }
}
