//! A transactional interface to file system operations needed by the
//! installer.
//!
//! Installation or uninstallation of a single component is done
//! within a Transaction, which supports a few simple file system
//! operations. If the Transaction is dropped without committing then
//! it will *attempt* to roll back the transaction.
//!
//! FIXME: This uses ensure_dir_exists in some places but rollback
//! does not remove any dirs created by it.

use rustup_utils::{self, utils};
use temp;
use prefix::InstallPrefix;
use errors::*;

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

/// A Transaction tracks changes to the file system, allowing them to
/// be rolled back in case of an error. Instead of deleting or
/// overwriting file, the old copies are moved to a temporary
/// folder. If the transaction is rolled back, they will be moved back
/// into place. If the transaction is committed, these files are
/// automatically cleaned up using the temp system.
///
/// All operations that create files will automatically create any
/// intermediate directories in the path to the file if they do not
/// already exist.
///
/// All operations that create files will fail if the destination
/// already exists.
#[derive(Debug)]
pub struct Transaction<'a> {
    prefix: InstallPrefix,
    changes: Vec<ChangedItem<'a>>,
    temp_cfg: &'a temp::Cfg,
    notify_handler: NotifyHandler<'a>,
    committed: bool,
}

impl<'a> Transaction<'a> {
    pub fn new(prefix: InstallPrefix,
               temp_cfg: &'a temp::Cfg,
               notify_handler: NotifyHandler<'a>)
               -> Self {
        Transaction {
            prefix: prefix,
            changes: Vec::new(),
            temp_cfg: temp_cfg,
            notify_handler: notify_handler,
            committed: false,
        }
    }

    /// Commit must be called for all successful transactions. If not
    /// called the transaction will be rolled back on drop.
    pub fn commit(mut self) {
        self.committed = true;
    }

    fn change(&mut self, item: ChangedItem<'a>) {
        self.changes.push(item);
    }

    /// Add a file at a relative path to the install prefix. Returns a
    /// `File` that may be used to subsequently write the
    /// contents.
    pub fn add_file(&mut self, component: &str, relpath: PathBuf) -> Result<File> {
        assert!(relpath.is_relative());
        let (item, file) = try!(ChangedItem::add_file(&self.prefix, component, relpath));
        self.change(item);
        Ok(file)
    }

    /// Copy a file to a relative path of the install prefix.
    pub fn copy_file(&mut self, component: &str, relpath: PathBuf, src: &Path) -> Result<()> {
        assert!(relpath.is_relative());
        let item = try!(ChangedItem::copy_file(&self.prefix, component, relpath, src));
        self.change(item);
        Ok(())
    }

    /// Recursively copy a directory to a relative path of the install prefix.
    pub fn copy_dir(&mut self, component: &str, relpath: PathBuf, src: &Path) -> Result<()> {
        assert!(relpath.is_relative());
        let item = try!(ChangedItem::copy_dir(&self.prefix, component, relpath, src));
        self.change(item);
        Ok(())
    }

    /// Remove a file from a relative path to the install prefix.
    pub fn remove_file(&mut self, component: &str, relpath: PathBuf) -> Result<()> {
        assert!(relpath.is_relative());
        let item = try!(ChangedItem::remove_file(&self.prefix, component, relpath, &self.temp_cfg));
        self.change(item);
        Ok(())
    }

    /// Recursively remove a directory from a relative path of the
    /// install prefix.
    pub fn remove_dir(&mut self, component: &str, relpath: PathBuf) -> Result<()> {
        assert!(relpath.is_relative());
        let item = try!(ChangedItem::remove_dir(&self.prefix, component, relpath, &self.temp_cfg));
        self.change(item);
        Ok(())
    }

    /// Create a new file with string contents at a relative path to
    /// the install prefix.
    pub fn write_file(&mut self, component: &str, relpath: PathBuf, content: String) -> Result<()> {
        assert!(relpath.is_relative());
        let (item, mut file) = try!(ChangedItem::add_file(&self.prefix, component, relpath.clone()));
        self.change(item);
        try!(write!(file, "{}", content).map_err(|e| {
            rustup_utils::Error::WritingFile {
                name: "component",
                path: self.prefix.abs_path(&relpath),
                error: e,
            }
        }));
        Ok(())
    }

    /// If the file exists back it up for rollback, otherwise ensure that the path
    /// to it exists so that subsequent calls to `File::create` will succeed.
    ///
    /// This is used for arbitrarily manipulating a file.
    pub fn modify_file(&mut self, relpath: PathBuf) -> Result<()> {
        assert!(relpath.is_relative());
        let item = try!(ChangedItem::modify_file(&self.prefix, relpath, &self.temp_cfg));
        self.change(item);
        Ok(())
    }

    pub fn temp(&self) -> &'a temp::Cfg {
        self.temp_cfg
    }
    pub fn notify_handler(&self) -> NotifyHandler<'a> {
        self.notify_handler
    }
}

/// If a Transaction is dropped without being committed, the changes
/// are automatically rolled back.
impl<'a> Drop for Transaction<'a> {
    fn drop(&mut self) {
        if !self.committed {
            self.notify_handler.call(Notification::RollingBack);
            for item in self.changes.iter().rev() {
                ok_ntfy!(self.notify_handler,
                         Notification::NonFatalError,
                         item.roll_back(&self.prefix));
            }
        }
    }
}

/// This is the set of fundamental operations supported on a
/// Transaction. More complicated operations, such as installing a
/// package, or updating a component, distill down into a series of
/// these primitives.
#[derive(Debug)]
enum ChangedItem<'a> {
    AddedFile(PathBuf),
    AddedDir(PathBuf),
    RemovedFile(PathBuf, temp::File<'a>),
    RemovedDir(PathBuf, temp::Dir<'a>),
    ModifiedFile(PathBuf, Option<temp::File<'a>>),
}

impl<'a> ChangedItem<'a> {
    fn roll_back(&self, prefix: &InstallPrefix) -> Result<()> {
        use self::ChangedItem::*;
        match *self {
            AddedFile(ref path) => try!(utils::remove_file("component", &prefix.abs_path(path))),
            AddedDir(ref path) => {
                try!(utils::remove_dir("component",
                                       &prefix.abs_path(path),
                                       rustup_utils::NotifyHandler::none()))
            }
            RemovedFile(ref path, ref tmp) | ModifiedFile(ref path, Some(ref tmp)) => {
                try!(utils::rename_file("component", &tmp, &prefix.abs_path(path)))
            }
            RemovedDir(ref path, ref tmp) => {
                try!(utils::rename_dir("component", &tmp.join("bk"), &prefix.abs_path(path)))
            }
            ModifiedFile(ref path, None) => {
                let abs_path = prefix.abs_path(path);
                if utils::is_file(&abs_path) {
                    try!(utils::remove_file("component", &abs_path));
                }
            }
        }
        Ok(())
    }
    fn add_file(prefix: &InstallPrefix, component: &str, relpath: PathBuf) -> Result<(Self, File)> {
        let abs_path = prefix.abs_path(&relpath);
        if utils::path_exists(&abs_path) {
            Err(Error::ComponentConflict {
                name: component.to_owned(),
                path: relpath.clone(),
            })
        } else {
            if let Some(p) = abs_path.parent() {
                try!(utils::ensure_dir_exists("component", p, rustup_utils::NotifyHandler::none()));
            }
            let file = try!(File::create(&abs_path).map_err(|e| {
                rustup_utils::Error::WritingFile {
                    name: "component",
                    path: abs_path,
                    error: e,
                }
            }));
            Ok((ChangedItem::AddedFile(relpath), file))
        }
    }
    fn copy_file(prefix: &InstallPrefix,
                 component: &str,
                 relpath: PathBuf,
                 src: &Path)
                 -> Result<Self> {
        let abs_path = prefix.abs_path(&relpath);
        if utils::path_exists(&abs_path) {
            Err(Error::ComponentConflict {
                name: component.to_owned(),
                path: relpath.clone(),
            })
        } else {
            if let Some(p) = abs_path.parent() {
                try!(utils::ensure_dir_exists("component", p, rustup_utils::NotifyHandler::none()));
            }
            try!(utils::copy_file(src, &abs_path));
            Ok(ChangedItem::AddedFile(relpath))
        }
    }
    fn copy_dir(prefix: &InstallPrefix, component: &str, relpath: PathBuf, src: &Path) -> Result<Self> {
        let abs_path = prefix.abs_path(&relpath);
        if utils::path_exists(&abs_path) {
            Err(Error::ComponentConflict {
                name: component.to_owned(),
                path: relpath.clone(),
            })
        } else {
            if let Some(p) = abs_path.parent() {
                try!(utils::ensure_dir_exists("component", p, rustup_utils::NotifyHandler::none()));
            }
            try!(utils::copy_dir(src, &abs_path, rustup_utils::NotifyHandler::none()));
            Ok(ChangedItem::AddedDir(relpath))
        }
    }
    fn remove_file(prefix: &InstallPrefix, component: &str, relpath: PathBuf, temp_cfg: &'a temp::Cfg) -> Result<Self> {
        let abs_path = prefix.abs_path(&relpath);
        let backup = try!(temp_cfg.new_file());
        if !utils::path_exists(&abs_path) {
            Err(Error::ComponentMissingFile {
                name: component.to_owned(),
                path: relpath.clone(),
            })
        } else {
            try!(utils::rename_file("component", &abs_path, &backup));
            Ok(ChangedItem::RemovedFile(relpath, backup))
        }
    }
    fn remove_dir(prefix: &InstallPrefix, component: &str, relpath: PathBuf, temp_cfg: &'a temp::Cfg) -> Result<Self> {
        let abs_path = prefix.abs_path(&relpath);
        let backup = try!(temp_cfg.new_directory());
        if !utils::path_exists(&abs_path) {
            Err(Error::ComponentMissingDir {
                name: component.to_owned(),
                path: relpath.clone(),
            })
        } else {
            try!(utils::rename_dir("component", &abs_path, &backup.join("bk")));
            Ok(ChangedItem::RemovedDir(relpath, backup))
        }
    }
    fn modify_file(prefix: &InstallPrefix, relpath: PathBuf, temp_cfg: &'a temp::Cfg) -> Result<Self> {
        let abs_path = prefix.abs_path(&relpath);

        if utils::is_file(&abs_path) {
            let backup = try!(temp_cfg.new_file());
            try!(utils::copy_file(&abs_path, &backup));
            Ok(ChangedItem::ModifiedFile(relpath, Some(backup)))
        } else {
            if let Some(p) = abs_path.parent() {
                try!(utils::ensure_dir_exists("component", p, rustup_utils::NotifyHandler::none()));
            }
            Ok(ChangedItem::ModifiedFile(relpath, None))
        }
    }
}


