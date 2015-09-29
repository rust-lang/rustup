use std::fs;
use std::path::{Path, PathBuf};
use std::io;
use std::ops;
use std::fmt::{self, Display};
use utils::raw;

pub use self::Notification::*;

pub enum Error {
	CreatingRoot { path: PathBuf, error: io::Error },
	CreatingFile { path: PathBuf, error: io::Error },
	CreatingDirectory { path: PathBuf, error: io::Error },
}

pub type Result<T> = ::std::result::Result<T, Error>;
pub type NotifyHandler = ::notify::NotifyHandler<for<'a> Fn(Notification<'a>)>;

pub enum Notification<'a> {
	CreatingRoot(&'a Path),
	CreatingFile(&'a Path),
	CreatingDirectory(&'a Path),
	FileDeletion(&'a Path, io::Result<()>),
	DirectoryDeletion(&'a Path, io::Result<()>),
}

pub struct Cfg {
	root_directory: PathBuf,
	notify_handler: NotifyHandler,
}

pub struct Dir<'a> {
	cfg: &'a Cfg,
	path: PathBuf,
}

pub struct File<'a> {
	cfg: &'a Cfg,
	path: PathBuf,
}

impl<'a> Notification<'a> {
	pub fn is_verbose(&self) -> bool {
		match *self {
			CreatingRoot(_) | CreatingFile(_) | CreatingDirectory(_) =>
				true,
			FileDeletion(_, ref result) | DirectoryDeletion(_, ref result) =>
				result.is_ok(),
		}
	}
}

impl<'a> Display for Notification<'a> {
	fn fmt(&self, f: &mut fmt::Formatter) -> ::std::result::Result<(), fmt::Error> {
		match *self {
			CreatingRoot(path) =>
				write!(f, "creating temp root: {}", path.display()),
			CreatingFile(path) =>
				write!(f, "creating temp file: {}", path.display()),
			CreatingDirectory(path) =>
				write!(f, "creating temp directory: {}", path.display()),
			FileDeletion(path, ref result) =>
				if result.is_ok() {
					write!(f, "deleted temp file: {}", path.display())
				} else {
					write!(f, "could not delete temp file: {}", path.display())
				},
			DirectoryDeletion(path, ref result) =>
				if result.is_ok() {
					write!(f, "deleted temp directory: {}", path.display())
				} else {
					write!(f, "could not delete temp directory: {}", path.display())
				},
		}
	}
}

impl Cfg {
	pub fn new(root_directory: PathBuf, notify_handler: NotifyHandler) -> Self {
		Cfg {
			root_directory: root_directory,
			notify_handler: notify_handler,
		}
	}
	
	pub fn create_root(&self) -> Result<bool> {
		raw::ensure_dir_exists(&self.root_directory, |p| {
			self.notify_handler.call(CreatingRoot(p));
		}).map_err(|e| Error::CreatingRoot { path: PathBuf::from(&self.root_directory), error: e })
	}
	
	pub fn new_directory(&self) -> Result<Dir> {
		try!(self.create_root());
		
		loop {
			let temp_name = raw::random_string(16) + "_dir";
			
			let temp_dir = self.root_directory.join(temp_name);
			
			// This is technically racey, but the probability of getting the same
			// random names at exactly the same time is... low.
			if !raw::path_exists(&temp_dir) {
				self.notify_handler.call(CreatingDirectory(&temp_dir));
				try!(fs::create_dir(&temp_dir)
					.map_err(|e| Error::CreatingDirectory { path: PathBuf::from(&temp_dir), error: e }));
				return Ok(Dir { cfg: self, path: temp_dir });
			}
		}
	}

	pub fn new_file(&self) -> Result<File> {
		try!(self.create_root());
		
		loop {
			let temp_name = raw::random_string(16) + "_file";
			
			let temp_file = self.root_directory.join(temp_name);
			
			// This is technically racey, but the probability of getting the same
			// random names at exactly the same time is... low.
			if !raw::path_exists(&temp_file) {
				self.notify_handler.call(CreatingFile(&temp_file));
				try!(fs::File::create(&temp_file)
					.map_err(|e| Error::CreatingFile { path: PathBuf::from(&temp_file), error: e }));
				return Ok(File { cfg: self, path: temp_file });
			}
		}
	}
}

impl<'a> ops::Deref for Dir<'a> {
	type Target = Path;
	
	fn deref(&self) -> &Path {
		ops::Deref::deref(&self.path)
	}
}

impl<'a> ops::Deref for File<'a> {
	type Target = Path;
	
	fn deref(&self) -> &Path {
		ops::Deref::deref(&self.path)
	}
}

impl<'a> Drop for Dir<'a> {
	fn drop(&mut self) {
		if raw::is_directory(&self.path) {
			self.cfg.notify_handler.call(DirectoryDeletion(&self.path, fs::remove_dir_all(&self.path)));
		}
	}
}

impl<'a> Drop for File<'a> {
	fn drop(&mut self) {
		if raw::is_file(&self.path) {
			self.cfg.notify_handler.call(FileDeletion(&self.path, fs::remove_file(&self.path)));
		}
	}
}
