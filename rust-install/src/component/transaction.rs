// FIXME: This uses ensure_dir_exists in some places but rollback does
// not remove any dirs created by it.

use utils;
use temp;
use install::InstallPrefix;
use errors::*;

use std::fs::File;
use std::io::Write;
use std::path::Path;

// This is the set of fundamental operations supported on a Transaction. More complicated operations,
// such as installing a package, or updating a component, distill down into a series of these
// primitives.
enum ChangedItem<'a> {
    AddedFile(String),
    AddedDir(String),
    RemovedFile(String, temp::File<'a>),
    RemovedDir(String, temp::Dir<'a>),
    ModifiedFile(String, Option<temp::File<'a>>),
}

impl<'a> ChangedItem<'a> {
    fn roll_back(&self, prefix: &InstallPrefix) -> Result<()> {
        use self::ChangedItem::*;
        match *self {
            AddedFile(ref path) => try!(utils::remove_file("component", &prefix.abs_path(path))),
            AddedDir(ref path) => {
                try!(utils::remove_dir("component",
                                       &prefix.abs_path(path),
                                       utils::NotifyHandler::none()))
            }
            RemovedFile(ref path, ref tmp) | ModifiedFile(ref path, Some(ref tmp)) => {
                try!(utils::rename_file("component", &tmp, &prefix.abs_path(path)))
            }
            RemovedDir(ref path, ref tmp) => {
                try!(utils::rename_dir("component", &tmp, &prefix.abs_path(path)))
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
    fn add_file(prefix: &InstallPrefix, component: &str, path: String) -> Result<(Self, File)> {
        let abs_path = prefix.abs_path(&path);
        if utils::path_exists(&abs_path) {
            Err(Error::ComponentConflict {
                name: component.to_owned(),
                path: path.clone(),
            })
        } else {
            if let Some(p) = abs_path.parent() {
                try!(utils::ensure_dir_exists("component", p, utils::NotifyHandler::none()));
            }
            let file = try!(File::create(&abs_path).map_err(|e| {
                utils::Error::WritingFile {
                    name: "component",
                    path: abs_path,
                    error: e,
                }
            }));
            Ok((ChangedItem::AddedFile(path), file))
        }
    }
    fn copy_file(prefix: &InstallPrefix,
                 component: &str,
                 path: String,
                 src: &Path)
                 -> Result<Self> {
        let abs_path = prefix.abs_path(&path);
        if utils::path_exists(&abs_path) {
            Err(Error::ComponentConflict {
                name: component.to_owned(),
                path: path.clone(),
            })
        } else {
            if let Some(p) = abs_path.parent() {
                try!(utils::ensure_dir_exists("component", p, utils::NotifyHandler::none()));
            }
            try!(utils::copy_file(src, &abs_path));
            Ok(ChangedItem::AddedFile(path))
        }
    }
    fn add_dir(prefix: &InstallPrefix, component: &str, path: String) -> Result<Self> {
        let abs_path = prefix.abs_path(&path);
        if utils::path_exists(&abs_path) {
            Err(Error::ComponentConflict {
                name: component.to_owned(),
                path: path.clone(),
            })
        } else {
            try!(utils::ensure_dir_exists("component", &abs_path, utils::NotifyHandler::none()));
            Ok(ChangedItem::AddedDir(path))
        }
    }
    fn copy_dir(prefix: &InstallPrefix, component: &str, path: String, src: &Path) -> Result<Self> {
        let abs_path = prefix.abs_path(&path);
        if utils::path_exists(&abs_path) {
            Err(Error::ComponentConflict {
                name: component.to_owned(),
                path: path.clone(),
            })
        } else {
            if let Some(p) = abs_path.parent() {
                try!(utils::ensure_dir_exists("component", p, utils::NotifyHandler::none()));
            }
            try!(utils::copy_dir(src, &abs_path, utils::NotifyHandler::none()));
            Ok(ChangedItem::AddedDir(path))
        }
    }
    fn remove_file(prefix: &InstallPrefix, path: String, temp_cfg: &'a temp::Cfg) -> Result<Self> {
        let abs_path = prefix.abs_path(&path);
        let backup = try!(temp_cfg.new_file());
        try!(utils::rename_file("component", &abs_path, &backup));
        Ok(ChangedItem::RemovedFile(path, backup))
    }
    fn remove_dir(prefix: &InstallPrefix, path: String, temp_cfg: &'a temp::Cfg) -> Result<Self> {
        let abs_path = prefix.abs_path(&path);
        let backup = try!(temp_cfg.new_directory());
        try!(utils::rename_dir("component", &abs_path, &backup));
        Ok(ChangedItem::RemovedDir(path, backup))
    }
    fn modify_file(prefix: &InstallPrefix, path: String, temp_cfg: &'a temp::Cfg) -> Result<Self> {
        let abs_path = prefix.abs_path(&path);

        if utils::is_file(&abs_path) {
            let backup = try!(temp_cfg.new_file());
            try!(utils::copy_file(&abs_path, &backup));
            Ok(ChangedItem::ModifiedFile(path, Some(backup)))
        } else {
            if let Some(p) = abs_path.parent() {
                try!(utils::ensure_dir_exists("component", p, utils::NotifyHandler::none()));
            }
            Ok(ChangedItem::ModifiedFile(path, None))
        }
    }
}


// A Transaction tracks changes to the file system, allowing them to be rolled back in case
// of an error. Instead of deleting or overwriting file, the old copies are moved to a
// temporary folder. If the transaction is rolled back, they will be moved back into place.
// If the transaction is committed, these files are automatically cleaned up using the
// temp system.
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
    pub fn commit(mut self) {
        self.committed = true;
    }
    fn change(&mut self, item: ChangedItem<'a>) {
        self.changes.push(item);
    }
    pub fn add_file(&mut self, component: &str, path: String) -> Result<File> {
        let (item, file) = try!(ChangedItem::add_file(&self.prefix, component, path));
        self.change(item);
        Ok(file)
    }
    pub fn copy_file(&mut self, component: &str, path: String, src: &Path) -> Result<()> {
        let item = try!(ChangedItem::copy_file(&self.prefix, component, path, src));
        self.change(item);
        Ok(())
    }
    pub fn add_dir(&mut self, component: &str, path: String) -> Result<()> {
        let item = try!(ChangedItem::add_dir(&self.prefix, component, path));
        self.change(item);
        Ok(())
    }
    pub fn copy_dir(&mut self, component: &str, path: String, src: &Path) -> Result<()> {
        let item = try!(ChangedItem::copy_dir(&self.prefix, component, path, src));
        self.change(item);
        Ok(())
    }
    pub fn remove_file(&mut self, path: String) -> Result<()> {
        let item = try!(ChangedItem::remove_file(&self.prefix, path, &self.temp_cfg));
        self.change(item);
        Ok(())
    }
    pub fn remove_dir(&mut self, path: String) -> Result<()> {
        let item = try!(ChangedItem::remove_dir(&self.prefix, path, &self.temp_cfg));
        self.change(item);
        Ok(())
    }
    pub fn write_file(&mut self, component: &str, path: String, content: String) -> Result<()> {
        let (item, mut file) = try!(ChangedItem::add_file(&self.prefix, component, path.clone()));
        self.change(item);
        try!(write!(file, "{}", content).map_err(|e| {
            utils::Error::WritingFile {
                name: "component",
                path: self.prefix.abs_path(&path),
                error: e,
            }
        }));
        Ok(())
    }
    pub fn modify_file(&mut self, path: String) -> Result<()> {
        let item = try!(ChangedItem::modify_file(&self.prefix, path, &self.temp_cfg));
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

// If a Transaction is dropped without being committed, the changes are automatically
// rolled back.
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
