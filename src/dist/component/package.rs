//! An interpreter for the rust-installer package format.  Responsible
//! for installing from a directory or tarball to an installation
//! prefix, represented by a `Components` instance.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::io::{self, ErrorKind as IOErrorKind, Read};
use std::iter::FromIterator;
use std::mem;
use std::path::{Path, PathBuf};

use tar::EntryType;

use crate::diskio::{get_executor, Executor, Item, Kind};
use crate::dist::component::components::*;
use crate::dist::component::transaction::*;
use crate::dist::temp;
use crate::errors::*;
use crate::process;
use crate::utils::notifications::Notification;
use crate::utils::utils;

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
        Ok(Self {
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
        }

        let tx = builder.finish()?;

        Ok(tx)
    }

    fn components(&self) -> Vec<String> {
        self.components.iter().cloned().collect()
    }
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

struct MemoryBudget {
    limit: usize,
    used: usize,
}

// Probably this should live in diskio but ¯\_(ツ)_/¯
impl MemoryBudget {
    fn new(
        max_file_size: usize,
        effective_max_ram: Option<usize>,
        notify_handler: Option<&dyn Fn(Notification<'_>)>,
    ) -> Self {
        const DEFAULT_UNPACK_RAM_MAX: usize = 500 * 1024 * 1024;
        const RAM_ALLOWANCE_FOR_RUSTUP_AND_BUFFERS: usize = 100 * 1024 * 1024;
        let default_max_unpack_ram = if let Some(effective_max_ram) = effective_max_ram {
            let ram_for_unpacking = effective_max_ram - RAM_ALLOWANCE_FOR_RUSTUP_AND_BUFFERS;
            std::cmp::min(DEFAULT_UNPACK_RAM_MAX, ram_for_unpacking)
        } else {
            // Rustup does not know how much RAM the machine has: use the
            // minimum known to work reliably.
            DEFAULT_UNPACK_RAM_MAX
        };
        let unpack_ram = match process()
            .var("RUSTUP_UNPACK_RAM")
            .ok()
            .and_then(|budget_str| budget_str.parse::<usize>().ok())
        {
            // Note: In future we may want to add a warning or even an override if a user
            // supplied budget is larger than effective_max_ram.
            Some(budget) => budget,
            None => {
                if let Some(h) = notify_handler {
                    h(Notification::SetDefaultBufferSize(default_max_unpack_ram))
                }
                default_max_unpack_ram
            }
        };

        if max_file_size > unpack_ram {
            panic!("RUSTUP_UNPACK_RAM must be larger than {}", max_file_size);
        }
        Self {
            limit: unpack_ram,
            used: 0,
        }
    }
    fn reclaim(&mut self, op: &Item) {
        match &op.kind {
            Kind::Directory => {}
            Kind::File(content) => self.used -= content.len(),
        };
    }

    fn claim(&mut self, op: &Item) {
        match &op.kind {
            Kind::Directory => {}
            Kind::File(content) => self.used += content.len(),
        };
    }

    fn available(&self) -> usize {
        self.limit - self.used
    }
}

/// Handle the async result of io operations
/// Replaces op.result with Ok(())
fn filter_result(op: &mut Item) -> io::Result<()> {
    let result = mem::replace(&mut op.result, Ok(()));
    match result {
        Ok(_) => Ok(()),
        Err(e) => match e.kind() {
            IOErrorKind::AlreadyExists => {
                // mkdir of e.g. ~/.rustup already existing is just fine;
                // for others it would be better to know whether it is
                // expected to exist or not -so put a flag in the state.
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

/// Dequeue the children of directories queued up waiting for the directory to
/// be created.
///
/// Currently the volume of queued items does not count as backpressure against
/// the main tar extraction process.
fn trigger_children(
    io_executor: &mut dyn Executor,
    directories: &mut HashMap<PathBuf, DirStatus>,
    budget: &mut MemoryBudget,
    item: Item,
) -> Result<usize> {
    let mut result = 0;
    if let Kind::Directory = item.kind {
        let mut pending = Vec::new();
        directories
            .entry(item.full_path)
            .and_modify(|status| match status {
                DirStatus::Exists => unreachable!(),
                DirStatus::Pending(pending_inner) => {
                    pending.append(pending_inner);
                    *status = DirStatus::Exists;
                }
            })
            .or_insert_with(|| unreachable!());
        result += pending.len();
        for pending_item in pending.into_iter() {
            for mut item in Vec::from_iter(io_executor.execute(pending_item)) {
                // TODO capture metrics
                budget.reclaim(&item);
                filter_result(&mut item).chain_err(|| ErrorKind::ExtractingPackage)?;
                result += trigger_children(io_executor, directories, budget, item)?;
            }
        }
    };
    Ok(result)
}

/// What is the status of this directory ?
enum DirStatus {
    Exists,
    Pending(Vec<Item>),
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
    const MAX_FILE_SIZE: u64 = 220_000_000;
    let effective_max_ram = match effective_limits::memory_limit() {
        Ok(ram) => Some(ram as usize),
        Err(e) => {
            if let Some(h) = notify_handler {
                h(Notification::Error(e.to_string()))
            }
            None
        }
    };
    let mut budget = MemoryBudget::new(MAX_FILE_SIZE as usize, effective_max_ram, notify_handler);

    let mut directories: HashMap<PathBuf, DirStatus> = HashMap::new();
    // Path is presumed to exist. Call it a precondition.
    directories.insert(path.to_owned(), DirStatus::Exists);

    'entries: for entry in entries {
        // drain completed results to keep memory pressure low and respond
        // rapidly to completed events even if we couldn't submit work (because
        // our unpacked item is pending dequeue)
        for mut item in Vec::from_iter(io_executor.completed()) {
            // TODO capture metrics
            budget.reclaim(&item);
            filter_result(&mut item).chain_err(|| ErrorKind::ExtractingPackage)?;
            trigger_children(&mut *io_executor, &mut directories, &mut budget, item)?;
        }

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
        // Throw away the first path component: our root was supplied.
        components.next();
        let full_path = path.join(&components.as_path());
        if full_path == path {
            // The tmp dir code makes the root dir for us.
            continue;
        }

        let size = entry.header().size()?;
        if size > MAX_FILE_SIZE {
            return Err(format!("File too big {} {}", relpath.display(), size).into());
        }
        while size > budget.available() as u64 {
            for mut item in Vec::from_iter(io_executor.completed()) {
                // TODO capture metrics
                budget.reclaim(&item);
                filter_result(&mut item).chain_err(|| ErrorKind::ExtractingPackage)?;
                trigger_children(&mut *io_executor, &mut directories, &mut budget, item)?;
            }
        }
        // Bail out if we get hard links, device nodes or any other unusual content
        // - it is most likely an attack, as rusts cross-platform nature precludes
        // such artifacts
        let kind = entry.header().entry_type();
        // https://github.com/rust-lang/rustup/issues/1140 and before that
        // https://github.com/rust-lang/rust/issues/25479
        // tl;dr: code got convoluted and we *may* have damaged tarballs out
        // there.
        // However the mandate we have is very simple: unpack as the current
        // user with modes matching the tar contents. No documented tars with
        // bad modes are in the bug tracker : the previous permission splatting
        // code was inherited from interactions with sudo that are best
        // addressed outside of rustup (run with an appropriate effective uid).
        // THAT SAID: If regressions turn up immediately post release this code -
        // https://play.rust-lang.org/?version=stable&mode=debug&edition=2018&gist=a8549057f0827bf3a068d8917256765a
        // is a translation of the prior helper function into an in-iterator
        // application.
        let tar_mode = entry.header().mode().ok().unwrap();
        // That said, the tarballs that are shipped way back have single-user
        // permissions:
        // -rwx------ rustbuild/rustbuild  ..... release/test-release.sh
        // so we should normalise the mode to match the previous behaviour users
        // may be expecting where the above file would end up with mode 0o755
        let u_mode = tar_mode & 0o700;
        let g_mode = (u_mode & 0o0500) >> 3;
        let o_mode = g_mode >> 3;
        let mode = u_mode | g_mode | o_mode;

        let mut item = match kind {
            EntryType::Directory => {
                directories.insert(full_path.to_owned(), DirStatus::Pending(Vec::new()));
                Item::make_dir(full_path, mode)
            }
            EntryType::Regular => {
                let mut v = Vec::with_capacity(size as usize);
                entry.read_to_end(&mut v)?;
                Item::write_file(full_path, v, mode)
            }
            _ => return Err(ErrorKind::UnsupportedKind(format!("{:?}", kind)).into()),
        };
        budget.claim(&item);

        let item = loop {
            // Create the full path to the entry if it does not exist already
            if let Some(parent) = item.full_path.to_owned().parent() {
                match directories.get_mut(parent) {
                    None => {
                        // Tar has item before containing directory
                        // Complain about this so we can see if these exist.
                        writeln!(
                            process().stderr(),
                            "Unexpected: missing parent '{}' for '{}'",
                            parent.display(),
                            entry.path()?.display()
                        )?;
                        directories.insert(parent.to_owned(), DirStatus::Pending(vec![item]));
                        item = Item::make_dir(parent.to_owned(), 0o755);
                        // Check the parent's parent
                        continue;
                    }
                    Some(DirStatus::Exists) => {
                        break item;
                    }
                    Some(DirStatus::Pending(pending)) => {
                        // Parent dir is being made, take next item from tar
                        pending.push(item);
                        continue 'entries;
                    }
                }
            } else {
                // We should never see a path with no parent.
                panic!();
            }
        };

        for mut item in Vec::from_iter(io_executor.execute(item)) {
            // TODO capture metrics
            budget.reclaim(&item);
            filter_result(&mut item).chain_err(|| ErrorKind::ExtractingPackage)?;
            trigger_children(&mut *io_executor, &mut directories, &mut budget, item)?;
        }
    }

    loop {
        let mut triggered = 0;
        for mut item in Vec::from_iter(io_executor.join()) {
            // handle final IOs
            // TODO capture metrics
            budget.reclaim(&item);
            filter_result(&mut item).chain_err(|| ErrorKind::ExtractingPackage)?;
            triggered += trigger_children(&mut *io_executor, &mut directories, &mut budget, item)?;
        }
        if triggered == 0 {
            // None of the IO submitted before the prior join triggered any new
            // submissions
            break;
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
