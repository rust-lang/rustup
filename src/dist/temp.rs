use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{fmt, fs, ops};

pub(crate) use anyhow::{Context as _, Result};
use thiserror::Error as ThisError;

use crate::notifications::Notification;
use crate::utils::{self, raw};

#[derive(Debug, ThisError)]
pub(crate) enum CreatingError {
    #[error("could not create temp root {}" ,.0.display())]
    Root(PathBuf),
    #[error("could not create temp file {}",.0.display())]
    File(PathBuf),
    #[error("could not create temp directory {}",.0.display())]
    Directory(PathBuf),
}

#[derive(Debug)]
pub(crate) struct Dir {
    cfg: Arc<Context>,
    path: PathBuf,
}

impl ops::Deref for Dir {
    type Target = Path;

    fn deref(&self) -> &Path {
        self.path.as_path()
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        if raw::is_directory(&self.path) {
            let n = Notification::DirectoryDeletion(
                &self.path,
                remove_dir_all::remove_dir_all(&self.path),
            );
            (self.cfg.notify_handler)(n);
        }
    }
}

#[derive(Debug)]
pub struct File {
    cfg: Arc<Context>,
    path: PathBuf,
}

impl ops::Deref for File {
    type Target = Path;

    fn deref(&self) -> &Path {
        self.path.as_path()
    }
}

impl Drop for File {
    fn drop(&mut self) {
        if raw::is_file(&self.path) {
            let n = Notification::FileDeletion(&self.path, fs::remove_file(&self.path));
            (self.cfg.notify_handler)(n);
        }
    }
}

pub type NotifyHandler = dyn for<'a> Fn(Notification<'a>) + Sync + Send;

#[derive(Clone)]
pub struct Context {
    root_directory: PathBuf,
    pub dist_server: String,
    notify_handler: Arc<NotifyHandler>,
}

impl Context {
    pub fn new(
        root_directory: PathBuf,
        dist_server: &str,
        notify_handler: Arc<NotifyHandler>,
    ) -> Self {
        Self {
            root_directory,
            dist_server: dist_server.to_owned(),
            notify_handler,
        }
    }

    pub(crate) fn create_root(&self) -> Result<bool> {
        raw::ensure_dir_exists(&self.root_directory, |p| {
            (self.notify_handler)(Notification::CreatingRoot(p));
        })
        .with_context(|| CreatingError::Root(PathBuf::from(&self.root_directory)))
    }

    pub(crate) fn new_directory(self: Arc<Self>) -> Result<Dir> {
        self.create_root()?;

        loop {
            let temp_name = raw::random_string(16) + "_dir";

            let temp_dir = self.root_directory.join(temp_name);

            // This is technically racey, but the probability of getting the same
            // random names at exactly the same time is... low.
            if !raw::path_exists(&temp_dir) {
                (self.notify_handler)(Notification::CreatingDirectory("temp", &temp_dir));
                fs::create_dir(&temp_dir)
                    .with_context(|| CreatingError::Directory(PathBuf::from(&temp_dir)))?;
                return Ok(Dir {
                    cfg: self,
                    path: temp_dir,
                });
            }
        }
    }

    pub fn new_file(self: Arc<Self>) -> Result<File> {
        self.new_file_with_ext("", "")
    }

    pub(crate) fn new_file_with_ext(self: Arc<Self>, prefix: &str, ext: &str) -> Result<File> {
        self.create_root()?;

        loop {
            let temp_name = prefix.to_owned() + &raw::random_string(16) + "_file" + ext;

            let temp_file = self.root_directory.join(temp_name);

            // This is technically racey, but the probability of getting the same
            // random names at exactly the same time is... low.
            if !raw::path_exists(&temp_file) {
                (self.notify_handler)(Notification::CreatingFile(&temp_file));
                fs::File::create(&temp_file)
                    .with_context(|| CreatingError::File(PathBuf::from(&temp_file)))?;
                return Ok(File {
                    cfg: self,
                    path: temp_file,
                });
            }
        }
    }

    pub(crate) fn clean(&self) {
        utils::delete_dir_contents_following_links(&self.root_directory);
    }
}

impl fmt::Debug for Context {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cfg")
            .field("root_directory", &self.root_directory)
            .field("notify_handler", &"...")
            .finish()
    }
}
