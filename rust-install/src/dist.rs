
use temp;
use errors::*;
use utils;

use std::path::Path;
use std::fmt;
use std::env;

use regex::Regex;
use hyper;

pub const DEFAULT_DIST_ROOT: &'static str = "https://static.rust-lang.org/dist";
pub const UPDATE_HASH_LEN: usize = 20;

pub enum ToolchainDesc {
	Channel(String),
	ChannelDate(String, String),
}

impl ToolchainDesc {
	pub fn from_str(name: &str) -> Option<Self> {
		let re = Regex::new(r"^(nightly|beta|stable)(?:-(\d{4}-\d{2}-\d{2}))?$").unwrap();
		re.captures(name).map(|c| {
			let channel = c.at(1).unwrap().to_owned();
			if let Some(date) = c.at(2) {
				ToolchainDesc::ChannelDate(channel, date.to_owned())
			} else {
				ToolchainDesc::Channel(channel)
			}
		})
	}
	
	pub fn manifest_url(&self, dist_root: &str) -> String {
		match *self {
			ToolchainDesc::Channel(ref channel) =>
				format!("{}/channel-rust-{}", dist_root, channel),
			ToolchainDesc::ChannelDate(ref channel, ref date) =>
				format!("{}/{}/channel-rust-{}", dist_root, date, channel),
		}
	}
	
	pub fn package_dir(&self, dist_root: &str) -> String {
		match *self {
			ToolchainDesc::Channel(_) =>
				format!("{}", dist_root),
			ToolchainDesc::ChannelDate(_, ref date) =>
				format!("{}/{}", dist_root, date),
		}
	}
	
	pub fn download_manifest<'a>(&self, cfg: DownloadCfg<'a>) -> Result<Manifest<'a>> {
		let url = self.manifest_url(cfg.dist_root);
		let package_dir = self.package_dir(cfg.dist_root);
		
		let manifest = try!(download_and_check(&url, None, "", cfg)).unwrap();
		
		Ok(Manifest(manifest, package_dir))
	}
}

pub struct Manifest<'a>(temp::File<'a>, String);

impl<'a> Manifest<'a> {
	pub fn package_url(&self, package: &str, target_triple: &str, ext: &str) -> Result<Option<String>> {
		let suffix = target_triple.to_owned() + ext;
		utils::match_file("manifest", &self.0, |line| {
			if line.starts_with(package) && line.ends_with(&suffix) {
				Some(format!("{}/{}", &self.1, line))
			} else {
				None
			}
		})
	}
}

impl fmt::Display for ToolchainDesc {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			ToolchainDesc::Channel(ref channel) => write!(f, "{}", channel),
			ToolchainDesc::ChannelDate(ref channel, ref date) => write!(f, "{}-{}", channel, date),
		}
	}
}
fn parse_url(url: &str) -> Result<hyper::Url> {
	hyper::Url::parse(url).map_err(|_| Error::InvalidUrl)
}

pub fn download_and_check<'a>(url: &str, update_hash: Option<&Path>, ext: &str, cfg: DownloadCfg<'a>) -> Result<Option<temp::File<'a>>> {
	let hash = try!(download_hash(url, cfg));
	let partial_hash: String = hash.chars().take(UPDATE_HASH_LEN).collect();
	
	if let Some(hash_file) = update_hash {
		if utils::is_file(hash_file) {
			if let Ok(contents) = utils::read_file("update hash", hash_file) {
				if contents == partial_hash {
					// Skip download, update hash matches
					cfg.notify_handler.call(Notification::UpdateHashMatches(&partial_hash));
					return Ok(None);
				}
			} else {
				cfg.notify_handler.call(Notification::CantReadUpdateHash(hash_file));
			}
		} else {
			cfg.notify_handler.call(Notification::NoUpdateHash(hash_file));
		}
		
		try!(utils::write_file("update hash", hash_file, &partial_hash));
	}
	
	let url = try!(parse_url(url));
	let file = try!(cfg.temp_cfg.new_file_with_ext(ext));
	try!(utils::download_file(url, &file, cfg.notify_handler));
	// TODO: Actually download and check the checksum and signature of the file
	Ok(Some(file))
}

#[derive(Copy, Clone)]
pub struct DownloadCfg<'a> {
	pub dist_root: &'a str,
	pub temp_cfg: &'a temp::Cfg,
	pub notify_handler: &'a NotifyHandler,
}

pub fn download_dist<'a>(toolchain: &str, update_hash: Option<&Path>, cfg: DownloadCfg<'a>) -> Result<Option<temp::File<'a>>> {
	let desc = try!(ToolchainDesc::from_str(toolchain)
		.ok_or(Error::InvalidToolchainName));
	
	let target_triple = try!(get_host_triple().ok_or(Error::UnsupportedHost));
	let ext = get_installer_ext();
	
	let manifest = try!(desc.download_manifest(cfg));
	
	let maybe_url = try!(manifest.package_url("rust", &target_triple, ext));
	
	let url = try!(maybe_url.ok_or(Error::UnsupportedHost));
	
	download_and_check(&url, update_hash, ext, cfg)
}

pub fn get_host_triple() -> Option<&'static str> {
	match (env::consts::ARCH, env::consts::OS, cfg!(target_env = "gnu")) {
		("x86_64", "macos", _) => Some("x86_64-apple-darwin"),
		("x86_64", "windows", true) => Some("x86_64-pc-windows-gnu"),
		("x86_64", "windows", false) => Some("x86_64-pc-windows-msvc"),
		("x86_64", "linux", _) => Some("x86_64-unknown-linux-gnu"),
		("i686", "macos", _) => Some("i686-apple-darwin"),
		("i686", "windows", true) => Some("i686-pc-windows-gnu"),
		("i686", "windows", false) => Some("i686-pc-windows-msvc"),
		("i686", "linux", _) => Some("i686-unknown-linux-gnu"),
		_ => None
	}
}

pub fn get_installer_ext() -> &'static str {
	if cfg!(windows) {
		if env::var_os("MSYSTEM").and_then(utils::if_not_empty).is_none() {
			return ".msi"
		}
	}
	".tar.gz"
}

pub fn download_hash(url: &str, cfg: DownloadCfg) -> Result<String> {
	let hash_url = try!(parse_url(&(url.to_owned() + ".sha256")));
	let hash_file = try!(cfg.temp_cfg.new_file());
	
	try!(utils::download_file(hash_url, &hash_file, cfg.notify_handler));
	
	utils::read_file("hash", &hash_file)
}
