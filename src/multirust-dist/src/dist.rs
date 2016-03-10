
use temp;
use errors::*;
use multirust_utils::utils;
use prefix::InstallPrefix;
use manifest::Component;
use manifest::Manifest as ManifestV2;
use manifestation::{Manifestation, UpdateStatus, Changes};
use hyper;

use std::path::Path;
use std::fmt;
use std::env;

use regex::Regex;
use openssl::crypto::hash::{Type, Hasher};
use itertools::Itertools;

pub const DEFAULT_DIST_ROOT: &'static str = "https://static.rust-lang.org/dist";
pub const UPDATE_HASH_LEN: usize = 20;

#[derive(Debug)]
pub struct ToolchainDesc {
    pub arch: Option<String>,
    pub os: Option<String>,
    pub env: Option<String>,
    // Either "nightly", "stable", "beta", or an explicit version number
    pub channel: String,
    pub date: Option<String>,
}

impl ToolchainDesc {
    pub fn from_str(name: &str) -> Result<Self> {
        let archs = ["i686", "x86_64"];
        let oses = ["pc-windows", "unknown-linux", "apple-darwin"];
        let envs = ["gnu", "msvc"];
        let channels = ["nightly", "beta", "stable",
                        r"\d{1}\.\d{1}\.\d{1}",
                        r"\d{1}\.\d{2}\.\d{1}"];

        let pattern = format!(
            r"^(?:({})-)?(?:({})-)?(?:({})-)?({})(?:-(\d{{4}}-\d{{2}}-\d{{2}}))?$",
            archs.join("|"), oses.join("|"), envs.join("|"), channels.join("|")
            );

        let re = Regex::new(&pattern).unwrap();
        re.captures(name).map(|c| {
            fn fn_map(s: &str) -> Option<String> {
                if s == "" {
                    None
                } else {
                    Some(s.to_owned())
                }
            }

            ToolchainDesc {
                arch: c.at(1).and_then(fn_map),
                os: c.at(2).and_then(fn_map),
                env: c.at(3).and_then(fn_map),
                channel: c.at(4).unwrap().to_owned(),
                date: c.at(5).and_then(fn_map),
            }
        }).ok_or(Error::InvalidToolchainName(name.to_string()))
    }

    pub fn target_triple(&self) -> String {
        let (host_arch, host_os, host_env) = get_host_triple();
        let arch = self.arch.as_ref().map(|s| &**s).unwrap_or(host_arch);
        let os = self.os.as_ref().map(|s| &**s).unwrap_or(host_os);
        // Mixing arbitrary host envs into arbitrary target specs can't work sensibly.
        // Only provide a default when the operating system matches.
        let env = if self.env.is_none() && os == host_os {
            host_env
        } else {
            self.env.as_ref().map(|s| &**s)
        };

        if let Some(ref env) = env {
            format!("{}-{}-{}", arch, os, env)
        } else {
            format!("{}-{}", arch, os)
        }
    }

    pub fn manifest_v1_url(&self, dist_root: &str) -> String {
        match self.date {
            None => format!("{}/channel-rust-{}", dist_root, self.channel),
            Some(ref date) => format!("{}/{}/channel-rust-{}", dist_root, date, self.channel),
        }
    }

    pub fn manifest_v2_url(&self, dist_root: &str) -> String {
        format!("{}.toml", self.manifest_v1_url(dist_root))
    }

    pub fn package_dir(&self, dist_root: &str) -> String {
        match self.date {
            None => format!("{}", dist_root),
            Some(ref date) => format!("{}/{}", dist_root, date),
        }
    }

    pub fn full_spec(&self) -> String {
        let triple = self.target_triple();
        if let Some(ref date) = self.date {
            format!("{}-{}-{}", triple, &self.channel, date)
        } else {
            format!("{}-{} (tracking)", triple, &self.channel)
        }
    }

    pub fn is_tracking(&self) -> bool {
        self.date.is_none()
    }
}

#[derive(Debug)]
pub struct Manifest<'a>(temp::File<'a>, String);

impl<'a> Manifest<'a> {
    pub fn package_url(&self,
                       package: &str,
                       target_triple: &str,
                       ext: &str)
                       -> Result<Option<String>> {
        let suffix = target_triple.to_owned() + ext;
        Ok(try!(utils::match_file("manifest", &self.0, |line| {
            if line.starts_with(package) && line.ends_with(&suffix) {
                Some(format!("{}/{}", &self.1, line))
            } else {
                None
            }
        })))
    }
}

impl fmt::Display for ToolchainDesc {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(ref arch) = self.arch {
            try!(write!(f, "{}-", arch));
        }
        if let Some(ref os) = self.os {
            try!(write!(f, "{}-", os));
        }
        if let Some(ref env) = self.env {
            try!(write!(f, "{}-", env));
        }

        try!(write!(f, "{}", &self.channel));

        if let Some(ref date) = self.date {
            try!(write!(f, "-{}", date));
        }

        Ok(())
    }
}

pub fn download_and_check<'a>(url_str: &str,
                              update_hash: Option<&Path>,
                              ext: &str,
                              cfg: DownloadCfg<'a>)
                              -> Result<Option<(temp::File<'a>, String)>> {
    let hash = try!(download_hash(url_str, cfg));
    let partial_hash: String = hash.chars().take(UPDATE_HASH_LEN).collect();

    if let Some(hash_file) = update_hash {
        if utils::is_file(hash_file) {
            if let Ok(contents) = utils::read_file("update hash", hash_file) {
                if contents == partial_hash {
                    // Skip download, update hash matches
                    return Ok(None);
                }
            } else {
                cfg.notify_handler.call(Notification::CantReadUpdateHash(hash_file));
            }
        } else {
            cfg.notify_handler.call(Notification::NoUpdateHash(hash_file));
        }
    }

    let url = try!(utils::parse_url(url_str));
    let file = try!(cfg.temp_cfg.new_file_with_ext("", ext));

    let mut hasher = Hasher::new(Type::SHA256);
    try!(utils::download_file(url, &file, Some(&mut hasher), ntfy!(&cfg.notify_handler)));
    let actual_hash = hasher.finish()
                            .iter()
                            .map(|b| format!("{:02x}", b))
                            .join("");

    if hash != actual_hash {
        // Incorrect hash
        return Err(Error::ChecksumFailed {
            url: url_str.to_owned(),
            expected: hash,
            calculated: actual_hash,
        });
    } else {
        cfg.notify_handler.call(Notification::ChecksumValid(url_str));
    }

    // TODO: Check the signature of the file

    Ok(Some((file, partial_hash)))
}

#[derive(Copy, Clone, Debug)]
pub struct DownloadCfg<'a> {
    pub dist_root: &'a str,
    pub temp_cfg: &'a temp::Cfg,
    pub notify_handler: NotifyHandler<'a>,
}

pub fn get_host_triple() -> (&'static str, &'static str, Option<&'static str>) {
    let arch = match env::consts::ARCH {
        "x86" => "i686", // Why, rust... WHY?
        other => other,
    };

    let os = match env::consts::OS {
        "windows" => "pc-windows",
        "linux" => "unknown-linux",
        "macos" => "apple-darwin",
        _ => unimplemented!()
    };

    let env = match () {
        () if cfg!(target_env = "gnu") => Some("gnu"),
        () if cfg!(target_env = "msvc") => Some("msvc"),
        _ => None,
    };

    (arch, os, env)
}

pub fn get_installer_ext() -> &'static str {
    if cfg!(windows) {
        return ".msi";
    }
    ".tar.gz"
}

pub fn download_hash(url: &str, cfg: DownloadCfg) -> Result<String> {
    let hash_url = try!(utils::parse_url(&(url.to_owned() + ".sha256")));
    let hash_file = try!(cfg.temp_cfg.new_file());

    try!(utils::download_file(hash_url, &hash_file, None, ntfy!(&cfg.notify_handler)));

    Ok(try!(utils::read_file("hash", &hash_file).map(|s| s[0..64].to_owned())))
}

// Installs or updates a toolchain from a dist server. If an initial
// install then it will be installed with the default components. If
// an upgrade then all the existing components will be upgraded.
//
// Returns the manifest's hash if anything changed.
pub fn update_from_dist<'a>(download: DownloadCfg<'a>,
                            update_hash: Option<&Path>,
                            toolchain: &str,
                            prefix: &InstallPrefix,
                            add: &[Component],
                            remove: &[Component],
                            ) -> Result<Option<String>> {

    let ref toolchain = try!(ToolchainDesc::from_str(toolchain));
    let trip = toolchain.target_triple();
    let manifestation = try!(Manifestation::open(prefix.clone(), &trip));

    let changes = Changes {
        add_extensions: add.to_owned(),
        remove_extensions: remove.to_owned(),
    };

    // TODO: Add a notification about which manifest version is going to be used
    download.notify_handler.call(Notification::DownloadingManifest);
    match dl_v2_manifest(download, update_hash, toolchain) {
        Ok(Some((m, hash))) => {
            return match try!(manifestation.update(&m, changes, &download.temp_cfg,
                                                   download.notify_handler.clone())) {
                UpdateStatus::Unchanged => Ok(None),
                UpdateStatus::Changed => Ok(Some(hash)),
            }
        }
        Ok(None) => return Ok(None),
        Err(Error::Utils(::multirust_utils::errors::Error::DownloadingFile {
            error: ::multirust_utils::raw::DownloadError::Status(hyper::status::StatusCode::NotFound),
            ..
        })) => {
            // Proceed to try v1 as a fallback
            download.notify_handler.call(Notification::DownloadingLegacyManifest);
        }
        Err(e) => return Err(e)
    }

    // If the v2 manifest is not found then try v1
    let manifest = try!(dl_v1_manifest(download, toolchain));
    match try!(manifestation.update_v1(&manifest, update_hash,
                                       &download.temp_cfg, download.notify_handler.clone())) {
        None => Ok(None),
        Some(hash) => Ok(Some(hash)),
    }
}

fn dl_v2_manifest<'a>(download: DownloadCfg<'a>,
                      update_hash: Option<&Path>,
                      toolchain: &ToolchainDesc) -> Result<Option<(ManifestV2, String)>> {
    let manifest_url = toolchain.manifest_v2_url(download.dist_root);
    let manifest_dl = try!(download_and_check(&manifest_url,
                                              update_hash, ".toml", download));
    let (manifest_file, manifest_hash) = if let Some(m) = manifest_dl { m } else { return Ok(None) };
    let manifest_str = try!(utils::read_file("manifest", &manifest_file));
    let manifest = try!(ManifestV2::parse(&manifest_str));

    Ok(Some((manifest, manifest_hash)))
}

fn dl_v1_manifest<'a>(download: DownloadCfg<'a>,
                      toolchain: &ToolchainDesc) -> Result<Vec<String>> {
    let root_url = toolchain.package_dir(download.dist_root);

    if !["nightly", "beta", "stable"].contains(&&*toolchain.channel) {
        // This is an explicit version. In v1 there was no manifest,
        // you just know the file to download, so synthesize one.
        let trip = toolchain.target_triple();
        let installer_name = format!("{}/rust-{}-{}.tar.gz",
                                     root_url, toolchain.channel, trip);
        return Ok(vec![installer_name]);
    }
    
    let manifest_url = toolchain.manifest_v1_url(download.dist_root);
    let manifest_dl = try!(download_and_check(&manifest_url, None, "", download));
    let (manifest_file, _) = manifest_dl.unwrap();
    let manifest_str = try!(utils::read_file("manifest", &manifest_file));
    let urls = manifest_str.lines().map(|s| format!("{}/{}", root_url, s)).collect();

    Ok(urls)
}
