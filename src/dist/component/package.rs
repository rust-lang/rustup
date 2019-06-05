//! An interpreter for the rust-installer package format.  Responsible
//! for installing from a directory or tarball to an installation
//! prefix, represented by a `Components` instance.

use crate::diskio::{get_executor, Executor, Item, Kind};
use crate::dist::component::components::*;
use crate::dist::component::transaction::*;
use crate::dist::temp;
use crate::errors::*;
use crate::utils::notifications::Notification;
use crate::utils::utils;

use std::collections::HashSet;
use std::fmt;
use std::io::{self, ErrorKind as IOErrorKind, Read};
use std::path::{Path, PathBuf};

use tar::EntryType;

/// The current metadata revision used by rust-installer
pub const INSTALLER_VERSION: &str = "3";
pub const VERSION_FILE: &str = "rust-installer-version";

pub trait Package: fmt::Debug {
    fn contains(&self, component: &str, short_name: Option<&str>) -> bool;
    fn install<'a>(
        &self,
        target: &Components,
        component: &str,
        short_name: Option<&str>,
        tx: Transaction<'a>,
    ) -> Result<Transaction<'a>>;
    fn components(&self) -> Vec<String>;
}

#[derive(Debug)]
pub struct DirectoryPackage {
    path: PathBuf,
    components: HashSet<String>,
    copy: bool,
}

impl DirectoryPackage {
    pub fn new(path: PathBuf, copy: bool) -> Result<Self> {
        validate_installer_version(&path)?;

        let content = utils::read_file("package components", &path.join("components"))?;
        let components = content
            .lines()
            .map(std::borrow::ToOwned::to_owned)
            .collect();
        Ok(DirectoryPackage {
            path,
            components,
            copy,
        })
    }
}

fn validate_installer_version(path: &Path) -> Result<()> {
    let file = utils::read_file("installer version", &path.join(VERSION_FILE))?;
    let v = file.trim();
    if v == INSTALLER_VERSION {
        Ok(())
    } else {
        Err(ErrorKind::BadInstallerVersion(v.to_owned()).into())
    }
}

impl Package for DirectoryPackage {
    fn contains(&self, component: &str, short_name: Option<&str>) -> bool {
        self.components.contains(component)
            || if let Some(n) = short_name {
                self.components.contains(n)
            } else {
                false
            }
    }
    fn install<'a>(
        &self,
        target: &Components,
        name: &str,
        short_name: Option<&str>,
        tx: Transaction<'a>,
    ) -> Result<Transaction<'a>> {
        let actual_name = if self.components.contains(name) {
            name
        } else if let Some(n) = short_name {
            n
        } else {
            name
        };

        let root = self.path.join(actual_name);

        let manifest = utils::read_file("package manifest", &root.join("manifest.in"))?;
        let mut builder = target.add(name, tx);

        for l in manifest.lines() {
            let part = ComponentPart::decode(l)
                .ok_or_else(|| ErrorKind::CorruptComponent(name.to_owned()))?;

            let path = part.1;
            let src_path = root.join(&path);

            match &*part.0 {
                "file" => {
                    if self.copy {
                        builder.copy_file(path.clone(), &src_path)?
                    } else {
                        builder.move_file(path.clone(), &src_path)?
                    }
                }
                "dir" => {
                    if self.copy {
                        builder.copy_dir(path.clone(), &src_path)?
                    } else {
                        builder.move_dir(path.clone(), &src_path)?
                    }
                }
                _ => return Err(ErrorKind::CorruptComponent(name.to_owned()).into()),
            }

            set_file_perms(&target.prefix().path().join(path), &src_path)?;
        }

        let tx = builder.finish()?;

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
    use std::fs::{self, Metadata};
    use std::os::unix::fs::PermissionsExt;
    use walkdir::WalkDir;

    // Compute whether this entry needs the X bit
    fn needs_x(meta: &Metadata) -> bool {
        meta.is_dir() || // Directories need it
        meta.permissions().mode() & 0o700 == 0o700 // If it is rwx for the user, it gets the X bit
    }

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
            let entry = entry.chain_err(|| ErrorKind::ComponentDirPermissionsFailed)?;
            let meta = entry
                .metadata()
                .chain_err(|| ErrorKind::ComponentDirPermissionsFailed)?;

            let mut perm = meta.permissions();
            perm.set_mode(if needs_x(&meta) { 0o755 } else { 0o644 });
            fs::set_permissions(entry.path(), perm)
                .chain_err(|| ErrorKind::ComponentFilePermissionsFailed)?;
        }
    } else {
        let meta =
            fs::metadata(dest_path).chain_err(|| ErrorKind::ComponentFilePermissionsFailed)?;
        let mut perm = meta.permissions();
        perm.set_mode(if is_bin || needs_x(&meta) {
            0o755
        } else {
            0o644
        });
        fs::set_permissions(dest_path, perm)
            .chain_err(|| ErrorKind::ComponentFilePermissionsFailed)?;
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
    pub fn new<R: Read>(
        stream: R,
        temp_cfg: &'a temp::Cfg,
        notify_handler: Option<&'a dyn Fn(Notification<'_>)>,
    ) -> Result<Self> {
        let temp_dir = temp_cfg.new_directory()?;
        let mut archive = tar::Archive::new(stream);
        // The rust-installer packages unpack to a directory called
        // $pkgname-$version-$target. Skip that directory when
        // unpacking.
        unpack_without_first_dir(&mut archive, &*temp_dir, notify_handler)?;

        Ok(TarPackage(
            DirectoryPackage::new(temp_dir.to_owned(), false)?,
            temp_dir,
        ))
    }
}

// Handle the async result of io operations
fn filter_result(op: Item) -> io::Result<()> {
    match op.result {
        Ok(_) => Ok(()),
        Err(e) => match e.kind() {
            // TODO: the IO execution logic should pass this back rather than
            // being the code to ignore it.
            IOErrorKind::AlreadyExists => {
                if let Kind::Directory = op.kind {
                    Ok(())
                } else {
                    Err(e)
                }
            }
            _ => Err(e),
        },
    }
}

fn unpack_without_first_dir<'a, R: Read>(
    archive: &mut tar::Archive<R>,
    path: &Path,
    notify_handler: Option<&'a dyn Fn(Notification<'_>)>,
) -> Result<()> {
    let mut io_executor: Box<dyn Executor> = get_executor(notify_handler);
    let entries = archive
        .entries()
        .chain_err(|| ErrorKind::ExtractingPackage)?;
    let mut checked_parents: HashSet<PathBuf> = HashSet::new();

    for entry in entries {
        let mut entry = entry.chain_err(|| ErrorKind::ExtractingPackage)?;
        let relpath = {
            let path = entry.path();
            let path = path.chain_err(|| ErrorKind::ExtractingPackage)?;
            path.into_owned()
        };
        // Reject path components that are not normal (.|..|/| etc)
        for part in relpath.components() {
            match part {
                std::path::Component::Normal(_) => {}
                _ => return Err(ErrorKind::BadPath(relpath).into()),
            }
        }
        let mut components = relpath.components();
        // Throw away the first path component: we make our own root
        components.next();
        let full_path = path.join(&components.as_path());

        let size = entry.header().size()?;
        if size > 100_000_000 {
            return Err(format!("File too big {} {}", relpath.display(), size).into());
        }
        // Bail out if we get hard links, device nodes or any other unusual content
        // - it is most likely an attack, as rusts cross-platform nature precludes
        // such artifacts
        let kind = entry.header().entry_type();
        let mode = entry.header().mode().ok().unwrap();
        let item = match kind {
            EntryType::Directory => Item::make_dir(full_path, mode),
            EntryType::Regular => {
                let mut v = Vec::with_capacity(size as usize);
                entry.read_to_end(&mut v)?;
                Item::write_file(full_path, v, mode)
            }
            _ => return Err(ErrorKind::UnsupportedKind(format!("{:?}", kind)).into()),
        };

        // FUTURE: parallelise or delete (surely all distribution tars are well formed in this regard).
        // Create the full path to the entry if it does not exist already
        if let Some(parent) = item.full_path.parent() {
            if !checked_parents.contains(parent) {
                checked_parents.insert(parent.to_owned());
                // It would be nice to optimise this stat out, but the tar could be like so:
                // a/deep/file.txt
                // a/file.txt
                // which would require tracking the segments rather than a simple hash.
                // Until profile shows that one stat per dir is a problem (vs one stat per file)
                // leave till later.

                if !parent.exists() {
                    let path_display = format!("{}", parent.display());
                    trace_scoped!("create_dir_all", "name": path_display);
                    std::fs::create_dir_all(&parent).chain_err(|| ErrorKind::ExtractingPackage)?
                }
            }
        }

        for item in io_executor.execute(item) {
            // TODO capture metrics, add directories to created cache
            filter_result(item).chain_err(|| ErrorKind::ExtractingPackage)?;
        }

        // drain completed results to keep memory pressure low
        if let Some(iter) = io_executor.completed() {
            for prev_item in iter {
                // TODO capture metrics, add directories to created cache
                filter_result(prev_item).chain_err(|| ErrorKind::ExtractingPackage)?;
            }
        }
    }

    if let Some(iter) = io_executor.join() {
        for item in iter {
            // handle final IOs
            // TODO capture metrics, add directories to created cache
            filter_result(item).chain_err(|| ErrorKind::ExtractingPackage)?;
        }
    }

    Ok(())
}

impl<'a> Package for TarPackage<'a> {
    fn contains(&self, component: &str, short_name: Option<&str>) -> bool {
        self.0.contains(component, short_name)
    }
    fn install<'b>(
        &self,
        target: &Components,
        component: &str,
        short_name: Option<&str>,
        tx: Transaction<'b>,
    ) -> Result<Transaction<'b>> {
        self.0.install(target, component, short_name, tx)
    }
    fn components(&self) -> Vec<String> {
        self.0.components()
    }
}

#[derive(Debug)]
pub struct TarGzPackage<'a>(TarPackage<'a>);

impl<'a> TarGzPackage<'a> {
    pub fn new<R: Read>(
        stream: R,
        temp_cfg: &'a temp::Cfg,
        notify_handler: Option<&'a dyn Fn(Notification<'_>)>,
    ) -> Result<Self> {
        let stream = flate2::read::GzDecoder::new(stream);
        Ok(TarGzPackage(TarPackage::new(
            stream,
            temp_cfg,
            notify_handler,
        )?))
    }
}

impl<'a> Package for TarGzPackage<'a> {
    fn contains(&self, component: &str, short_name: Option<&str>) -> bool {
        self.0.contains(component, short_name)
    }
    fn install<'b>(
        &self,
        target: &Components,
        component: &str,
        short_name: Option<&str>,
        tx: Transaction<'b>,
    ) -> Result<Transaction<'b>> {
        self.0.install(target, component, short_name, tx)
    }
    fn components(&self) -> Vec<String> {
        self.0.components()
    }
}

#[derive(Debug)]
pub struct TarXzPackage<'a>(TarPackage<'a>);

impl<'a> TarXzPackage<'a> {
    pub fn new<R: Read>(
        stream: R,
        temp_cfg: &'a temp::Cfg,
        notify_handler: Option<&'a dyn Fn(Notification<'_>)>,
    ) -> Result<Self> {
        let stream = xz2::read::XzDecoder::new(stream);
        Ok(TarXzPackage(TarPackage::new(
            stream,
            temp_cfg,
            notify_handler,
        )?))
    }
}

impl<'a> Package for TarXzPackage<'a> {
    fn contains(&self, component: &str, short_name: Option<&str>) -> bool {
        self.0.contains(component, short_name)
    }
    fn install<'b>(
        &self,
        target: &Components,
        component: &str,
        short_name: Option<&str>,
        tx: Transaction<'b>,
    ) -> Result<Transaction<'b>> {
        self.0.install(target, component, short_name, tx)
    }
    fn components(&self) -> Vec<String> {
        self.0.components()
    }
}
