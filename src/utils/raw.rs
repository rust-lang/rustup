
use std::fs;
use std::path::{Path, PathBuf};
use std::io;
use std::char::from_u32;
use std::env;
use std::io::Write;
use std::process::{Command, Stdio};
use std::ffi::{OsStr, OsString};
use hyper::{self, Client};

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

pub fn to_absolute<P: AsRef<Path>>(path: P) -> Option<PathBuf> {
	env::current_dir().map(|mut v| {
			v.push(path);
			v
		}).ok()
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

pub fn download_file<P: AsRef<Path>>(url: hyper::Url, path: P) -> Result<(),()> {
	let client = Client::new();

	let mut res = try!(client.get(url).send().map_err(|_|()));
	if res.status != hyper::Ok { return Err(()); }
	
	let buffer_size = 0x1000;
	let mut buffer = vec![0u8; buffer_size];
	
	let mut file = try!(fs::File::create(path).map_err(|_|()));
	
	loop {
		let bytes_read = try!(io::Read::read(&mut res, &mut buffer).map_err(|_|()));
		if bytes_read != 0 {
			try!(io::Write::write_all(&mut file, &mut buffer[0..bytes_read]).map_err(|_|()));
		} else {
			try!(file.sync_data().map_err(|_|()));
			return Ok(());
		}
	}
}

pub fn symlink_dir(src: &Path, dest: &Path) -> Option<()> {
	#[cfg(windows)]
	fn symlink_dir_inner(src: &Path, dest: &Path) -> Option<()> {
		::std::os::windows::fs::symlink_dir(src, dest).ok()
	}
	#[cfg(not(windows))]
	fn symlink_dir_inner(src: &Path, dest: &Path) -> Option<()> {
		::std::os::unix::fs::symlink(src, dest).ok()
	}
	
	symlink_dir_inner(src, dest)
}

pub fn copy_dir(src: &Path, dest: &Path) -> Option<()> {
	#[cfg(windows)]
	fn copy_dir_inner(src: &Path, dest: &Path) -> Option<()> {
		Command::new("robocopy")
			.arg(src).arg(dest).arg("/E")
			.status().ok()
			.and_then(|code| if code.success() { Some(()) } else { None })
	}
	#[cfg(not(windows))]
	fn copy_dir_inner(src: &Path, dest: &Path) -> Option<()> {
		Command::new("cp")
			.arg("-R").arg(src).arg(dest)
			.status().ok()
			.and_then(|code| if code.success() { Some(()) } else { None })
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
	fn has_cmd(cmd: &str) -> bool {
		Command::new("command")
			.arg("-v").arg(cmd)
			.stdin(Stdio::null())
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.status()
			.map(|s| s.success()) == Ok(true)
	}
	#[cfg(not(windows))]
	fn inner(path: &Path) -> io::Result<bool> {
		let commands = ["xdg-open", "open", "firefox", "chromium"];
		if let Some(cmd) = commands.filter(has_cmd).next() {
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
