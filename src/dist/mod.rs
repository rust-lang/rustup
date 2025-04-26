//! Installation from a Rust distribution server

use std::{
    collections::HashSet, env, fmt, io::Write, ops::Deref, path::Path, str::FromStr, sync::LazyLock,
};

use anyhow::{Context, Result, anyhow, bail};
use chrono::NaiveDate;
use clap::{ValueEnum, builder::PossibleValue};
use itertools::Itertools;
use regex::Regex;
use serde::{Deserialize, Serialize};
use thiserror::Error as ThisError;
use tracing::{info, warn};

use crate::{
    config::{Cfg, dist_root_server},
    errors::RustupError,
    process::Process,
    toolchain::ToolchainName,
    utils,
};

pub mod component;
pub(crate) mod config;

pub mod download;
use download::DownloadCfg;

pub mod manifest;
use manifest::{Component, Manifest as ManifestV2};

pub mod manifestation;
use manifestation::{Changes, Manifestation, UpdateStatus};

pub(crate) mod notifications;
pub use notifications::Notification;

pub mod prefix;
use prefix::InstallPrefix;

pub mod temp;

pub(crate) mod triple;
pub(crate) use triple::*;

pub static DEFAULT_DIST_SERVER: &str = "https://static.rust-lang.org";

/// Returns a error message indicating that certain [`Component`]s are missing in a toolchain distribution.
///
/// This message is currently used exclusively in toolchain-wide operations,
/// otherwise [`component_unavailable_msg`](../../errors/fn.component_unavailable_msg.html) will be used.
///
/// # Panics
/// This function will panic when the collection of unavailable components `cs` is empty.
fn components_missing_msg(cs: &[Component], manifest: &ManifestV2, toolchain: &str) -> String {
    let mut buf = vec![];

    match cs {
        [] => panic!(
            "`components_missing_msg` should not be called with an empty collection of unavailable components"
        ),
        [c] => {
            let _ = writeln!(
                buf,
                "component {} is unavailable for download for channel '{}'",
                c.description(manifest),
                toolchain,
            );
        }
        cs => {
            let cs_str = cs
                .iter()
                .map(|c| c.description(manifest))
                .collect::<Vec<_>>()
                .join(", ");
            let _ = write!(
                buf,
                "some components are unavailable for download for channel '{toolchain}': {cs_str}"
            );
        }
    }

    if toolchain.starts_with("nightly") {
        let _ = write!(
            buf,
            "\
Sometimes not all components are available in any given nightly.
If you don't need these components, you could try a minimal installation with:

    rustup toolchain add {toolchain} --profile minimal

If you require these components, please install and use the latest successfully built version,
which you can find at <https://rust-lang.github.io/rustup-components-history>.

After determining the correct date, install it with a command such as:

    rustup toolchain install nightly-2018-12-27

Then you can use the toolchain with commands such as:

    cargo +nightly-2018-12-27 build"
        );
    } else if ["beta", "stable"].iter().any(|&p| toolchain.starts_with(p)) {
        let _ = write!(
            buf,
            "\
One or many components listed above might have been permanently removed from newer versions
of the official Rust distribution due to deprecation.

You can find the list of removed components at
<https://rust-lang.github.io/rustup/devel/concepts/components.html#previous-components>.

If you are updating an existing toolchain, after determining the deprecated component(s)
and/or target(s) in question, please remove them with:

    rustup component remove --toolchain {toolchain} <COMPONENT>...
    rustup target remove --toolchain {toolchain} <TARGET>...

After that, you should be able to continue with the update as usual.",
        );
    }

    String::from_utf8(buf).unwrap()
}

#[derive(Debug, ThisError)]
pub enum DistError {
    #[error("{}", components_missing_msg(.0, .1, .2))]
    ToolchainComponentsMissing(Vec<Component>, Box<ManifestV2>, String),
    #[error("no release found for '{0}'")]
    MissingReleaseForToolchain(String),
    #[error("invalid toolchain name: '{0}'")]
    InvalidOfficialName(String),
}

#[derive(Debug, PartialEq)]
struct ParsedToolchainDesc {
    channel: Channel,
    date: Option<String>,
    target: Option<String>,
}

/// A toolchain descriptor from rustup's perspective. These contain
/// 'partial target triples', which allow toolchain names like
/// 'stable-msvc' to work. Partial target triples though are parsed
/// from a hardcoded set of known triples, whereas target triples
/// are nearly-arbitrary strings.
#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub struct PartialToolchainDesc {
    pub channel: Channel,
    pub date: Option<String>,
    pub target: PartialTargetTriple,
}

/// Fully-resolved toolchain descriptors. These always have full target
/// triples attached to them and are used for canonical identification,
/// such as naming their installation directory.
///
/// As strings they look like stable-x86_64-pc-windows-msvc or
/// 1.55-x86_64-pc-windows-msvc
#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub struct ToolchainDesc {
    pub channel: Channel,
    pub date: Option<String>,
    pub target: TargetTriple,
}

#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub enum Channel {
    Stable,
    Beta,
    Nightly,
    Version(PartialVersion),
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stable => write!(f, "stable"),
            Self::Beta => write!(f, "beta"),
            Self::Nightly => write!(f, "nightly"),
            Self::Version(ver) => write!(f, "{ver}"),
        }
    }
}

impl FromStr for Channel {
    type Err = anyhow::Error;
    fn from_str(chan: &str) -> Result<Self> {
        match chan {
            "stable" => Ok(Self::Stable),
            "beta" => Ok(Self::Beta),
            "nightly" => Ok(Self::Nightly),
            ver => ver.parse().map(Self::Version),
        }
    }
}

/// A possibly incomplete Rust toolchain version that
/// can be converted from and to its string form.
#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub struct PartialVersion {
    pub major: u64,
    pub minor: Option<u64>,
    pub patch: Option<u64>,
    pub pre: semver::Prerelease,
}

impl fmt::Display for PartialVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.major)?;
        if let Some(minor) = self.minor {
            write!(f, ".{minor}")?;
        }
        if let Some(patch) = self.patch {
            write!(f, ".{patch}")?;
        }
        if !self.pre.is_empty() {
            write!(f, "-{}", self.pre)?;
        }
        Ok(())
    }
}

impl FromStr for PartialVersion {
    type Err = anyhow::Error;
    fn from_str(ver: &str) -> Result<Self> {
        // `semver::Comparator::from_str` supports an optional operator
        // (e.g. `=`, `>`, `>=`, `<`, `<=`, `~`, `^`, `*`) before the
        // partial version, so we should exclude that case first.
        if let Some(ch) = ver.chars().nth(0) {
            if !ch.is_ascii_digit() {
                return Err(anyhow!(
                    "expected ASCII digit at the beginning of `{ver}`, found `{ch}`"
                )
                .context("error parsing `PartialVersion`"));
            }
        }
        let (ver, pre) = ver.split_once('-').unwrap_or((ver, ""));
        let comparator =
            semver::Comparator::from_str(ver).context("error parsing `PartialVersion`")?;
        Ok(Self {
            major: comparator.major,
            minor: comparator.minor,
            patch: comparator.patch,
            pre: semver::Prerelease::new(pre).context("error parsing `PartialVersion`")?,
        })
    }
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize)]
#[serde(transparent)]
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
#[cfg(all(not(windows), not(target_env = "musl")))]
const TRIPLE_LOONGARCH64_UNKNOWN_LINUX: &str = "loongarch64-unknown-linux-gnu";
#[cfg(all(not(windows), target_env = "musl"))]
const TRIPLE_LOONGARCH64_UNKNOWN_LINUX: &str = "loongarch64-unknown-linux-musl";
#[cfg(all(not(windows), not(target_env = "musl")))]
const TRIPLE_POWERPC64LE_UNKNOWN_LINUX: &str = "powerpc64le-unknown-linux-gnu";
#[cfg(all(not(windows), target_env = "musl"))]
const TRIPLE_POWERPC64LE_UNKNOWN_LINUX: &str = "powerpc64le-unknown-linux-musl";

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
        // Note this regex gives you a guaranteed match of the channel (1)
        // and an optional match of the date (2) and target (3)
        static TOOLCHAIN_CHANNEL_RE: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(&format!(
                r"^({})(?:-([0-9]{{4}}-[0-9]{{2}}-[0-9]{{2}}))?(?:-(.+))?$",
                // The channel patterns we support
                [
                    "nightly",
                    "beta",
                    "stable",
                    // Allow from 1.0.0 through to 9.999.99 with optional patch version
                    // and optional beta tag
                    r"[0-9]{1}\.[0-9]{1,3}(?:\.[0-9]{1,2})?(?:-beta(?:\.[0-9]{1,2})?)?",
                ]
                .join("|")
            ))
            .unwrap()
        });

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
                channel: Channel::from_str(channel).unwrap(),
                date: c.get(2).map(|s| s.as_str()).and_then(fn_map),
                target: c.get(3).map(|s| s.as_str()).and_then(fn_map),
            }
        });

        match d {
            Some(d) => Ok(d),
            None => Err(RustupError::InvalidToolchainName(desc.to_string()).into()),
        }
    }
}

impl Deref for TargetTriple {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Check if /bin/sh is a 32-bit binary. If it doesn't exist, fall back to
/// checking if _we_ are a 32-bit binary.
/// rustup-init.sh also relies on checking /bin/sh for bitness.
#[cfg(not(windows))]
fn is_32bit_userspace() -> bool {
    use std::fs;
    use std::io::{self, Read};

    // inner function is to simplify error handling.
    fn inner() -> io::Result<bool> {
        let mut f = fs::File::open("/bin/sh")?;
        let mut buf = [0; 5];
        f.read_exact(&mut buf)?;

        // ELF files start out "\x7fELF", and the following byte is
        //   0x01 for 32-bit and
        //   0x02 for 64-bit.
        Ok(&buf == b"\x7fELF\x01")
    }

    inner().unwrap_or(cfg!(target_pointer_width = "32"))
}

impl TargetTriple {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub(crate) fn from_build() -> Self {
        if let Some(triple) = option_env!("RUSTUP_OVERRIDE_BUILD_TRIPLE") {
            Self::new(triple)
        } else {
            Self::new(env!("TARGET"))
        }
    }

    #[cfg(not(target_os = "macos"))]
    pub(crate) fn is_host_emulated() -> bool {
        false
    }

    /// Detects Rosetta emulation on macOS
    #[cfg(target_os = "macos")]
    pub(crate) fn is_host_emulated() -> bool {
        unsafe {
            let mut ret: libc::c_int = 0;
            let mut size = std::mem::size_of::<libc::c_int>() as libc::size_t;
            let err = libc::sysctlbyname(
                c"sysctl.proc_translated".as_ptr().cast(),
                (&mut ret) as *mut _ as *mut libc::c_void,
                &mut size,
                std::ptr::null_mut(),
                0,
            );
            err == 0 && ret != 0
        }
    }

    pub(crate) fn from_host(process: &Process) -> Option<Self> {
        #[cfg(windows)]
        fn inner() -> Option<TargetTriple> {
            use std::mem;

            /// Get the host architecture using `IsWow64Process2`. This function
            /// produces the most accurate results (supports detecting aarch64), but
            /// it is only available on Windows 10 1511+, so we use `GetProcAddress`
            /// to maintain backward compatibility with older Windows versions.
            fn arch_primary() -> Option<&'static str> {
                use windows_sys::Win32::Foundation::{BOOL, HANDLE};
                use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
                use windows_sys::Win32::System::Threading::GetCurrentProcess;
                use windows_sys::core::s;

                const IMAGE_FILE_MACHINE_ARM64: u16 = 0xAA64;
                const IMAGE_FILE_MACHINE_AMD64: u16 = 0x8664;
                const IMAGE_FILE_MACHINE_I386: u16 = 0x014c;

                #[allow(non_snake_case)]
                let IsWow64Process2: unsafe extern "system" fn(
                    HANDLE,
                    *mut u16,
                    *mut u16,
                )
                    -> BOOL = unsafe {
                    let module = GetModuleHandleA(s!("kernel32.dll"));
                    if module.is_null() {
                        return None;
                    }
                    mem::transmute(GetProcAddress(module, s!("IsWow64Process2"))?)
                };

                let mut _machine = 0;
                let mut native_machine = 0;
                unsafe {
                    // cannot fail; handle does not need to be closed.
                    let process = GetCurrentProcess();
                    if IsWow64Process2(process, &mut _machine, &mut native_machine) == 0 {
                        return None;
                    }
                };
                match native_machine {
                    IMAGE_FILE_MACHINE_AMD64 => Some("x86_64"),
                    IMAGE_FILE_MACHINE_I386 => Some("i686"),
                    IMAGE_FILE_MACHINE_ARM64 => Some("aarch64"),
                    _ => None,
                }
            }

            /// Get the host architecture using `GetNativeSystemInfo`.
            /// Does not support detecting aarch64.
            fn arch_fallback() -> Option<&'static str> {
                use windows_sys::Win32::System::SystemInformation::GetNativeSystemInfo;

                const PROCESSOR_ARCHITECTURE_AMD64: u16 = 9;
                const PROCESSOR_ARCHITECTURE_INTEL: u16 = 0;

                let mut sys_info;
                unsafe {
                    sys_info = mem::zeroed();
                    GetNativeSystemInfo(&mut sys_info);
                }

                match unsafe { sys_info.Anonymous.Anonymous }.wProcessorArchitecture {
                    PROCESSOR_ARCHITECTURE_AMD64 => Some("x86_64"),
                    PROCESSOR_ARCHITECTURE_INTEL => Some("i686"),
                    _ => None,
                }
            }

            // Default to msvc
            let arch = arch_primary().or_else(arch_fallback)?;
            let msvc_triple = format!("{arch}-pc-windows-msvc");
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

            #[cfg(not(target_os = "android"))]
            let host_triple = match (sysname, machine) {
                (b"Linux", b"x86_64") => Some(TRIPLE_X86_64_UNKNOWN_LINUX),
                (b"Linux", b"i686") => Some("i686-unknown-linux-gnu"),
                (b"Linux", b"mips") => Some(TRIPLE_MIPS_UNKNOWN_LINUX_GNU),
                (b"Linux", b"mips64") => Some(TRIPLE_MIPS64_UNKNOWN_LINUX_GNUABI64),
                (b"Linux", b"arm") => Some("arm-unknown-linux-gnueabi"),
                (b"Linux", b"armv7l") => Some("armv7-unknown-linux-gnueabihf"),
                (b"Linux", b"armv8l") => Some("armv7-unknown-linux-gnueabihf"),
                (b"Linux", b"aarch64") => Some(if is_32bit_userspace() {
                    "armv7-unknown-linux-gnueabihf"
                } else {
                    TRIPLE_AARCH64_UNKNOWN_LINUX
                }),
                (b"Linux", b"loongarch64") => Some(TRIPLE_LOONGARCH64_UNKNOWN_LINUX),
                (b"Linux", b"ppc64le") => Some(TRIPLE_POWERPC64LE_UNKNOWN_LINUX),
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

            #[cfg(target_os = "android")]
            let host_triple = match (sysname, machine) {
                (_, b"arm") => Some("arm-linux-androideabi"),
                (_, b"armv7l") => Some("armv7-linux-androideabi"),
                (_, b"armv8l") => Some("armv7-linux-androideabi"),
                (_, b"aarch64") => Some("aarch64-linux-android"),
                (_, b"i686") => Some("i686-linux-android"),
                (_, b"x86_64") => Some("x86_64-linux-android"),
                _ => None,
            };

            host_triple.map(TargetTriple::new)
        }

        if let Ok(triple) = process.var("RUSTUP_OVERRIDE_HOST_TRIPLE") {
            Some(Self(triple))
        } else {
            inner()
        }
    }

    pub(crate) fn from_host_or_build(process: &Process) -> Self {
        Self::from_host(process).unwrap_or_else(Self::from_build)
    }

    pub(crate) fn can_run(&self, other: &TargetTriple) -> Result<bool> {
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
            // Windows is a special case here: we can run gnu and msvc on the same system,
            // x86_64 can run i686, and aarch64 can run i686 through emulation
            (partial_self.arch == partial_other.arch)
                || (partial_self.arch.as_deref() == Some("x86_64")
                    && partial_other.arch.as_deref() == Some("i686"))
                || (partial_self.arch.as_deref() == Some("aarch64")
                    && partial_other.arch.as_deref() == Some("i686"))
        } else {
            // For other OSes, for now, we assume other toolchains won't run
            false
        };
        Ok(ret)
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
    /// Create a toolchain desc using input_host to fill in missing fields
    pub(crate) fn resolve(self, input_host: &TargetTriple) -> Result<ToolchainDesc> {
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
            format!("{arch}-{os}-{env}")
        } else {
            format!("{arch}-{os}")
        };

        Ok(ToolchainDesc {
            channel: self.channel,
            date: self.date,
            target: TargetTriple(trip),
        })
    }

    pub(crate) fn has_triple(&self) -> bool {
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
    pub(crate) fn manifest_v1_url(&self, dist_root: &str, process: &Process) -> String {
        let do_manifest_staging = process.var("RUSTUP_STAGED_MANIFEST").is_ok();
        match (self.date.as_ref(), do_manifest_staging) {
            (None, false) => format!("{}/channel-rust-{}", dist_root, self.channel),
            (Some(date), false) => format!("{}/{}/channel-rust-{}", dist_root, date, self.channel),
            (None, true) => format!("{}/staging/channel-rust-{}", dist_root, self.channel),
            (Some(_), true) => panic!("not a real-world case"),
        }
    }

    pub(crate) fn manifest_v2_url(&self, dist_root: &str, process: &Process) -> String {
        format!("{}.toml", self.manifest_v1_url(dist_root, process))
    }
    /// Either "$channel" or "channel-$date"
    pub fn manifest_name(&self) -> String {
        match self.date {
            None => self.channel.to_string(),
            Some(ref date) => format!("{}-{}", self.channel, date),
        }
    }

    pub(crate) fn package_dir(&self, dist_root: &str) -> String {
        match self.date {
            None => dist_root.to_string(),
            Some(ref date) => format!("{dist_root}/{date}"),
        }
    }

    /// Toolchain channels are considered 'tracking' if it is one of the named channels
    /// such as `stable`, or is an incomplete version such as `1.48`, and the
    /// date field is empty.
    pub(crate) fn is_tracking(&self) -> bool {
        match &self.channel {
            _ if self.date.is_some() => false,
            Channel::Stable | Channel::Beta | Channel::Nightly => true,
            Channel::Version(ver) => ver.patch.is_none() || &*ver.pre == "beta",
        }
    }
}

impl TryFrom<&ToolchainName> for ToolchainDesc {
    type Error = DistError;

    fn try_from(value: &ToolchainName) -> std::result::Result<Self, Self::Error> {
        match value {
            ToolchainName::Custom(n) => Err(DistError::InvalidOfficialName(n.to_string())),
            ToolchainName::Official(n) => Ok(n.clone()),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Profile {
    Minimal,
    #[default]
    Default,
    Complete,
}

impl Profile {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Minimal => "minimal",
            Self::Default => "default",
            Self::Complete => "complete",
        }
    }
}

impl ValueEnum for Profile {
    fn value_variants<'a>() -> &'a [Self] {
        &[Profile::Minimal, Profile::Default, Profile::Complete]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(PossibleValue::new(self.as_str()))
    }

    fn from_str(input: &str, _: bool) -> Result<Self, String> {
        <Self as FromStr>::from_str(input).map_err(|e| e.to_string())
    }
}

impl FromStr for Profile {
    type Err = anyhow::Error;

    fn from_str(name: &str) -> Result<Self> {
        match name {
            "minimal" | "m" => Ok(Self::Minimal),
            "default" | "d" | "" => Ok(Self::Default),
            "complete" | "c" => Ok(Self::Complete),
            _ => Err(anyhow!(format!(
                "unknown profile name: '{}'; valid profile names are: {}",
                name,
                Self::value_variants().iter().join(", ")
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AutoInstallMode {
    #[default]
    Enable,
    Disable,
}

impl AutoInstallMode {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Enable => "enable",
            Self::Disable => "disable",
        }
    }
}

impl ValueEnum for AutoInstallMode {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Enable, Self::Disable]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(PossibleValue::new(self.as_str()))
    }

    fn from_str(input: &str, _: bool) -> Result<Self, String> {
        <Self as FromStr>::from_str(input).map_err(|e| e.to_string())
    }
}

impl FromStr for AutoInstallMode {
    type Err = anyhow::Error;

    fn from_str(mode: &str) -> Result<Self> {
        match mode {
            "enable" => Ok(Self::Enable),
            "disable" => Ok(Self::Disable),
            _ => Err(anyhow!(format!(
                "unknown auto install mode: '{}'; valid modes are {}",
                mode,
                Self::value_variants().iter().join(", ")
            ))),
        }
    }
}

impl std::fmt::Display for AutoInstallMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
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
            write!(f, "-{date}")?;
        }
        if let Some(ref arch) = self.target.arch {
            write!(f, "-{arch}")?;
        }
        if let Some(ref os) = self.target.os {
            write!(f, "-{os}")?;
        }
        if let Some(ref env) = self.target.env {
            write!(f, "-{env}")?;
        }

        Ok(())
    }
}

impl fmt::Display for ToolchainDesc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &self.channel)?;

        if let Some(ref date) = self.date {
            write!(f, "-{date}")?;
        }
        write!(f, "-{}", self.target)?;

        Ok(())
    }
}

impl fmt::Display for Profile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Clone)]
pub(crate) struct DistOptions<'a> {
    pub(crate) cfg: &'a Cfg<'a>,
    pub(crate) toolchain: &'a ToolchainDesc,
    pub(crate) profile: Profile,
    pub(crate) update_hash: Option<&'a Path>,
    pub(crate) dl_cfg: DownloadCfg<'a>,
    /// --force bool is whether to force an update/install
    pub(crate) force: bool,
    /// --allow-downgrade
    pub(crate) allow_downgrade: bool,
    /// toolchain already exists
    pub(crate) exists: bool,
    /// currently installed date and version
    pub(crate) old_date_version: Option<(String, String)>,
    /// Extra components to install from dist
    pub(crate) components: &'a [&'a str],
    /// Extra targets to install from dist
    pub(crate) targets: &'a [&'a str],
}

// Installs or updates a toolchain from a dist server. If an initial
// install then it will be installed with the default components. If
// an upgrade then all the existing components will be upgraded.
//
// Returns the manifest's hash if anything changed.
#[tracing::instrument(level = "trace", err(level = "trace"), skip_all, fields(profile=format!("{:?}", opts.profile), prefix=prefix.path().to_string_lossy().to_string()))]
pub(crate) async fn update_from_dist(
    prefix: &InstallPrefix,
    opts: &DistOptions<'_>,
) -> Result<Option<String>> {
    let fresh_install = !prefix.path().exists();
    if let Some(hash) = opts.update_hash {
        // fresh_install means the toolchain isn't present, but hash_exists means there is a stray hash file
        if fresh_install && Path::exists(hash) {
            (opts.dl_cfg.notify_handler)(Notification::StrayHash(hash));
            std::fs::remove_file(hash)?;
        }
    }

    let mut fetched = String::new();
    let mut first_err = None;
    let backtrack = opts.toolchain.channel == Channel::Nightly && opts.toolchain.date.is_none();
    // We want to limit backtracking if we do not already have a toolchain
    let mut backtrack_limit: Option<i32> = if opts.toolchain.date.is_some() {
        None
    } else {
        // We limit the backtracking to 21 days by default (half a release cycle).
        // The limit of 21 days is an arbitrary selection, so we let the user override it.
        const BACKTRACK_LIMIT_DEFAULT: i32 = 21;
        let provided = opts
            .dl_cfg
            .process
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
    let first_manifest = date_from_manifest_date("2014-12-20").unwrap();
    let old_manifest = opts
        .old_date_version
        .as_ref()
        .and_then(|(d, _)| date_from_manifest_date(d))
        .unwrap_or(first_manifest);
    let last_manifest = if opts.allow_downgrade {
        first_manifest
    } else {
        old_manifest
    };

    let current_manifest = {
        let manifestation = Manifestation::open(prefix.clone(), opts.toolchain.target.clone())?;
        manifestation.load_manifest()?
    };

    let mut toolchain = opts.toolchain.clone();
    let res = loop {
        let result = try_update_from_dist_(
            opts.dl_cfg,
            opts.update_hash,
            &toolchain,
            match opts.exists {
                false => Some(opts.profile),
                true => None,
            },
            prefix,
            opts.force,
            opts.components,
            opts.targets,
            &mut fetched,
        )
        .await;

        let e = match result {
            Ok(v) => break Ok(v),
            Err(e) if !backtrack => break Err(e),
            Err(e) => e,
        };

        let cause = e.downcast_ref::<DistError>();
        match cause {
            Some(DistError::ToolchainComponentsMissing(components, manifest, ..)) => {
                (opts.dl_cfg.notify_handler)(Notification::SkippingNightlyMissingComponent(
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
            _ => {
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
        let try_next = date_from_manifest_date(toolchain_date)
            .unwrap_or_else(|| panic!("Malformed manifest date: {toolchain_date:?}"))
            .pred_opt()
            .unwrap();

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
    };

    // Don't leave behind an empty / broken installation directory
    if res.is_err() && fresh_install {
        // FIXME Ignoring cascading errors
        let _ = utils::remove_dir("toolchain", prefix.path(), opts.dl_cfg.notify_handler);
    }

    res
}

#[allow(clippy::too_many_arguments)]
async fn try_update_from_dist_(
    download: DownloadCfg<'_>,
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
    )
    .await
    {
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

            for component in components {
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

            for &target in targets {
                let triple = TargetTriple::new(target);
                all_components.insert(Component::new("rust-std".to_string(), Some(triple), false));
            }

            let mut explicit_add_components: Vec<_> = all_components.into_iter().collect();
            explicit_add_components.sort();

            let changes = Changes {
                explicit_add_components,
                remove_components: Vec::new(),
            };

            fetched.clone_from(&m.date);

            return match manifestation
                .update(
                    &m,
                    changes,
                    force_update,
                    &download,
                    &toolchain.manifest_name(),
                    true,
                )
                .await
            {
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
                        Box::new(manifest.to_owned()),
                        toolchain.to_owned(),
                    ))),
                    Some(_) | None => Err(err),
                },
            };
        }
        Ok(None) => return Ok(None),
        Err(err) => {
            match err.downcast_ref::<RustupError>() {
                Some(RustupError::ChecksumFailed { .. }) => return Ok(None),
                Some(RustupError::DownloadNotExists { .. }) => {
                    // Proceed to try v1 as a fallback
                    (download.notify_handler)(Notification::DownloadingLegacyManifest)
                }
                _ => return Err(err),
            }
        }
    }

    // If the v2 manifest is not found then try v1
    let manifest = match dl_v1_manifest(download, toolchain).await {
        Ok(m) => m,
        Err(err) => match err.downcast_ref::<RustupError>() {
            Some(RustupError::ChecksumFailed { .. }) => return Err(err),
            Some(RustupError::DownloadNotExists { .. }) => {
                bail!(DistError::MissingReleaseForToolchain(
                    toolchain.manifest_name()
                ));
            }
            _ => {
                return Err(err).with_context(|| {
                    format!(
                        "failed to download manifest for '{}'",
                        toolchain.manifest_name()
                    )
                });
            }
        },
    };

    let result = manifestation
        .update_v1(
            &manifest,
            update_hash,
            download.tmp_cx,
            &download.notify_handler,
            download.process,
        )
        .await;

    // inspect, determine what context to add, then process afterwards.
    if let Err(e) = &result {
        if let Some(RustupError::DownloadNotExists { .. }) = e.downcast_ref::<RustupError>() {
            return result.with_context(|| {
                format!("could not download nonexistent rust version `{toolchain_str}`")
            });
        }
    }

    result
}

pub(crate) async fn dl_v2_manifest(
    download: DownloadCfg<'_>,
    update_hash: Option<&Path>,
    toolchain: &ToolchainDesc,
) -> Result<Option<(ManifestV2, String)>> {
    let manifest_url = toolchain.manifest_v2_url(download.dist_root, download.process);
    match download
        .download_and_check(&manifest_url, update_hash, ".toml")
        .await
    {
        Ok(manifest_dl) => {
            // Downloaded ok!
            let Some((manifest_file, manifest_hash)) = manifest_dl else {
                return Ok(None);
            };
            let manifest_str = utils::read_file("manifest", &manifest_file)?;
            let manifest =
                ManifestV2::parse(&manifest_str).with_context(|| RustupError::ParsingFile {
                    name: "manifest",
                    path: manifest_file.to_path_buf(),
                })?;

            Ok(Some((manifest, manifest_hash)))
        }
        Err(any) => {
            if let Some(err @ RustupError::ChecksumFailed { .. }) =
                any.downcast_ref::<RustupError>()
            {
                // Manifest checksum mismatched.
                warn!("{err}");

                let server = dist_root_server(download.process)?;
                if server == DEFAULT_DIST_SERVER {
                    info!(
                        "this is likely due to an ongoing update of the official release server, please try again later"
                    );
                    info!("see <https://github.com/rust-lang/rustup/issues/3390> for more details");
                } else {
                    info!(
                        "this might indicate an issue with the third-party release server '{server}'"
                    );
                    info!("see <https://github.com/rust-lang/rustup/issues/3885> for more details");
                }
            }
            Err(any)
        }
    }
}

async fn dl_v1_manifest(
    download: DownloadCfg<'_>,
    toolchain: &ToolchainDesc,
) -> Result<Vec<String>> {
    let root_url = toolchain.package_dir(download.dist_root);

    if let Channel::Version(ver) = &toolchain.channel {
        // This is an explicit version. In v1 there was no manifest,
        // you just know the file to download, so synthesize one.
        let installer_name = format!("{}/rust-{}-{}.tar.gz", root_url, ver, toolchain.target);
        return Ok(vec![installer_name]);
    }

    let manifest_url = toolchain.manifest_v1_url(download.dist_root, download.process);
    let manifest_dl = download.download_and_check(&manifest_url, None, "").await?;
    let (manifest_file, _) = manifest_dl.unwrap();
    let manifest_str = utils::read_file("manifest", &manifest_file)?;
    let urls = manifest_str
        .lines()
        .map(|s| format!("{root_url}/{s}"))
        .collect();

    Ok(urls)
}

fn date_from_manifest_date(date_str: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok()
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

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
            // channels with beta tags
            ("0.0.0-beta", ("0.0.0-beta", None, None)),
            ("0.0.0-beta.1", ("0.0.0-beta.1", None, None)),
            (
                "0.0.0-beta.1-0000-00-00",
                ("0.0.0-beta.1", Some("0000-00-00"), None),
            ),
            (
                "0.0.0-beta.1-anything",
                ("0.0.0-beta.1", None, Some("anything")),
            ),
            (
                "0.0.0-beta-anything",
                ("0.0.0-beta", None, Some("anything")),
            ),
            (
                "0.0.0-beta.1-0000-00-00-any-other-thing",
                ("0.0.0-beta.1", Some("0000-00-00"), Some("any-other-thing")),
            ),
        ];

        for (input, (channel, date, target)) in success_cases {
            let parsed = input.parse::<ParsedToolchainDesc>();
            assert!(
                parsed.is_ok(),
                "expected parsing of `{input}` to succeed: {parsed:?}"
            );

            let expected = ParsedToolchainDesc {
                channel: Channel::from_str(channel).unwrap(),
                date: date.map(String::from),
                target: target.map(String::from),
            };
            assert_eq!(parsed.unwrap(), expected, "input: `{input}`");
        }

        let failure_cases = vec!["anything", "00.0000.000", "3", "", "--", "0.0.0-"];

        for input in failure_cases {
            let parsed = input.parse::<ParsedToolchainDesc>();
            assert!(
                parsed.is_err(),
                "expected parsing of `{input}` to fail: {parsed:?}"
            );

            let error_message = format!("invalid toolchain name: '{input}'");

            assert_eq!(
                parsed.unwrap_err().to_string(),
                error_message,
                "input: `{input}`"
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
            ("1.23-beta", true),
            ("1.23.0-beta", true),
            ("1.23.0-beta.2", false),
        ];
        for case in CASES {
            let full_tcn = format!("{}-x86_64-unknown-linux-gnu", case.0);
            let tcd = ToolchainDesc::from_str(&full_tcn).unwrap();
            eprintln!("Considering {}", case.0);
            assert_eq!(tcd.is_tracking(), case.1);
        }
    }

    #[test]
    fn partial_version_from_str() -> Result<()> {
        assert_eq!(
            PartialVersion::from_str("0.12")?,
            PartialVersion {
                major: 0,
                minor: Some(12),
                patch: None,
                pre: semver::Prerelease::EMPTY,
            },
        );
        assert_eq!(
            PartialVersion::from_str("1.23-beta")?,
            PartialVersion {
                major: 1,
                minor: Some(23),
                patch: None,
                pre: semver::Prerelease::new("beta").unwrap(),
            },
        );
        assert_eq!(
            PartialVersion::from_str("1.23.0-beta.4")?,
            PartialVersion {
                major: 1,
                minor: Some(23),
                patch: Some(0),
                pre: semver::Prerelease::new("beta.4").unwrap(),
            },
        );

        assert!(PartialVersion::from_str("1.01").is_err()); // no leading zeros
        assert!(PartialVersion::from_str("^1.23").is_err()); // no comparing operators
        assert!(PartialVersion::from_str(">=1").is_err());
        assert!(PartialVersion::from_str("*").is_err());
        assert!(PartialVersion::from_str("stable").is_err());

        Ok(())
    }

    proptest! {
        #[test]
        fn partial_version_from_str_to_str(
            ver in r"[0-9]{1}(\.(0|[1-9][0-9]{0,2}))(\.(0|[1-9][0-9]{0,1}))?(-beta(\.(0|[1-9][1-9]{0,1}))?)?"
        ) {
            prop_assert_eq!(PartialVersion::from_str(&ver).unwrap().to_string(), ver);
        }
    }

    #[test]
    fn compatible_host_triples() {
        static CASES: &[(&str, &[&str], &[&str])] = &[
            (
                // 64bit linux
                "x86_64-unknown-linux-gnu",
                // Not compatible beyond itself
                &[],
                // Even 32bit linux is considered not compatible by default
                &["i686-unknown-linux-gnu"],
            ),
            (
                // On the other hand, 64 bit Windows
                "x86_64-pc-windows-msvc",
                // is compatible with 32 bit windows, and even gnu
                &[
                    "i686-pc-windows-msvc",
                    "x86_64-pc-windows-gnu",
                    "i686-pc-windows-gnu",
                ],
                // But is not compatible with Linux
                &["x86_64-unknown-linux-gnu"],
            ),
            (
                // Indeed, 64bit windows with the gnu toolchain
                "x86_64-pc-windows-gnu",
                // is compatible with the other windows platforms
                &[
                    "i686-pc-windows-msvc",
                    "x86_64-pc-windows-gnu",
                    "i686-pc-windows-gnu",
                ],
                // But is not compatible with Linux despite also being gnu
                &["x86_64-unknown-linux-gnu"],
            ),
            (
                // However, 32bit Windows is not expected to be able to run
                // 64bit windows
                "i686-pc-windows-msvc",
                &["i686-pc-windows-gnu"],
                &["x86_64-pc-windows-msvc", "x86_64-pc-windows-gnu"],
            ),
        ];

        for &(host, compatible, incompatible) in CASES {
            println!("host={host}");
            let host = TargetTriple::new(host);
            assert!(host.can_run(&host).unwrap(), "host wasn't self-compatible");
            for &other in compatible.iter() {
                println!("compatible with {other}");
                let other = TargetTriple::new(other);
                assert!(
                    host.can_run(&other).unwrap(),
                    "host and other were unexpectedly incompatible"
                );
            }
            for &other in incompatible.iter() {
                println!("incompatible with {other}");
                let other = TargetTriple::new(other);
                assert!(
                    !host.can_run(&other).unwrap(),
                    "host and other were unexpectedly compatible"
                );
            }
        }
    }
}
