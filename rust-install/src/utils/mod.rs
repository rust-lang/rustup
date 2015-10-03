
use errors::*;
use std::path::{Path, PathBuf};
use std::fs;
use std::process::Command;
use hyper;

pub mod raw;

pub use self::raw::{
	is_directory,
	is_file,
	path_exists,
	to_absolute,
	if_not_empty,
	random_string,
	prefix_arg,
};

pub fn ensure_dir_exists(name: &'static str, path: &Path, notify_handler: &NotifyHandler) -> Result<bool> {
	raw::ensure_dir_exists(path, |p| {
		notify_handler.call(CreatingDirectory(name, p))
	}).map_err(|e| Error::CreatingDirectory { name: name, path: PathBuf::from(path), error: e })
}

pub fn read_file(name: &'static str, path: &Path) -> Result<String> {
	raw::read_file(path)
		.map_err(|e| Error::ReadingFile { name: name, path: PathBuf::from(path), error: e })
}

pub fn write_file(name: &'static str, path: &Path, contents: &str) -> Result<()> {
	raw::write_file(path, contents)
		.map_err(|e| Error::WritingFile { name: name, path: PathBuf::from(path), error: e })
}

pub fn append_file(name: &'static str, path: &Path, line: &str) -> Result<()> {
	raw::append_file(path, line)
		.map_err(|e| Error::WritingFile { name: name, path: PathBuf::from(path), error: e })
}

pub fn rename_file(name: &'static str, src: &Path, dest: &Path) -> Result<()> {
	fs::rename(src, dest)
		.map_err(|e| Error::RenamingFile {
			name: name,
			src: PathBuf::from(src),
			dest: PathBuf::from(dest),
			error: e
		})
}

pub fn rename_dir(name: &'static str, src: &Path, dest: &Path) -> Result<()> {
	fs::rename(src, dest)
		.map_err(|e| Error::RenamingDirectory {
			name: name,
			src: PathBuf::from(src),
			dest: PathBuf::from(dest),
			error: e
		})
}

pub fn filter_file<F: FnMut(&str) -> bool>(name: &'static str, src: &Path, dest: &Path, filter: F) -> Result<usize> {
	raw::filter_file(src, dest, filter)
		.map_err(|e| Error::FilteringFile {
			name: name,
			src: PathBuf::from(src),
			dest: PathBuf::from(dest),
			error: e
		})
}

pub fn match_file<T, F: FnMut(&str) -> Option<T>>(name: &'static str, src: &Path, f: F) -> Result<Option<T>> {
	raw::match_file(src, f)
		.map_err(|e| Error::ReadingFile {
			name: name,
			path: PathBuf::from(src),
			error: e
		})
}

pub fn canonicalize_path(path: &Path, notify_handler: &NotifyHandler) -> PathBuf {
	fs::canonicalize(path)
		.unwrap_or_else(|_| {
			notify_handler.call(Notification::NoCanonicalPath(path));
			PathBuf::from(path)
		})
}

pub fn download_file(url: hyper::Url, path: &Path, notify_handler: &NotifyHandler) -> Result<()> {
	notify_handler.call(DownloadingFile(&url, path));
	raw::download_file(url.clone(), path)
		.map_err(|_| Error::DownloadingFile { url: url, path: PathBuf::from(path) })
}

pub fn cmd_status(name: &'static str, mut cmd: Command) -> Result<()> {
	cmd.status()
		.map_err(|e| Error::RunningCommand { name: name, error: e })
		.and_then(|s| {
			if s.success() {
				Ok(())
			} else {
				Err(Error::CommandStatus {
					name: name,
					status: s,
				})
			}
		})
}

pub fn assert_is_file(path: &Path) -> Result<()> {
	if !is_file(path) {
		Err(Error::NotAFile { path: PathBuf::from(path) })
	} else {
		Ok(())
	}
}

pub fn assert_is_directory(path: &Path) -> Result<()> {
	if !is_directory(path) {
		Err(Error::NotADirectory { path: PathBuf::from(path) })
	} else {
		Ok(())
	}
}

pub fn symlink_dir(src: &Path, dest: &Path, notify_handler: &NotifyHandler) -> Result<()> {
	notify_handler.call(LinkingDirectory(src, dest));
	raw::symlink_dir(src, dest)
		.ok_or_else(|| Error::LinkingDirectory(PathBuf::from(src), PathBuf::from(dest)))
}

pub fn copy_dir(src: &Path, dest: &Path, notify_handler: &NotifyHandler) -> Result<()> {
	notify_handler.call(CopyingDirectory(src, dest));
	raw::copy_dir(src, dest)
		.ok_or_else(|| Error::CopyingDirectory(PathBuf::from(src), PathBuf::from(dest)))
}

pub fn copy_file(src: &Path, dest: &Path) -> Result<()> {
	fs::copy(src, dest)
		.map_err(|_| Error::CopyingFile(PathBuf::from(src), PathBuf::from(dest)))
		.map(|_|())
}

pub fn remove_dir(name: &'static str, path: &Path, notify_handler: &NotifyHandler) -> Result<()> {
	notify_handler.call(RemovingDirectory(name, path));
	fs::remove_dir_all(path)
		.map_err(|e| Error::RemovingDirectory { name: name, path: PathBuf::from(path), error: e })
}

pub fn remove_file(name: &'static str, path: &Path) -> Result<()> {
	fs::remove_file(path)
		.map_err(|e| Error::RemovingFile { name: name, path: PathBuf::from(path), error: e })
}

pub fn read_dir(name: &'static str, path: &Path) -> Result<fs::ReadDir> {
	fs::read_dir(path)
		.map_err(|e| Error::ReadingDirectory { name: name, path: PathBuf::from(path), error: e })
}

pub fn open_browser(path: &Path) -> Result<()> {
	if let Ok(true) = raw::open_browser(path) {
		Ok(())
	} else {
		Err(Error::OpeningBrowser)
	}
}
