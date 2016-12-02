
use temp;
use errors::*;
use notifications::*;
use rustup_utils::{self, utils};
use prefix::InstallPrefix;
use manifest::Component;
use manifest::Manifest as ManifestV2;
use manifestation::{Manifestation, UpdateStatus, Changes};

use std::path::Path;
use std::fmt;
use std::env;

use regex::Regex;
use sha2::{Sha256, Digest};

pub const DEFAULT_DIST_SERVER: &'static str = "https://static.rust-lang.org";
pub const UPDATE_HASH_LEN: usize = 20;

// Deprecated
pub const DEFAULT_DIST_ROOT: &'static str = "https://static.rust-lang.org/dist";

// A toolchain descriptor from rustup's perspective. These contain
// 'partial target triples', which allow toolchain names like
// 'stable-msvc' to work. Partial target triples though are parsed
// from a hardcoded set of known triples, whereas target triples
// are nearly-arbitrary strings.
#[derive(Debug, Clone)]
pub struct PartialToolchainDesc {
    // Either "nightly", "stable", "beta", or an explicit version number
    pub channel: String,
    pub date: Option<String>,
    pub target: PartialTargetTriple,
}

#[derive(Debug, Clone)]
pub struct PartialTargetTriple {
    pub arch: Option<String>,
    pub os: Option<String>,
    pub env: Option<String>,
}

// Fully-resolved toolchain descriptors. These always have full target
// triples attached to them and are used for canonical identification,
// such as naming their installation directory.
#[derive(Debug, Clone)]
pub struct ToolchainDesc {
    // Either "nightly", "stable", "beta", or an explicit version number
    pub channel: String,
    pub date: Option<String>,
    pub target: TargetTriple,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct TargetTriple(String);

// These lists contain the targets known to rustup, and used to build
// the PartialTargetTriple.

static LIST_ARCHS: &'static [&'static str] = &["i386",
                                               "i586",
                                               "i686",
                                               "x86_64",
                                               "arm",
                                               "armv7",
                                               "armv7s",
                                               "aarch64",
                                               "mips",
                                               "mipsel",
                                               "mips64",
                                               "mips64el",
                                               "powerpc",
                                               "powerpc64",
                                               "powerpc64le"];
static LIST_OSES: &'static [&'static str] = &["pc-windows",
                                              "unknown-linux",
                                              "apple-darwin",
                                              "unknown-netbsd",
                                              "apple-ios",
                                              "linux",
                                              "rumprun-netbsd",
                                              "unknown-freebsd"];
static LIST_ENVS: &'static [&'static str] =
    &["gnu", "msvc", "gnueabi", "gnueabihf", "gnuabi64", "androideabi", "musl"];

// MIPS platforms don't indicate endianness in uname, however binaries only
// run on boxes with the same endianness, as expected.
// Hence we could distinguish between the variants with compile-time cfg()
// attributes alone.
#[cfg(all(not(windows), target_endian = "big"))]
const TRIPLE_MIPS_UNKNOWN_LINUX_GNU: &'static str = "mips-unknown-linux-gnu";
#[cfg(all(not(windows), target_endian = "little"))]
const TRIPLE_MIPS_UNKNOWN_LINUX_GNU: &'static str = "mipsel-unknown-linux-gnu";

#[cfg(all(not(windows), target_endian = "big"))]
const TRIPLE_MIPS64_UNKNOWN_LINUX_GNUABI64: &'static str =
    "mips64-unknown-linux-gnuabi64";
#[cfg(all(not(windows), target_endian = "little"))]
const TRIPLE_MIPS64_UNKNOWN_LINUX_GNUABI64: &'static str =
    "mips64el-unknown-linux-gnuabi64";

impl TargetTriple {
    pub fn from_str(name: &str) -> Self {
        TargetTriple(name.to_string())
    }

    pub fn from_build() -> Self {
        if let Some(triple) = option_env!("RUSTUP_OVERRIDE_BUILD_TRIPLE") {
            TargetTriple::from_str(triple)
        } else {
            TargetTriple::from_str(include_str!(concat!(env!("OUT_DIR"), "/target.txt")))
        }
    }

    pub fn from_host() -> Option<Self> {
        #[cfg(windows)]
        fn inner() -> Option<TargetTriple> {
            use kernel32::GetNativeSystemInfo;
            use std::mem;

            // First detect architecture
            const PROCESSOR_ARCHITECTURE_AMD64: u16 = 9;
            const PROCESSOR_ARCHITECTURE_INTEL: u16 = 0;

            let mut sys_info;
            unsafe {
                sys_info = mem::zeroed();
                GetNativeSystemInfo(&mut sys_info);
            }

            let arch = match sys_info.wProcessorArchitecture {
                PROCESSOR_ARCHITECTURE_AMD64 => "x86_64",
                PROCESSOR_ARCHITECTURE_INTEL => "i686",
                _ => return None,
            };

            // Default to msvc
            let msvc_triple = format!("{}-pc-windows-msvc", arch);
            Some(TargetTriple(msvc_triple))
        }

        #[cfg(not(windows))]
        fn inner() -> Option<TargetTriple> {
            use libc;
            use std::mem;
            use std::ffi::CStr;

            let mut sys_info;
            let (sysname, machine) = unsafe {
                sys_info = mem::zeroed();
                if libc::uname(&mut sys_info) != 0 {
                    return None;
                }

                (CStr::from_ptr(sys_info.sysname.as_ptr()).to_bytes(),
                 CStr::from_ptr(sys_info.machine.as_ptr()).to_bytes())
            };

            let host_triple = match (sysname, machine) {
                (b"Linux", b"x86_64") => Some("x86_64-unknown-linux-gnu"),
                (b"Linux", b"i686") => Some("i686-unknown-linux-gnu"),
                (b"Linux", b"mips") => Some(TRIPLE_MIPS_UNKNOWN_LINUX_GNU),
                (b"Linux", b"mips64") => Some(TRIPLE_MIPS64_UNKNOWN_LINUX_GNUABI64),
                (b"Linux", b"arm") => Some("arm-unknown-linux-gnueabi"),
                (b"Linux", b"aarch64") => Some("aarch64-unknown-linux-gnu"),
                (b"Darwin", b"x86_64") => Some("x86_64-apple-darwin"),
                (b"Darwin", b"i686") => Some("i686-apple-darwin"),
                (b"FreeBSD", b"x86_64") => Some("x86_64-unknown-freebsd"),
                (b"FreeBSD", b"i686") => Some("i686-unknown-freebsd"),
                (b"OpenBSD", b"x86_64") => Some("x86_64-unknown-openbsd"),
                (b"OpenBSD", b"i686") => Some("i686-unknown-openbsd"),
                (b"NetBSD", b"x86_64") => Some("x86_64-unknown-netbsd"),
                (b"NetBSD", b"i686") => Some("i686-unknown-netbsd"),
                (b"DragonFly", b"x86_64") => Some("x86_64-unknown-dragonfly"),
                _ => None,
            };

            host_triple.map(TargetTriple::from_str)
        }

        if let Ok(triple) = env::var("RUSTUP_OVERRIDE_HOST_TRIPLE") {
            Some(TargetTriple(triple))
        } else {
            inner()
        }
    }

    pub fn from_host_or_build() -> Self {
        Self::from_host().unwrap_or_else(Self::from_build)
    }
}

impl PartialTargetTriple {
    pub fn from_str(name: &str) -> Option<Self> {
        if name.is_empty() {
            return Some(PartialTargetTriple {
                arch: None,
                os: None,
                env: None,
            });
        }

        // Prepending `-` makes this next regex easier since
        // we can count  on all triple components being
        // delineated by it.
        let name = format!("-{}", name);
        let pattern = format!(r"^(?:-({}))?(?:-({}))?(?:-({}))?$",
                              LIST_ARCHS.join("|"),
                              LIST_OSES.join("|"),
                              LIST_ENVS.join("|"));

        let re = Regex::new(&pattern).unwrap();
        re.captures(&name).map(|c| {
            fn fn_map(s: &str) -> Option<String> {
                if s == "" {
                    None
                } else {
                    Some(s.to_owned())
                }
            }

            PartialTargetTriple {
                arch: c.at(1).and_then(fn_map),
                os: c.at(2).and_then(fn_map),
                env: c.at(3).and_then(fn_map),
            }
        })
    }
}

impl PartialToolchainDesc {
    pub fn from_str(name: &str) -> Result<Self> {
        let channels =
            ["nightly", "beta", "stable", r"\d{1}\.\d{1}\.\d{1}", r"\d{1}\.\d{2}\.\d{1}"];

        let pattern = format!(r"^({})(?:-(\d{{4}}-\d{{2}}-\d{{2}}))?(?:-(.*))?$",
                              channels.join("|"));


        let re = Regex::new(&pattern).unwrap();
        let d = re.captures(name).map(|c| {
            fn fn_map(s: &str) -> Option<String> {
                if s == "" {
                    None
                } else {
                    Some(s.to_owned())
                }
            }

            let trip = c.at(3).unwrap_or("");
            let trip = PartialTargetTriple::from_str(&trip);
            trip.map(|t| {
                PartialToolchainDesc {
                    channel: c.at(1).unwrap().to_owned(),
                    date: c.at(2).and_then(fn_map),
                    target: t,
                }
            })
        });

        if let Some(Some(d)) = d {
            Ok(d)
        } else {
            Err(ErrorKind::InvalidToolchainName(name.to_string()).into())
        }
    }

    pub fn resolve(self, host: &TargetTriple) -> ToolchainDesc {
        let host = PartialTargetTriple::from_str(&host.0)
            .expect("host triple couldn't be converted to partial triple");
        let host_arch = host.arch.expect("");
        let host_os = host.os.expect("");
        let host_env = host.env;

        // If OS was specified, don't default to host environment, even if the OS matches
        // the host OS, otherwise cannot specify no environment.
        let env = if self.target.os.is_some() {
            self.target.env
        } else {
            self.target.env.or_else(|| host_env)
        };
        let arch = self.target.arch.unwrap_or_else(|| host_arch);
        let os = self.target.os.unwrap_or_else(|| host_os);

        let trip = if let Some(env) = env {
            format!("{}-{}-{}", arch, os, env)
        } else {
            format!("{}-{}", arch, os)
        };

        ToolchainDesc {
            channel: self.channel,
            date: self.date,
            target: TargetTriple(trip),
        }
    }
}

impl ToolchainDesc {
    pub fn from_str(name: &str) -> Result<Self> {
        let channels =
            ["nightly", "beta", "stable", r"\d{1}\.\d{1}\.\d{1}", r"\d{1}\.\d{2}\.\d{1}"];

        let pattern = format!(
            r"^({})(?:-(\d{{4}}-\d{{2}}-\d{{2}}))?-(.*)?$",
            channels.join("|"),
            );

        let re = Regex::new(&pattern).unwrap();
        re.captures(name)
            .map(|c| {
                fn fn_map(s: &str) -> Option<String> {
                    if s == "" {
                        None
                    } else {
                        Some(s.to_owned())
                    }
                }

                ToolchainDesc {
                    channel: c.at(1).unwrap().to_owned(),
                    date: c.at(2).and_then(fn_map),
                    target: TargetTriple(c.at(3).unwrap().to_owned()),
                }
            })
            .ok_or(ErrorKind::InvalidToolchainName(name.to_string()).into())
    }

    pub fn manifest_v1_url(&self, dist_root: &str) -> String {
        let do_manifest_staging = env::var("RUSTUP_STAGED_MANIFEST").is_ok();
        match (self.date.as_ref(), do_manifest_staging) {
            (None, false) => format!("{}/channel-rust-{}", dist_root, self.channel),
            (Some(date), false) => format!("{}/{}/channel-rust-{}", dist_root, date, self.channel),
            (None, true) => format!("{}/staging/channel-rust-{}", dist_root, self.channel),
            (Some(_), true) => panic!("not a real-world case"),
        }
    }

    pub fn manifest_v2_url(&self, dist_root: &str) -> String {
        format!("{}.toml", self.manifest_v1_url(dist_root))
    }
    /// Either "$channel" or "channel-$date"
    pub fn manifest_name(&self) -> String {
        match self.date {
            None => self.channel.clone(),
            Some(ref date) => format!("{}-{}", self.channel, date),
        }
    }

    pub fn package_dir(&self, dist_root: &str) -> String {
        match self.date {
            None => format!("{}", dist_root),
            Some(ref date) => format!("{}/{}", dist_root, date),
        }
    }

    pub fn full_spec(&self) -> String {
        if self.date.is_some() {
            format!("{}", self)
        } else {
            format!("{} (tracking)", self)
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

impl fmt::Display for TargetTriple {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for PartialToolchainDesc {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "{}", &self.channel));

        if let Some(ref date) = self.date {
            try!(write!(f, "-{}", date));
        }
        if let Some(ref arch) = self.target.arch {
            try!(write!(f, "-{}", arch));
        }
        if let Some(ref os) = self.target.os {
            try!(write!(f, "-{}", os));
        }
        if let Some(ref env) = self.target.env {
            try!(write!(f, "-{}", env));
        }

        Ok(())
    }
}

impl fmt::Display for ToolchainDesc {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "{}", &self.channel));

        if let Some(ref date) = self.date {
            try!(write!(f, "-{}", date));
        }
        try!(write!(f, "-{}", self.target));

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
                (cfg.notify_handler)(Notification::CantReadUpdateHash(hash_file));
            }
        } else {
            (cfg.notify_handler)(Notification::NoUpdateHash(hash_file));
        }
    }

    let url = try!(utils::parse_url(url_str));
    let file = try!(cfg.temp_cfg.new_file_with_ext("", ext));

    let mut hasher = Sha256::new();
    try!(utils::download_file(&url,
                              &file,
                              Some(&mut hasher),
                              &|n| (cfg.notify_handler)(n.into())));
    let actual_hash = hasher.result_str();

    if hash != actual_hash {
        // Incorrect hash
        return Err(ErrorKind::ChecksumFailed {
                url: url_str.to_owned(),
                expected: hash,
                calculated: actual_hash,
            }
            .into());
    } else {
        (cfg.notify_handler)(Notification::ChecksumValid(url_str));
    }

    // TODO: Check the signature of the file

    Ok(Some((file, partial_hash)))
}

#[derive(Copy, Clone)]
pub struct DownloadCfg<'a> {
    pub dist_root: &'a str,
    pub temp_cfg: &'a temp::Cfg,
    pub notify_handler: &'a Fn(Notification),
}

pub fn download_hash(url: &str, cfg: DownloadCfg) -> Result<String> {
    let hash_url = try!(utils::parse_url(&(url.to_owned() + ".sha256")));
    let hash_file = try!(cfg.temp_cfg.new_file());

    try!(utils::download_file(&hash_url,
                              &hash_file,
                              None,
                              &|n| (cfg.notify_handler)(n.into())));

    Ok(try!(utils::read_file("hash", &hash_file).map(|s| s[0..64].to_owned())))
}

// Installs or updates a toolchain from a dist server. If an initial
// install then it will be installed with the default components. If
// an upgrade then all the existing components will be upgraded.
//
// Returns the manifest's hash if anything changed.
pub fn update_from_dist<'a>(download: DownloadCfg<'a>,
                            update_hash: Option<&Path>,
                            toolchain: &ToolchainDesc,
                            prefix: &InstallPrefix,
                            add: &[Component],
                            remove: &[Component])
                            -> Result<Option<String>> {

    let fresh_install = !prefix.path().exists();

    let res = update_from_dist_(download,
                                update_hash,
                                toolchain,
                                prefix,
                                add,
                                remove);

    // Don't leave behind an empty / broken installation directory
    if res.is_err() && fresh_install {
        // FIXME Ignoring cascading errors
        let _ = utils::remove_dir("toolchain", prefix.path(),
                                  &|n| (download.notify_handler)(n.into()));
    }

    res
}

pub fn update_from_dist_<'a>(download: DownloadCfg<'a>,
                            update_hash: Option<&Path>,
                            toolchain: &ToolchainDesc,
                            prefix: &InstallPrefix,
                            add: &[Component],
                            remove: &[Component])
                            -> Result<Option<String>> {

    let toolchain_str = toolchain.to_string();
    let manifestation = try!(Manifestation::open(prefix.clone(), toolchain.target.clone()));

    let changes = Changes {
        add_extensions: add.to_owned(),
        remove_extensions: remove.to_owned(),
    };

    // TODO: Add a notification about which manifest version is going to be used
    (download.notify_handler)(Notification::DownloadingManifest(&toolchain_str));
    match dl_v2_manifest(download, update_hash, toolchain) {
        Ok(Some((m, hash))) => {
            return match try!(manifestation.update(&m,
                                                   changes,
                                                   &download.temp_cfg,
                                                   download.notify_handler.clone())) {
                UpdateStatus::Unchanged => Ok(None),
                UpdateStatus::Changed => Ok(Some(hash)),
            }
        }
        Ok(None) => return Ok(None),
        Err(Error(ErrorKind::Utils(::rustup_utils::ErrorKind::DownloadNotExists { .. }), _)) => {
            // Proceed to try v1 as a fallback
            (download.notify_handler)(Notification::DownloadingLegacyManifest);
        }
        Err(e) => return Err(e),
    }

    // If the v2 manifest is not found then try v1
    let manifest = match dl_v1_manifest(download, toolchain) {
        Ok(m) => m,
        Err(Error(ErrorKind::Utils(rustup_utils::ErrorKind::DownloadNotExists { .. }), _)) => {
            return Err(format!("no release found for '{}'", toolchain.manifest_name()).into());
        }
        Err(e @ Error(ErrorKind::ChecksumFailed { .. }, _)) => {
            return Err(e);
        }
        Err(e) => {
            return Err(e).chain_err(|| {
                format!("failed to download manifest for '{}'",
                        toolchain.manifest_name())
            });
        }
    };
    match manifestation.update_v1(&manifest,
                                  update_hash,
                                  &download.temp_cfg,
                                  download.notify_handler.clone()) {
        Ok(None) => Ok(None),
        Ok(Some(hash)) => Ok(Some(hash)),
        e @ Err(Error(ErrorKind::Utils(rustup_utils::ErrorKind::DownloadNotExists { .. }), _)) => {
            e.chain_err(|| {
                format!("could not download nonexistent rust version `{}`",
                        toolchain_str)
            })
        }
        Err(e) => Err(e),
    }
}

fn dl_v2_manifest<'a>(download: DownloadCfg<'a>,
                      update_hash: Option<&Path>,
                      toolchain: &ToolchainDesc)
                      -> Result<Option<(ManifestV2, String)>> {
    let manifest_url = toolchain.manifest_v2_url(download.dist_root);
    let manifest_dl_res = download_and_check(&manifest_url, update_hash, ".toml", download);

    if let Ok(manifest_dl) = manifest_dl_res {
        // Downloaded ok!
        let (manifest_file, manifest_hash) = if let Some(m) = manifest_dl {
            m
        } else {
            return Ok(None);
        };
        let manifest_str = try!(utils::read_file("manifest", &manifest_file));
        let manifest = try!(ManifestV2::parse(&manifest_str));

        Ok(Some((manifest, manifest_hash)))
    } else {
        match *manifest_dl_res.as_ref().unwrap_err().kind() {
            // Checksum failed - issue warning to try again later
            ErrorKind::ChecksumFailed { .. } => {
                (download.notify_handler)(Notification::ManifestChecksumFailedHack)
            }
            _ => {}
        }
        Err(manifest_dl_res.unwrap_err())
    }

}

fn dl_v1_manifest<'a>(download: DownloadCfg<'a>, toolchain: &ToolchainDesc) -> Result<Vec<String>> {
    let root_url = toolchain.package_dir(download.dist_root);

    if !["nightly", "beta", "stable"].contains(&&*toolchain.channel) {
        // This is an explicit version. In v1 there was no manifest,
        // you just know the file to download, so synthesize one.
        let installer_name = format!("{}/rust-{}-{}.tar.gz",
                                     root_url,
                                     toolchain.channel,
                                     toolchain.target);
        return Ok(vec![installer_name]);
    }

    let manifest_url = toolchain.manifest_v1_url(download.dist_root);
    let manifest_dl = try!(download_and_check(&manifest_url, None, "", download));
    let (manifest_file, _) = manifest_dl.unwrap();
    let manifest_str = try!(utils::read_file("manifest", &manifest_file));
    let urls = manifest_str.lines().map(|s| format!("{}/{}", root_url, s)).collect();

    Ok(urls)
}
