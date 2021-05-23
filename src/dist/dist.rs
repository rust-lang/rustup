use std::collections::HashSet;
use std::env;
use std::fmt;
use std::io::Write;
use std::ops::Deref;
use std::path::Path;
use std::str::FromStr;

use anyhow::{anyhow, bail, Context, Result};
use chrono::{Date, NaiveDate, TimeZone, Utc};
use lazy_static::lazy_static;
use regex::Regex;
use thiserror::Error as ThisError;

use crate::dist::download::DownloadCfg;
use crate::dist::manifest::{Component, Manifest as ManifestV2};
use crate::dist::manifestation::{Changes, Manifestation, UpdateStatus};
use crate::dist::notifications::*;
use crate::dist::prefix::InstallPrefix;
use crate::dist::temp;
pub use crate::dist::triple::*;
use crate::errors::RustupError;
use crate::process;
use crate::utils::utils;

pub static DEFAULT_DIST_SERVER: &str = "https://static.rust-lang.org";

// Deprecated
pub static DEFAULT_DIST_ROOT: &str = "https://static.rust-lang.org/dist";

// The channel patterns we support
static TOOLCHAIN_CHANNELS: &[&str] = &[
    "nightly",
    "beta",
    "stable",
    // Allow from 1.0.0 through to 9.999.99 with optional patch version
    r"\d{1}\.\d{1,3}(?:\.\d{1,2})?",
];

fn components_missing_msg(cs: &[Component], manifest: &ManifestV2, toolchain: &str) -> String {
    assert!(!cs.is_empty());
    let mut buf = vec![];
    let suggestion = format!("    rustup toolchain add {} --profile minimal", toolchain);
    let nightly_tips = "Sometimes not all components are available in any given nightly. ";

    if cs.len() == 1 {
        let _ = writeln!(
            buf,
            "component {} is unavailable for download for channel '{}'",
            &cs[0].description(manifest),
            toolchain,
        );

        if toolchain.starts_with("nightly") {
            let _ = write!(buf, "{}", nightly_tips.to_string());
        }

        let _ = write!(
            buf,
            "If you don't need the component, you could try a minimal installation with:\n\n{}",
            suggestion
        );
    } else {
        let cs_str = cs
            .iter()
            .map(|c| c.description(manifest))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = write!(
            buf,
            "some components unavailable for download for channel '{}': {}",
            toolchain, cs_str
        );

        if toolchain.starts_with("nightly") {
            let _ = write!(buf, "{}", nightly_tips.to_string());
        }
        let _ = write!(
            buf,
            "If you don't need the components, you could try a minimal installation with:\n\n{}",
            suggestion
        );
    }

    String::from_utf8(buf).unwrap()
}

#[derive(Debug, ThisError)]
enum DistError {
    #[error("{}", components_missing_msg(&.0, &.1, &.2))]
    ToolchainComponentsMissing(Vec<Component>, ManifestV2, String),
    #[error("no release found for '{0}'")]
    MissingReleaseForToolchain(String),
}

#[derive(Debug, PartialEq)]
struct ParsedToolchainDesc {
    channel: String,
    date: Option<String>,
    target: Option<String>,
}

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

// Linux hosts don't indicate clib in uname, however binaries only
// run on boxes with the same clib, as expected.
#[cfg(all(not(windows), not(target_env = "musl")))]
const TRIPLE_X86_64_UNKNOWN_LINUX: &str = "x86_64-unknown-linux-gnu";
#[cfg(all(not(windows), target_env = "musl"))]
const TRIPLE_X86_64_UNKNOWN_LINUX: &str = "x86_64-unknown-linux-musl";
#[cfg(all(not(windows), not(target_env = "musl")))]
const TRIPLE_AARCH64_UNKNOWN_LINUX: &str = "aarch64-unknown-linux-gnu";
#[cfg(all(not(windows), target_env = "musl"))]
const TRIPLE_AARCH64_UNKNOWN_LINUX: &str = "aarch64-unknown-linux-musl";

// MIPS platforms don't indicate endianness in uname, however binaries only
// run on boxes with the same endianness, as expected.
// Hence we could distinguish between the variants with compile-time cfg()
// attributes alone.
#[cfg(all(not(windows), target_endian = "big"))]
static TRIPLE_MIPS_UNKNOWN_LINUX_GNU: &str = "mips-unknown-linux-gnu";
#[cfg(all(not(windows), target_endian = "little"))]
static TRIPLE_MIPS_UNKNOWN_LINUX_GNU: &str = "mipsel-unknown-linux-gnu";

#[cfg(all(not(windows), target_endian = "big"))]
static TRIPLE_MIPS64_UNKNOWN_LINUX_GNUABI64: &str = "mips64-unknown-linux-gnuabi64";
#[cfg(all(not(windows), target_endian = "little"))]
static TRIPLE_MIPS64_UNKNOWN_LINUX_GNUABI64: &str = "mips64el-unknown-linux-gnuabi64";

impl FromStr for ParsedToolchainDesc {
    type Err = anyhow::Error;
    fn from_str(desc: &str) -> Result<Self> {
        lazy_static! {
            static ref TOOLCHAIN_CHANNEL_PATTERN: String = format!(
                r"^({})(?:-(\d{{4}}-\d{{2}}-\d{{2}}))?(?:-(.+))?$",
                TOOLCHAIN_CHANNELS.join("|")
            );
            // Note this regex gives you a guaranteed match of the channel (1)
            // and an optional match of the date (2) and target (3)
            static ref TOOLCHAIN_CHANNEL_RE: Regex = Regex::new(&TOOLCHAIN_CHANNEL_PATTERN).unwrap();
        }

        let d = TOOLCHAIN_CHANNEL_RE.captures(desc).map(|c| {
            fn fn_map(s: &str) -> Option<String> {
                if s.is_empty() {
                    None
                } else {
                    Some(s.to_owned())
                }
            }

            // These versions don't have v2 manifests, but they don't have point releases either,
            // so to make the two-part version numbers work for these versions, specially turn
            // them into their corresponding ".0" version.
            let channel = match c.get(1).unwrap().as_str() {
                "1.0" => "1.0.0",
                "1.1" => "1.1.0",
                "1.2" => "1.2.0",
                "1.3" => "1.3.0",
                "1.4" => "1.4.0",
                "1.5" => "1.5.0",
                "1.6" => "1.6.0",
                "1.7" => "1.7.0",
                "1.8" => "1.8.0",
                other => other,
            };

            Self {
                channel: channel.to_owned(),
                date: c.get(2).map(|s| s.as_str()).and_then(fn_map),
                target: c.get(3).map(|s| s.as_str()).and_then(fn_map),
            }
        });

        if let Some(d) = d {
            Ok(d)
        } else {
            Err(RustupError::InvalidToolchainName(desc.to_string()).into())
        }
    }
}

impl Deref for TargetTriple {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TargetTriple {
    pub fn new(name: &str) -> Self {
        Self(name.to_string())
    }

    pub fn from_build() -> Self {
        if let Some(triple) = option_env!("RUSTUP_OVERRIDE_BUILD_TRIPLE") {
            Self::new(triple)
        } else {
            Self::new(env!("TARGET"))
        }
    }

    pub fn from_host() -> Option<Self> {
        #[cfg(windows)]
        fn inner() -> Option<TargetTriple> {
            use std::mem;
            use winapi::um::sysinfoapi::GetNativeSystemInfo;

            // First detect architecture
            const PROCESSOR_ARCHITECTURE_AMD64: u16 = 9;
            const PROCESSOR_ARCHITECTURE_INTEL: u16 = 0;

            let mut sys_info;
            unsafe {
                sys_info = mem::zeroed();
                GetNativeSystemInfo(&mut sys_info);
            }

            let arch = match unsafe { sys_info.u.s() }.wProcessorArchitecture {
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
            use std::ffi::CStr;
            use std::mem;

            let mut sys_info;
            let (sysname, machine) = unsafe {
                sys_info = mem::zeroed();
                if libc::uname(&mut sys_info) != 0 {
                    return None;
                }

                (
                    CStr::from_ptr(sys_info.sysname.as_ptr()).to_bytes(),
                    CStr::from_ptr(sys_info.machine.as_ptr()).to_bytes(),
                )
            };

            let host_triple = match (sysname, machine) {
                (_, b"arm") if cfg!(target_os = "android") => Some("arm-linux-androideabi"),
                (_, b"armv7l") if cfg!(target_os = "android") => Some("armv7-linux-androideabi"),
                (_, b"armv8l") if cfg!(target_os = "android") => Some("armv7-linux-androideabi"),
                (_, b"aarch64") if cfg!(target_os = "android") => Some("aarch64-linux-android"),
                (_, b"i686") if cfg!(target_os = "android") => Some("i686-linux-android"),
                (_, b"x86_64") if cfg!(target_os = "android") => Some("x86_64-linux-android"),
                (b"Linux", b"x86_64") => Some(TRIPLE_X86_64_UNKNOWN_LINUX),
                (b"Linux", b"i686") => Some("i686-unknown-linux-gnu"),
                (b"Linux", b"mips") => Some(TRIPLE_MIPS_UNKNOWN_LINUX_GNU),
                (b"Linux", b"mips64") => Some(TRIPLE_MIPS64_UNKNOWN_LINUX_GNUABI64),
                (b"Linux", b"arm") => Some("arm-unknown-linux-gnueabi"),
                (b"Linux", b"armv7l") => Some("armv7-unknown-linux-gnueabihf"),
                (b"Linux", b"armv8l") => Some("armv7-unknown-linux-gnueabihf"),
                (b"Linux", b"aarch64") => Some(TRIPLE_AARCH64_UNKNOWN_LINUX),
                (b"Darwin", b"x86_64") => Some("x86_64-apple-darwin"),
                (b"Darwin", b"i686") => Some("i686-apple-darwin"),
                (b"FreeBSD", b"x86_64") => Some("x86_64-unknown-freebsd"),
                (b"FreeBSD", b"i686") => Some("i686-unknown-freebsd"),
                (b"OpenBSD", b"x86_64") => Some("x86_64-unknown-openbsd"),
                (b"OpenBSD", b"i686") => Some("i686-unknown-openbsd"),
                (b"NetBSD", b"x86_64") => Some("x86_64-unknown-netbsd"),
                (b"NetBSD", b"i686") => Some("i686-unknown-netbsd"),
                (b"DragonFly", b"x86_64") => Some("x86_64-unknown-dragonfly"),
                (b"SunOS", b"i86pc") => Some("x86_64-unknown-illumos"),
                _ => None,
            };

            host_triple.map(TargetTriple::new)
        }

        if let Ok(triple) = process().var("RUSTUP_OVERRIDE_HOST_TRIPLE") {
            Some(Self(triple))
        } else {
            inner()
        }
    }

    pub fn from_host_or_build() -> Self {
        Self::from_host().unwrap_or_else(Self::from_build)
    }

    pub fn can_run(&self, other: &TargetTriple) -> Result<bool> {
        // Most trivial shortcut of all
        if self == other {
            return Ok(true);
        }
        // Otherwise we need to parse things
        let partial_self = PartialTargetTriple::new(&self.0)
            .ok_or_else(|| anyhow!(format!("Unable to parse target triple: {}", self.0)))?;
        let partial_other = PartialTargetTriple::new(&other.0)
            .ok_or_else(|| anyhow!(format!("Unable to parse target triple: {}", other.0)))?;
        // First obvious check is OS, if that doesn't match there's no chance
        let ret = if partial_self.os != partial_other.os {
            false
        } else if partial_self.os.as_deref() == Some("pc-windows") {
            // Windows is a special case here, we know we can run 32bit on 64bit
            // and we know we can run gnu and msvc on the same system
            // We don't immediately assume we can cross between x86 and aarch64 though
            (partial_self.arch == partial_other.arch)
                || (partial_self.arch.as_deref() == Some("x86_64")
                    && partial_other.arch.as_deref() == Some("i686"))
        } else {
            // For other OSes, for now, we assume other toolchains won't run
            false
        };
        Ok(ret)
    }
}

impl std::convert::TryFrom<PartialTargetTriple> for TargetTriple {
    type Error = &'static str;
    fn try_from(value: PartialTargetTriple) -> std::result::Result<Self, Self::Error> {
        if value.arch.is_some() && value.os.is_some() && value.env.is_some() {
            Ok(Self(format!(
                "{}-{}-{}",
                value.arch.unwrap(),
                value.os.unwrap(),
                value.env.unwrap()
            )))
        } else {
            Err("Incomplete / bad target triple")
        }
    }
}

impl FromStr for PartialToolchainDesc {
    type Err = anyhow::Error;
    fn from_str(name: &str) -> Result<Self> {
        let parsed: ParsedToolchainDesc = name.parse()?;
        let target = PartialTargetTriple::new(parsed.target.as_deref().unwrap_or(""));

        target
            .map(|target| Self {
                channel: parsed.channel,
                date: parsed.date,
                target,
            })
            .ok_or_else(|| anyhow!(RustupError::InvalidToolchainName(name.to_string())))
    }
}

impl PartialToolchainDesc {
    pub fn resolve(self, input_host: &TargetTriple) -> Result<ToolchainDesc> {
        let host = PartialTargetTriple::new(&input_host.0).ok_or_else(|| {
            anyhow!(format!(
                "Provided host '{}' couldn't be converted to partial triple",
                input_host.0
            ))
        })?;
        let host_arch = host.arch.ok_or_else(|| {
            anyhow!(format!(
                "Provided host '{}' did not specify a CPU architecture",
                input_host.0
            ))
        })?;
        let host_os = host.os.ok_or_else(|| {
            anyhow!(format!(
                "Provided host '{}' did not specify an operating system",
                input_host.0
            ))
        })?;
        let host_env = host.env;

        // If OS was specified, don't default to host environment, even if the OS matches
        // the host OS, otherwise cannot specify no environment.
        let env = if self.target.os.is_some() {
            self.target.env
        } else {
            self.target.env.or(host_env)
        };
        let arch = self.target.arch.unwrap_or(host_arch);
        let os = self.target.os.unwrap_or(host_os);

        let trip = if let Some(env) = env {
            format!("{}-{}-{}", arch, os, env)
        } else {
            format!("{}-{}", arch, os)
        };

        Ok(ToolchainDesc {
            channel: self.channel,
            date: self.date,
            target: TargetTriple(trip),
        })
    }

    pub fn has_triple(&self) -> bool {
        self.target.arch.is_some() || self.target.os.is_some() || self.target.env.is_some()
    }
}

impl FromStr for ToolchainDesc {
    type Err = anyhow::Error;
    fn from_str(name: &str) -> Result<Self> {
        let parsed: ParsedToolchainDesc = name.parse()?;

        if parsed.target.is_none() {
            return Err(anyhow!(RustupError::InvalidToolchainName(name.to_string())));
        }

        Ok(Self {
            channel: parsed.channel,
            date: parsed.date,
            target: TargetTriple(parsed.target.unwrap()),
        })
    }
}

impl ToolchainDesc {
    pub fn manifest_v1_url(&self, dist_root: &str) -> String {
        let do_manifest_staging = process().var("RUSTUP_STAGED_MANIFEST").is_ok();
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
            None => dist_root.to_string(),
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

    /// Toolchain channels are considered 'tracking' if it is one of the named channels
    /// such as `stable`, or is an incomplete version such as `1.48`, and the
    /// date field is empty.
    pub fn is_tracking(&self) -> bool {
        let channels = ["nightly", "beta", "stable"];
        lazy_static! {
            static ref TRACKING_VERSION: Regex = Regex::new(r"^\d{1}\.\d{1,3}$").unwrap();
        }
        (channels.iter().any(|x| *x == self.channel) || TRACKING_VERSION.is_match(&self.channel))
            && self.date.is_none()
    }
}

// A little convenience for just parsing a channel name or archived channel name
pub fn validate_channel_name(name: &str) -> Result<()> {
    let toolchain = PartialToolchainDesc::from_str(&name)?;
    if toolchain.has_triple() {
        Err(anyhow!(format!("target triple in channel name '{}'", name)))
    } else {
        Ok(())
    }
}

#[derive(Debug)]
pub struct Manifest<'a>(temp::File<'a>, String);

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum Profile {
    Minimal,
    Default,
    Complete,
}

impl FromStr for Profile {
    type Err = anyhow::Error;

    fn from_str(name: &str) -> Result<Self> {
        match name {
            "minimal" | "m" => Ok(Self::Minimal),
            "default" | "d" | "" => Ok(Self::Default),
            "complete" | "c" => Ok(Self::Complete),
            _ => Err(anyhow!(format!(
                "invalid profile name: '{}'; valid names are: {}",
                name,
                valid_profile_names()
            ))),
        }
    }
}

impl Profile {
    pub fn names() -> &'static [&'static str] {
        &["minimal", "default", "complete"]
    }

    pub fn default_name() -> &'static str {
        "default"
    }
}

impl Default for Profile {
    fn default() -> Self {
        Self::Default
    }
}

impl fmt::Display for TargetTriple {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for PartialToolchainDesc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &self.channel)?;

        if let Some(ref date) = self.date {
            write!(f, "-{}", date)?;
        }
        if let Some(ref arch) = self.target.arch {
            write!(f, "-{}", arch)?;
        }
        if let Some(ref os) = self.target.os {
            write!(f, "-{}", os)?;
        }
        if let Some(ref env) = self.target.env {
            write!(f, "-{}", env)?;
        }

        Ok(())
    }
}

impl fmt::Display for ToolchainDesc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &self.channel)?;

        if let Some(ref date) = self.date {
            write!(f, "-{}", date)?;
        }
        write!(f, "-{}", self.target)?;

        Ok(())
    }
}

impl fmt::Display for Profile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Minimal => write!(f, "minimal"),
            Self::Default => write!(f, "default"),
            Self::Complete => write!(f, "complete"),
        }
    }
}

pub fn valid_profile_names() -> String {
    Profile::names()
        .iter()
        .map(|s| format!("'{}'", s))
        .collect::<Vec<_>>()
        .join(", ")
}

// Installs or updates a toolchain from a dist server. If an initial
// install then it will be installed with the default components. If
// an upgrade then all the existing components will be upgraded.
//
// Returns the manifest's hash if anything changed.
pub fn update_from_dist<'a>(
    download: DownloadCfg<'a>,
    update_hash: Option<&Path>,
    toolchain: &ToolchainDesc,
    profile: Option<Profile>,
    prefix: &InstallPrefix,
    force_update: bool,
    allow_downgrade: bool,
    old_date: Option<&str>,
    components: &[&str],
    targets: &[&str],
) -> Result<Option<String>> {
    let fresh_install = !prefix.path().exists();
    let hash_exists = update_hash.map(Path::exists).unwrap_or(false);

    // fresh_install means the toolchain isn't present, but hash_exists means there is a stray hash file
    if fresh_install && hash_exists {
        // It's ok to unwrap, because hash have to exist at this point
        (download.notify_handler)(Notification::StrayHash(update_hash.unwrap()));
        std::fs::remove_file(update_hash.unwrap())?;
    }

    let res = update_from_dist_(
        download,
        update_hash,
        toolchain,
        profile,
        prefix,
        force_update,
        allow_downgrade,
        old_date,
        components,
        targets,
    );

    // Don't leave behind an empty / broken installation directory
    if res.is_err() && fresh_install {
        // FIXME Ignoring cascading errors
        let _ = utils::remove_dir("toolchain", prefix.path(), download.notify_handler);
    }

    res
}

fn update_from_dist_<'a>(
    download: DownloadCfg<'a>,
    update_hash: Option<&Path>,
    toolchain: &ToolchainDesc,
    profile: Option<Profile>,
    prefix: &InstallPrefix,
    force_update: bool,
    allow_downgrade: bool,
    old_date: Option<&str>,
    components: &[&str],
    targets: &[&str],
) -> Result<Option<String>> {
    let mut toolchain = toolchain.clone();
    let mut fetched = String::new();
    let mut first_err = None;
    let backtrack = toolchain.channel == "nightly" && toolchain.date.is_none();
    // We want to limit backtracking if we do not already have a toolchain
    let mut backtrack_limit: Option<i32> = if toolchain.date.is_some() {
        None
    } else {
        // We limit the backtracking to 21 days by default (half a release cycle).
        // The limit of 21 days is an arbitrary selection, so we let the user override it.
        const BACKTRACK_LIMIT_DEFAULT: i32 = 21;
        let provided = process()
            .var("RUSTUP_BACKTRACK_LIMIT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(BACKTRACK_LIMIT_DEFAULT);
        Some(if provided < 1 { 1 } else { provided })
    };

    // In case there is no allow-downgrade option set
    // we never want to backtrack further back than the nightly that's already installed.
    //
    // If no nightly is installed, it makes no sense to backtrack beyond the first ever manifest,
    // which is 2014-12-20 according to
    // https://static.rust-lang.org/cargo-dist/index.html.
    //
    // We could arguably use the date of the first rustup release here, but that would break a
    // bunch of the tests, which (inexplicably) use 2015-01-01 as their manifest dates.
    let first_manifest = Utc.from_utc_date(&NaiveDate::from_ymd(2014, 12, 20));
    let old_manifest = old_date
        .and_then(|date| utc_from_manifest_date(date))
        .unwrap_or(first_manifest);
    let last_manifest = if allow_downgrade {
        first_manifest
    } else {
        old_manifest
    };

    let current_manifest = {
        let manifestation = Manifestation::open(prefix.clone(), toolchain.target.clone())?;
        manifestation.load_manifest()?
    };

    loop {
        match try_update_from_dist_(
            download,
            update_hash,
            &toolchain,
            profile,
            prefix,
            force_update,
            components,
            targets,
            &mut fetched,
        ) {
            Ok(v) => break Ok(v),
            Err(e) => {
                if !backtrack {
                    break Err(e);
                }

                let cause = e.downcast_ref::<DistError>();
                match cause {
                    Some(DistError::ToolchainComponentsMissing(components, manifest, ..)) => {
                        (download.notify_handler)(Notification::SkippingNightlyMissingComponent(
                            &toolchain,
                            current_manifest.as_ref().unwrap_or(manifest),
                            components,
                        ));

                        if first_err.is_none() {
                            first_err = Some(e);
                        }
                        // We decrement the backtrack count only on unavailable component errors
                        // so that the limit only applies to nightlies that were indeed available,
                        // and ignores missing ones.
                        backtrack_limit = backtrack_limit.map(|n| n - 1);
                    }

                    Some(DistError::MissingReleaseForToolchain(..)) => {
                        // no need to even print anything for missing nightlies,
                        // since we don't really "skip" them
                    }
                    None => {
                        // All other errors break the loop
                        break Err(e);
                    }
                };

                if let Some(backtrack_limit) = backtrack_limit {
                    if backtrack_limit < 1 {
                        // This unwrap is safe because we can only hit this if we've
                        // had a chance to set first_err
                        break Err(first_err.unwrap());
                    }
                }

                // The user asked to update their nightly, but the latest nightly does not have all
                // the components that the user currently has installed. Let's try the previous
                // nightlies in reverse chronological order until we find a nightly that does,
                // starting at one date earlier than the current manifest's date.
                let toolchain_date = toolchain.date.as_ref().unwrap_or(&fetched);
                let try_next = utc_from_manifest_date(toolchain_date)
                    .unwrap_or_else(|| panic!("Malformed manifest date: {:?}", toolchain_date))
                    .pred();

                if try_next < last_manifest {
                    // Wouldn't be an update if we go further back than the user's current nightly.
                    if let Some(e) = first_err {
                        break Err(e);
                    } else {
                        // In this case, all newer nightlies are missing, which means there are no
                        // updates, so the user is already at the latest nightly.
                        break Ok(None);
                    }
                }

                toolchain.date = Some(try_next.format("%Y-%m-%d").to_string());
            }
        }
    }
}

fn try_update_from_dist_<'a>(
    download: DownloadCfg<'a>,
    update_hash: Option<&Path>,
    toolchain: &ToolchainDesc,
    profile: Option<Profile>,
    prefix: &InstallPrefix,
    force_update: bool,
    components: &[&str],
    targets: &[&str],
    fetched: &mut String,
) -> Result<Option<String>> {
    let toolchain_str = toolchain.to_string();
    let manifestation = Manifestation::open(prefix.clone(), toolchain.target.clone())?;

    // TODO: Add a notification about which manifest version is going to be used
    (download.notify_handler)(Notification::DownloadingManifest(&toolchain_str));
    match dl_v2_manifest(
        download,
        // Even if manifest has not changed, we must continue to install requested components.
        // So if components or targets is not empty, we skip passing `update_hash` so that
        // we essentially degenerate to `rustup component add` / `rustup target add`
        if components.is_empty() && targets.is_empty() {
            update_hash
        } else {
            None
        },
        toolchain,
    ) {
        Ok(Some((m, hash))) => {
            (download.notify_handler)(Notification::DownloadedManifest(
                &m.date,
                m.get_rust_version().ok(),
            ));

            let profile_components = match profile {
                Some(profile) => m.get_profile_components(profile, &toolchain.target)?,
                None => Vec::new(),
            };

            let mut all_components: HashSet<Component> = profile_components.into_iter().collect();

            let rust_package = m.get_package("rust")?;
            let rust_target_package = rust_package.get_target(Some(&toolchain.target.clone()))?;

            for component in components.iter().copied() {
                let mut component =
                    Component::new(component.to_string(), Some(toolchain.target.clone()), false);
                if let Some(renamed) = m.rename_component(&component) {
                    component = renamed;
                }
                // Look up the newly constructed/renamed component and ensure that
                // if it's a wildcard component we note such, otherwise we end up
                // exacerbating the problem we thought we'd fixed with #2087 and #2115
                if let Some(c) = rust_target_package
                    .components
                    .iter()
                    .find(|c| c.short_name_in_manifest() == component.short_name_in_manifest())
                {
                    if c.target.is_none() {
                        component = component.wildcard();
                    }
                }
                all_components.insert(component);
            }

            for target in targets {
                let triple = TargetTriple::new(target);
                all_components.insert(Component::new("rust-std".to_string(), Some(triple), false));
            }

            let mut explicit_add_components: Vec<_> = all_components.into_iter().collect();
            explicit_add_components.sort();

            let changes = Changes {
                explicit_add_components,
                remove_components: Vec::new(),
            };

            *fetched = m.date.clone();

            return match manifestation.update(
                &m,
                changes,
                force_update,
                &download,
                &download.notify_handler,
                &toolchain.manifest_name(),
                true,
            ) {
                Ok(status) => match status {
                    UpdateStatus::Unchanged => Ok(None),
                    UpdateStatus::Changed => Ok(Some(hash)),
                },
                Err(err) => match err.downcast_ref::<RustupError>() {
                    Some(RustupError::RequestedComponentsUnavailable {
                        components,
                        manifest,
                        toolchain,
                    }) => Err(anyhow!(DistError::ToolchainComponentsMissing(
                        components.to_owned(),
                        manifest.to_owned(),
                        toolchain.to_owned(),
                    ))),
                    Some(_) | None => Err(err),
                },
            };
        }
        Ok(None) => return Ok(None),
        Err(any) => {
            enum Cases {
                DNE,
                CF,
                Other,
            }
            let case = match any.downcast_ref::<RustupError>() {
                Some(RustupError::ChecksumFailed { .. }) => Cases::CF,
                Some(RustupError::DownloadNotExists { .. }) => Cases::DNE,
                _ => Cases::Other,
            };
            match case {
                Cases::CF => return Ok(None),
                Cases::DNE => {
                    // Proceed to try v1 as a fallback
                    (download.notify_handler)(Notification::DownloadingLegacyManifest);
                }
                Cases::Other => return Err(any),
            }
        }
    }

    // If the v2 manifest is not found then try v1
    let manifest = match dl_v1_manifest(download, toolchain) {
        Ok(m) => m,
        Err(any) => {
            enum Cases {
                DNE,
                CF,
                Other,
            }
            let case = match any.downcast_ref::<RustupError>() {
                Some(RustupError::ChecksumFailed { .. }) => Cases::CF,
                Some(RustupError::DownloadNotExists { .. }) => Cases::DNE,
                _ => Cases::Other,
            };
            match case {
                Cases::DNE => {
                    bail!(DistError::MissingReleaseForToolchain(
                        toolchain.manifest_name()
                    ));
                }
                Cases::CF => return Err(any),
                Cases::Other => {
                    return Err(any).with_context(|| {
                        format!(
                            "failed to download manifest for '{}'",
                            toolchain.manifest_name()
                        )
                    });
                }
            }
        }
    };
    let result = manifestation.update_v1(
        &manifest,
        update_hash,
        &download.temp_cfg,
        &download.notify_handler,
        &download.pgp_keys,
    );
    // inspect, determine what context to add, then process afterwards.
    let mut download_not_exists = false;
    match &result {
        Ok(_) => (),
        Err(e) => {
            if let Some(RustupError::DownloadNotExists { .. }) = e.downcast_ref::<RustupError>() {
                download_not_exists = true
            }
        }
    }
    if download_not_exists {
        result.with_context(|| {
            format!(
                "could not download nonexistent rust version `{}`",
                toolchain_str
            )
        })
    } else {
        result
    }
}

pub fn dl_v2_manifest<'a>(
    download: DownloadCfg<'a>,
    update_hash: Option<&Path>,
    toolchain: &ToolchainDesc,
) -> Result<Option<(ManifestV2, String)>> {
    let manifest_url = toolchain.manifest_v2_url(download.dist_root);
    match download.download_and_check(&manifest_url, update_hash, ".toml") {
        Ok(manifest_dl) => {
            // Downloaded ok!
            let (manifest_file, manifest_hash) = if let Some(m) = manifest_dl {
                m
            } else {
                return Ok(None);
            };
            let manifest_str = utils::read_file("manifest", &manifest_file)?;
            let manifest = ManifestV2::parse(&manifest_str)?;

            Ok(Some((manifest, manifest_hash)))
        }
        Err(any) => {
            if let Some(RustupError::ChecksumFailed { .. }) = any.downcast_ref::<RustupError>() {
                // Checksum failed - issue warning to try again later
                (download.notify_handler)(Notification::ManifestChecksumFailedHack);
            }
            Err(any)
        }
    }
}

fn dl_v1_manifest(download: DownloadCfg<'_>, toolchain: &ToolchainDesc) -> Result<Vec<String>> {
    let root_url = toolchain.package_dir(download.dist_root);

    if !["nightly", "beta", "stable"].contains(&&*toolchain.channel) {
        // This is an explicit version. In v1 there was no manifest,
        // you just know the file to download, so synthesize one.
        let installer_name = format!(
            "{}/rust-{}-{}.tar.gz",
            root_url, toolchain.channel, toolchain.target
        );
        return Ok(vec![installer_name]);
    }

    let manifest_url = toolchain.manifest_v1_url(download.dist_root);
    let manifest_dl = download.download_and_check(&manifest_url, None, "")?;
    let (manifest_file, _) = manifest_dl.unwrap();
    let manifest_str = utils::read_file("manifest", &manifest_file)?;
    let urls = manifest_str
        .lines()
        .map(|s| format!("{}/{}", root_url, s))
        .collect();

    Ok(urls)
}

fn utc_from_manifest_date(date_str: &str) -> Option<Date<Utc>> {
    NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .ok()
        .map(|date| Utc.from_utc_date(&date))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parsed_toolchain_desc_parse() {
        let success_cases = vec![
            ("nightly", ("nightly", None, None)),
            ("beta", ("beta", None, None)),
            ("stable", ("stable", None, None)),
            ("0.0", ("0.0", None, None)),
            ("0.0.0", ("0.0.0", None, None)),
            ("0.0.0--", ("0.0.0", None, Some("-"))), // possibly a bug?
            ("9.999.99", ("9.999.99", None, None)),
            ("0.0.0-anything", ("0.0.0", None, Some("anything"))),
            ("0.0.0-0000-00-00", ("0.0.0", Some("0000-00-00"), None)),
            // possibly unexpected behavior, if someone typos a date?
            (
                "0.0.0-00000-000-000",
                ("0.0.0", None, Some("00000-000-000")),
            ),
            // possibly unexpected behavior, if someone forgets to add target after the hyphen?
            ("0.0.0-0000-00-00-", ("0.0.0", None, Some("0000-00-00-"))),
            (
                "0.0.0-0000-00-00-any-other-thing",
                ("0.0.0", Some("0000-00-00"), Some("any-other-thing")),
            ),
            // special hardcoded cases that only have v1 manifests
            ("1.0", ("1.0.0", None, None)),
            ("1.1", ("1.1.0", None, None)),
            ("1.2", ("1.2.0", None, None)),
            ("1.3", ("1.3.0", None, None)),
            ("1.4", ("1.4.0", None, None)),
            ("1.5", ("1.5.0", None, None)),
            ("1.6", ("1.6.0", None, None)),
            ("1.7", ("1.7.0", None, None)),
            ("1.8", ("1.8.0", None, None)),
        ];

        for (input, (channel, date, target)) in success_cases {
            let parsed = input.parse::<ParsedToolchainDesc>();
            assert!(
                parsed.is_ok(),
                "expected parsing of `{}` to succeed: {:?}",
                input,
                parsed
            );

            let expected = ParsedToolchainDesc {
                channel: channel.into(),
                date: date.map(String::from),
                target: target.map(String::from),
            };
            assert_eq!(parsed.unwrap(), expected, "input: `{}`", input);
        }

        let failure_cases = vec!["anything", "00.0000.000", "3", "", "--", "0.0.0-"];

        for input in failure_cases {
            let parsed = input.parse::<ParsedToolchainDesc>();
            assert!(
                parsed.is_err(),
                "expected parsing of `{}` to fail: {:?}",
                input,
                parsed
            );

            let error_message = format!("invalid toolchain name: '{}'", input);

            assert_eq!(
                parsed.unwrap_err().to_string(),
                error_message,
                "input: `{}`",
                input
            );
        }
    }

    #[test]
    fn test_tracking_channels() {
        static CASES: &[(&str, bool)] = &[
            ("stable", true),
            ("beta", true),
            ("nightly", true),
            ("nightly-2020-10-04", false),
            ("1.48", true),
            ("1.47.0", false),
        ];
        for case in CASES {
            let full_tcn = format!("{}-x86_64-unknown-linux-gnu", case.0);
            let tcd = ToolchainDesc::from_str(&full_tcn).unwrap();
            eprintln!("Considering {}", case.0);
            assert_eq!(tcd.is_tracking(), case.1);
        }
    }
}
