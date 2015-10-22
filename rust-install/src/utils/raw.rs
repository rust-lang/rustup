
use std::fs;
use std::path::{Path, PathBuf};
use std::io;
use std::char::from_u32;
use std::io::Write;
use std::process::{Command, Stdio, ExitStatus};
use std::ffi::{OsStr, OsString};
use hyper::{self, Client};
use openssl::crypto::hash::Hasher;

use rand::random;

pub fn ensure_dir_exists<P: AsRef<Path>, F: FnOnce(&Path)>(path: P, callback: F) -> io::Result<bool> {
	if !is_directory(path.as_ref()) {
		callback(path.as_ref());
		fs::create_dir_all(path.as_ref()).map(|()| true)
	} else {
		Ok(false)
	}
}

pub fn is_directory<P: AsRef<Path>>(path: P) -> bool {
	fs::metadata(path).ok().as_ref().map(fs::Metadata::is_dir) == Some(true)
}

pub fn is_file<P: AsRef<Path>>(path: P) -> bool {
	fs::metadata(path).ok().as_ref().map(fs::Metadata::is_file) == Some(true)
}

pub fn path_exists<P: AsRef<Path>>(path: P) -> bool {
	fs::metadata(path).is_ok()
}

pub fn random_string(length: usize) -> String {
	let chars = b"abcdefghijklmnopqrstuvwxyz0123456789_";
	(0..length).map(|_| from_u32(chars[random::<usize>() % chars.len()] as u32).unwrap()).collect()
}

pub fn if_not_empty<S: PartialEq<str>>(s: S) -> Option<S> {
	if s == *"" {
		None
	} else {
		Some(s)
	}
}

pub fn write_file(path: &Path, contents: &str) -> io::Result<()> {
	let mut file = try!(fs::OpenOptions::new()
		.write(true)
		.truncate(true)
		.create(true)
		.open(path));
	
	try!(io::Write::write_all(&mut file, contents.as_bytes()));
	
	try!(file.sync_data());
	
	Ok(())
}

pub fn read_file(path: &Path) -> io::Result<String> {
	let mut file = try!(fs::OpenOptions::new()
		.read(true)
		.open(path));
	
	let mut contents = String::new();
	
	try!(io::Read::read_to_string(&mut file, &mut contents));
	
	Ok(contents)
}

pub fn filter_file<F: FnMut(&str) -> bool>(src: &Path, dest: &Path, mut filter: F) -> io::Result<usize> {
	let src_file = try!(fs::File::open(src));
	let dest_file = try!(fs::File::create(dest));
	
	let mut reader = io::BufReader::new(src_file);
	let mut writer = io::BufWriter::new(dest_file);
	let mut removed = 0;
	
	for result in io::BufRead::lines(&mut reader) {
		let line = try!(result);
		if filter(&line) {
			try!(writeln!(&mut writer, "{}", &line));
		} else {
			removed += 1;
		}
	}
	
	try!(writer.flush());
	
	Ok(removed)
}

pub fn match_file<T, F: FnMut(&str) -> Option<T>>(src: &Path, mut f: F) -> io::Result<Option<T>> {
	let src_file = try!(fs::File::open(src));
	
	let mut reader = io::BufReader::new(src_file);
	
	for result in io::BufRead::lines(&mut reader) {
		let line = try!(result);
		if let Some(r) = f(&line) {
			return Ok(Some(r));
		}
	}
	
	Ok(None)
}

pub fn append_file(dest: &Path, line: &str) -> io::Result<()> {
	let mut dest_file = try!(fs::OpenOptions::new()
		.write(true)
		.append(true)
		.create(true)
		.open(dest)
		);
	
	try!(writeln!(&mut dest_file, "{}", line));
	
	try!(dest_file.sync_data());
	
	Ok(())
}

pub enum DownloadError {
	Status(hyper::status::StatusCode),
	Network(hyper::Error),
	File(io::Error),
}
pub type DownloadResult<T> = Result<T, DownloadError>;

pub fn download_file<P: AsRef<Path>>(url: hyper::Url, path: P, mut hasher: Option<&mut Hasher>) -> DownloadResult<()> {
	let client = Client::new();

	let mut res = try!(client.get(url).send().map_err(DownloadError::Network));
	if res.status != hyper::Ok { return Err(DownloadError::Status(res.status)); }
	
	let buffer_size = 0x10000;
	let mut buffer = vec![0u8; buffer_size];
	
	let mut file = try!(fs::File::create(path).map_err(DownloadError::File));
	
	loop {
		let bytes_read = try!(io::Read::read(&mut res, &mut buffer)
			.map_err(hyper::Error::Io)
			.map_err(DownloadError::Network)
			);
		
		if bytes_read != 0 {
			if let Some(ref mut h) = hasher {
				try!(io::Write::write_all(*h, &mut buffer[0..bytes_read]).map_err(DownloadError::File));
			}
			try!(io::Write::write_all(&mut file, &mut buffer[0..bytes_read]).map_err(DownloadError::File));
		} else {
			try!(file.sync_data().map_err(DownloadError::File));
			return Ok(());
		}
	}
}

pub fn symlink_dir(src: &Path, dest: &Path) -> io::Result<()> {
	#[cfg(windows)]
	fn symlink_dir_inner(src: &Path, dest: &Path) -> io::Result<()> {
		::std::os::windows::fs::symlink_dir(src, dest)
	}
	#[cfg(not(windows))]
	fn symlink_dir_inner(src: &Path, dest: &Path) -> io::Result<()> {
		::std::os::unix::fs::symlink(src, dest)
	}
	
	// This is stupid, but seems to be the safest way to delete
	// a directory that may be a symbol link, without accidentally
	// deleting the contents too...
	let _ = fs::remove_file(dest);
	let _ = fs::remove_dir(dest);
	let _ = fs::remove_dir_all(dest);
	symlink_dir_inner(src, dest)
}

pub fn symlink_file(src: &Path, dest: &Path) -> io::Result<()> {
	#[cfg(windows)]
	fn symlink_file_inner(src: &Path, dest: &Path) -> io::Result<()> {
		::std::os::windows::fs::symlink_file(src, dest)
	}
	#[cfg(not(windows))]
	fn symlink_file_inner(src: &Path, dest: &Path) -> io::Result<()> {
		::std::os::unix::fs::symlink(src, dest)
	}
	
	let _ = fs::remove_file(dest);
	symlink_file_inner(src, dest)
}

pub fn hardlink(src: &Path, dest: &Path) -> io::Result<()> {
	let _ = fs::remove_file(dest);
	fs::hard_link(src, dest)
}

pub enum CommandError {
	Io(io::Error),
	Status(ExitStatus),
}

pub type CommandResult<T> = Result<T, CommandError>;

pub fn cmd_status(cmd: &mut Command) -> CommandResult<()> {
	cmd.status().map_err(CommandError::Io).and_then(|s| {
		if s.success() {
			Ok(())
		} else {
			Err(CommandError::Status(s))
		}
	})
}

pub fn copy_dir(src: &Path, dest: &Path) -> CommandResult<()> {
	#[cfg(windows)]
	fn copy_dir_inner(src: &Path, dest: &Path) -> CommandResult<()> {
		cmd_status(Command::new("robocopy").arg(src).arg(dest).arg("/E"))
	}
	#[cfg(not(windows))]
	fn copy_dir_inner(src: &Path, dest: &Path) -> CommandResult<()> {
		cmd_status(Command::new("cp").arg("-R").arg(src).arg(dest))
	}
	
	copy_dir_inner(src, dest)
}

pub fn prefix_arg<S: AsRef<OsStr>>(name: &str, s: S) -> OsString {
	let mut arg = OsString::from(name);
	arg.push(s);
	arg
}

pub fn open_browser(path: &Path) -> io::Result<bool> {
	#[cfg(not(windows))]
	fn has_cmd(cmd: &&str) -> bool {
		Command::new("command")
			.arg("-v").arg(cmd)
			.stdin(Stdio::null())
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.status()
			.map(|s| s.success())
			.unwrap_or(false)
	}
	#[cfg(not(windows))]
	fn inner(path: &Path) -> io::Result<bool> {
		let commands = ["xdg-open", "open", "firefox", "chromium"];
		if let Some(cmd) = commands.iter().map(|s| *s).filter(has_cmd).next() {
			Command::new(cmd)
				.arg(path)
				.stdin(Stdio::null())
				.stdout(Stdio::null())
				.stderr(Stdio::null())
				.spawn()
				.map(|_| true)
		} else {
			Ok(false)
		}
	}
	#[cfg(windows)]
	fn inner(path: &Path) -> io::Result<bool> {
		Command::new("start")
			.arg(path)
			.stdin(Stdio::null())
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.spawn()
			.map(|_| true)
	}
	inner(path)
}
pub fn home_dir() -> Option<PathBuf> {
	#[cfg(not(windows))]
	fn inner() -> Option<PathBuf> {
		::std::env::home_dir()
	}
	#[cfg(windows)]
	fn inner() -> Option<PathBuf> {
		windows::get_special_folder(&windows::FOLDERID_Profile).ok()
	}
	
	inner()
}

#[cfg(windows)]
pub mod windows {
	use winapi::*;
	use std::io;
	use std::path::PathBuf;
	use std::ptr;
	use std::slice;
	use std::ffi::OsString;
	use std::os::windows::ffi::OsStringExt;
	use shell32;
	use ole32;
	
	#[allow(non_upper_case_globals)]
	pub const FOLDERID_LocalAppData: GUID = GUID {
		Data1: 0xF1B32785, 
		Data2: 0x6FBA,
		Data3: 0x4FCF,
		Data4: [0x9D, 0x55, 0x7B, 0x8E, 0x7F, 0x15, 0x70, 0x91],
	};
	#[allow(non_upper_case_globals)]
	pub const FOLDERID_Profile: GUID = GUID {
		Data1: 0x5E6C858F, 
		Data2: 0x0E22,
		Data3: 0x4760,
		Data4: [0x9A, 0xFE, 0xEA, 0x33, 0x17, 0xB6, 0x71, 0x73],
	};
	
	pub fn get_special_folder(id: &shtypes::KNOWNFOLDERID) -> io::Result<PathBuf> {
		
		
		let mut path = ptr::null_mut();
		let result;
		
		unsafe {
			let code = shell32::SHGetKnownFolderPath(id, 0, ptr::null_mut(), &mut path);
			if code == 0 {
				let mut length = 0usize;
				while *path.offset(length as isize) != 0 {
					length += 1;
				}
				let slice = slice::from_raw_parts(path, length);
				result = Ok(OsString::from_wide(slice).into());
			} else {
				result = Err(io::Error::from_raw_os_error(code));
			}
			ole32::CoTaskMemFree(path as *mut _);
		}
		result
	}
}
