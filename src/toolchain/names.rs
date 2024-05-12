//! Overview of toolchain modeling.
//!
//! From the user (including config files, toolchain files and manifests) we get
//! a String. Strings are convertible into `MaybeOfficialToolchainName`,
//! `ResolvableToolchainName`, and `ResolvableLocalToolchainName`.
//!
//! `MaybeOfficialToolchainName` represents a toolchain passed to rustup-init:
//! 'none' to select no toolchain to install, and otherwise a partial toolchain
//! description - channel and optional triple and optional date.
//!
//! `ResolvableToolchainName` represents a toolchain name from a user. Either a
//! partial toolchain description or a single path component that is not 'none'.
//!
//! `MaybeResolvableToolchainName` is analogous to MaybeOfficialToolchainName
//! for both custom and official names.
//!
//! `ToolchainName` is the result of resolving `ResolvableToolchainName` with a
//! host triple, or parsing an installed toolchain name directly.
//!
//! `ResolvableLocalToolchainName` represents the values permittable in
//! `RUSTUP_TOOLCHAIN`: resolved or not resolved official names, custom names,
//! and absolute paths.
//!
//! `LocalToolchainName` represents all the toolchain names that can make sense
//! for referring to actually present toolchains. One of a `ToolchainName` or an
//! absolute path.
//!
//! From the toolchains directory we can iterate directly over
//! `ResolvedToolchainName`.
//!
//! OfficialToolchainName represents a resolved official toolchain name and can
//! be used to download or install toolchains via a downloader.
//!
//! CustomToolchainName can be used to link toolchains to local paths on disk.
//!
//! PathBasedToolchainName can obtained from rustup toolchain files.
//!
//! State from toolchains on disk can be loaded in an InstalledToolchain struct
//! and passed around and queried. The details on that are still vague :).
//!
//! Generally there are infallible Convert impl's for any safe conversion and
//! fallible ones otherwise.

use std::{
    fmt::Display,
    ops::Deref,
    path::{Path, PathBuf},
    str::FromStr,
};

use thiserror::Error;

use crate::dist::dist::{PartialToolchainDesc, TargetTriple, ToolchainDesc};

/// Errors related to toolchains
#[derive(Error, Debug)]
pub enum InvalidName {
    #[error("invalid official toolchain name '{0}'")]
    OfficialName(String),
    #[error("invalid custom toolchain name '{0}'")]
    CustomName(String),
    #[error("invalid path toolchain '{0}'")]
    PathToolchain(String),
    #[error("relative path toolchain '{0}'")]
    PathToolchainRelative(String),
    #[error("invalid toolchain: the path '{0}' has no bin/ directory")]
    ToolchainPath(String),
    #[error("invalid toolchain name '{0}'")]
    ToolchainName(String),
}

macro_rules! from_variant {
    ($from:ident, $to:ident, $variant:expr) => {
        impl From<$from> for $to {
            fn from(value: $from) -> Self {
                $variant(value)
            }
        }
        impl From<&$from> for $to {
            fn from(value: &$from) -> Self {
                $variant(value.to_owned())
            }
        }
    };
}

macro_rules! try_from_str {
    ($to:ident) => {
        try_from_str!(&str, $to);
        try_from_str!(&String, $to);
        impl TryFrom<String> for $to {
            type Error = InvalidName;

            fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
                $to::validate(&value)
            }
        }

        impl FromStr for $to {
            type Err = InvalidName;

            fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
                $to::validate(value)
            }
        }
    };
    ($from:ty, $to:ident) => {
        impl TryFrom<$from> for $to {
            type Error = InvalidName;

            fn try_from(value: $from) -> std::result::Result<Self, Self::Error> {
                $to::validate(value)
            }
        }
    };
}

/// Common validate rules for all sorts of toolchain names
fn validate(candidate: &str) -> Result<&str, InvalidName> {
    let normalized_name = candidate.trim_end_matches('/');
    if normalized_name.is_empty() {
        Err(InvalidName::ToolchainName(candidate.into()))
    } else {
        Ok(normalized_name)
    }
}

/// Thunk to avoid errors like
///  = note: `fn(&'2 str) -> Result<CustomToolchainName, <CustomToolchainName as TryFrom<&'2 str>>::Error> {<CustomToolchainName as TryFrom<&'2 str>>::try_from}` must implement `FnOnce<(&'1 str,)>`, for any lifetime `'1`...
/// = note: ...but it actually implements `FnOnce<(&'2 str,)>`, for some specific lifetime `'2`
pub(crate) fn partial_toolchain_desc_parser(
    value: &str,
) -> Result<PartialToolchainDesc, anyhow::Error> {
    value.parse::<PartialToolchainDesc>()
}

/// A toolchain name from user input.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ResolvableToolchainName {
    Custom(CustomToolchainName),
    Official(PartialToolchainDesc),
}

impl ResolvableToolchainName {
    /// Resolve to a concrete toolchain name
    pub fn resolve(&self, host: &TargetTriple) -> Result<ToolchainName, anyhow::Error> {
        match self.clone() {
            ResolvableToolchainName::Custom(c) => Ok(ToolchainName::Custom(c)),
            ResolvableToolchainName::Official(desc) => {
                let resolved = desc.resolve(host)?;
                Ok(ToolchainName::Official(resolved))
            }
        }
    }

    // If candidate could be resolved, return a ready to resolve version of it.
    // Otherwise error.
    fn validate(candidate: &str) -> Result<ResolvableToolchainName, InvalidName> {
        let candidate = validate(candidate)?;
        candidate
            .parse::<PartialToolchainDesc>()
            .map(ResolvableToolchainName::Official)
            .or_else(|_| {
                CustomToolchainName::try_from(candidate)
                    .map(ResolvableToolchainName::Custom)
                    .map_err(|_| InvalidName::ToolchainName(candidate.into()))
            })
    }
}

try_from_str!(ResolvableToolchainName);

impl From<&PartialToolchainDesc> for ResolvableToolchainName {
    fn from(value: &PartialToolchainDesc) -> Self {
        ResolvableToolchainName::Official(value.to_owned())
    }
}

impl Display for ResolvableToolchainName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolvableToolchainName::Custom(c) => write!(f, "{c}"),
            ResolvableToolchainName::Official(o) => write!(f, "{o}"),
        }
    }
}

/// Thunk to avoid errors like
///  = note: `fn(&'2 str) -> Result<CustomToolchainName, <CustomToolchainName as TryFrom<&'2 str>>::Error> {<CustomToolchainName as TryFrom<&'2 str>>::try_from}` must implement `FnOnce<(&'1 str,)>`, for any lifetime `'1`...
/// = note: ...but it actually implements `FnOnce<(&'2 str,)>`, for some specific lifetime `'2`
pub(crate) fn resolvable_toolchainame_parser(
    value: &str,
) -> Result<ResolvableToolchainName, InvalidName> {
    ResolvableToolchainName::try_from(value)
}

/// A toolchain name from user input. MaybeToolchainName accepts 'none' or a
/// custom or resolvable official name. Possibly this should be an Option with a
/// local trait for our needs.
#[derive(Debug, Clone)]
pub(crate) enum MaybeResolvableToolchainName {
    Some(ResolvableToolchainName),
    None,
}

impl MaybeResolvableToolchainName {
    // If candidate could be resolved, return a ready to resolve version of it.
    // Otherwise error.
    fn validate(candidate: &str) -> Result<MaybeResolvableToolchainName, InvalidName> {
        let candidate = validate(candidate)?;
        if candidate == "none" {
            Ok(MaybeResolvableToolchainName::None)
        } else {
            Ok(MaybeResolvableToolchainName::Some(
                ResolvableToolchainName::validate(candidate)?,
            ))
        }
    }
}

try_from_str!(MaybeResolvableToolchainName);

impl Display for MaybeResolvableToolchainName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MaybeResolvableToolchainName::Some(t) => write!(f, "{t}"),
            MaybeResolvableToolchainName::None => write!(f, "none"),
        }
    }
}

/// Thunk to avoid errors like
///  = note: `fn(&'2 str) -> Result<CustomToolchainName, <CustomToolchainName as TryFrom<&'2 str>>::Error> {<CustomToolchainName as TryFrom<&'2 str>>::try_from}` must implement `FnOnce<(&'1 str,)>`, for any lifetime `'1`...
/// = note: ...but it actually implements `FnOnce<(&'2 str,)>`, for some specific lifetime `'2`
pub(crate) fn maybe_resolvable_toolchainame_parser(
    value: &str,
) -> Result<MaybeResolvableToolchainName, InvalidName> {
    MaybeResolvableToolchainName::try_from(value)
}

/// ResolvableToolchainName + none, for overriding default-has-a-value
/// situations in the CLI with an official toolchain name or none
#[derive(Debug, Clone)]
pub(crate) enum MaybeOfficialToolchainName {
    None,
    Some(PartialToolchainDesc),
}

impl MaybeOfficialToolchainName {
    fn validate(candidate: &str) -> Result<MaybeOfficialToolchainName, InvalidName> {
        let candidate = validate(candidate)?;
        if candidate == "none" {
            Ok(MaybeOfficialToolchainName::None)
        } else {
            Ok(MaybeOfficialToolchainName::Some(
                validate(candidate)?
                    .parse::<PartialToolchainDesc>()
                    .map_err(|_| InvalidName::OfficialName(candidate.into()))?,
            ))
        }
    }
}

try_from_str!(MaybeOfficialToolchainName);

impl Display for MaybeOfficialToolchainName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MaybeOfficialToolchainName::None => write!(f, "none"),
            MaybeOfficialToolchainName::Some(t) => write!(f, "{t}"),
        }
    }
}

/// ToolchainName can be used in calls to Cfg that alter configuration,
/// like setting overrides, or that depend on configuration, like calculating
/// the toolchain directory.
#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub(crate) enum ToolchainName {
    Custom(CustomToolchainName),
    Official(ToolchainDesc),
}

impl ToolchainName {
    /// If the string is already resolved, allow direct conversion
    fn validate(candidate: &str) -> Result<Self, InvalidName> {
        let candidate = validate(candidate)?;
        candidate
            .parse::<ToolchainDesc>()
            .map(ToolchainName::Official)
            .or_else(|_| CustomToolchainName::try_from(candidate).map(ToolchainName::Custom))
            .map_err(|_| InvalidName::ToolchainName(candidate.into()))
    }
}

from_variant!(ToolchainDesc, ToolchainName, ToolchainName::Official);
from_variant!(CustomToolchainName, ToolchainName, ToolchainName::Custom);

try_from_str!(ToolchainName);

impl Display for ToolchainName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolchainName::Custom(t) => write!(f, "{t}"),
            ToolchainName::Official(t) => write!(f, "{t}"),
        }
    }
}

/// Sorts [`ToolchainName`]s in the following order:
/// 1. `stable`/`beta`/`nightly`-prefixed names, in this exact order.
/// 2. `X.Y.Z-suffix` names, sorted by semver rules on `X.Y.Z`, then by `suffix`.
/// 3. Other names, sorted alphanumerically.
pub(crate) fn toolchain_sort(v: &mut [ToolchainName]) {
    v.sort_by_key(|name| {
        let s = name.to_string();
        if s.starts_with("stable") {
            return (0, None, s);
        }
        if s.starts_with("beta") {
            return (1, None, s);
        }
        if s.starts_with("nightly") {
            return (2, None, s);
        }
        if let Some((ver_str, suffix)) = s.split_once('-') {
            if let Ok(ver) = semver::Version::parse(ver_str) {
                return (3, Some(ver), suffix.to_owned());
            }
        }
        (4, None, s)
    })
}

/// ResolvableLocalToolchainName is used to process values set in
/// RUSTUP_TOOLCHAIN: resolvable and resolved official names, custom names and
/// absolute paths.
#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub(crate) enum ResolvableLocalToolchainName {
    Named(ResolvableToolchainName),
    Path(PathBasedToolchainName),
}

impl ResolvableLocalToolchainName {
    /// Resolve to a concrete toolchain name
    pub fn resolve(&self, host: &TargetTriple) -> Result<LocalToolchainName, anyhow::Error> {
        match self.clone() {
            ResolvableLocalToolchainName::Named(t) => {
                Ok(LocalToolchainName::Named(t.resolve(host)?))
            }
            ResolvableLocalToolchainName::Path(t) => Ok(LocalToolchainName::Path(t)),
        }
    }

    /// Validates if the string is a resolvable toolchain, or a path based toolchain.
    fn validate(candidate: &str) -> Result<Self, InvalidName> {
        let candidate = validate(candidate)?;
        ResolvableToolchainName::try_from(candidate)
            .map(ResolvableLocalToolchainName::Named)
            .or_else(|_| {
                PathBasedToolchainName::try_from(&PathBuf::from(candidate) as &Path)
                    .map(ResolvableLocalToolchainName::Path)
            })
    }
}

try_from_str!(ResolvableLocalToolchainName);

impl Display for ResolvableLocalToolchainName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolvableLocalToolchainName::Named(t) => write!(f, "{t}"),
            ResolvableLocalToolchainName::Path(t) => write!(f, "{t}"),
        }
    }
}

pub(crate) fn resolvable_local_toolchainame_parser(
    value: &str,
) -> Result<ResolvableLocalToolchainName, InvalidName> {
    ResolvableLocalToolchainName::try_from(value)
}

/// LocalToolchainName can be used in calls to Cfg that alter configuration,
/// like setting overrides, or that depend on configuration, like calculating
/// the toolchain directory. It is not used to model the RUSTUP_TOOLCHAIN
/// variable, because that can take unresolved toolchain values that are not
/// invalid for referring to an installed toolchain.
#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub(crate) enum LocalToolchainName {
    Named(ToolchainName),
    Path(PathBasedToolchainName),
}

impl From<&ToolchainDesc> for LocalToolchainName {
    fn from(value: &ToolchainDesc) -> Self {
        ToolchainName::Official(value.to_owned()).into()
    }
}

impl From<&CustomToolchainName> for LocalToolchainName {
    fn from(value: &CustomToolchainName) -> Self {
        ToolchainName::Custom(value.to_owned()).into()
    }
}

impl From<CustomToolchainName> for LocalToolchainName {
    fn from(value: CustomToolchainName) -> Self {
        ToolchainName::Custom(value).into()
    }
}

from_variant!(ToolchainName, LocalToolchainName, LocalToolchainName::Named);
from_variant!(
    PathBasedToolchainName,
    LocalToolchainName,
    LocalToolchainName::Path
);

impl Display for LocalToolchainName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LocalToolchainName::Named(t) => write!(f, "{t}"),
            LocalToolchainName::Path(t) => write!(f, "{t}"),
        }
    }
}

/// A custom toolchain name, but not an official toolchain name
/// (e.g. my-custom-toolchain)
#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub(crate) struct CustomToolchainName(String);

impl CustomToolchainName {
    pub fn str(&self) -> &str {
        &self.0
    }

    fn validate(candidate: &str) -> Result<CustomToolchainName, InvalidName> {
        let candidate = validate(candidate)?;
        if candidate.parse::<PartialToolchainDesc>().is_ok()
            || candidate == "none"
            || candidate.contains('/')
            || candidate.contains('\\')
        {
            Err(InvalidName::CustomName(candidate.into()))
        } else {
            Ok(CustomToolchainName(candidate.into()))
        }
    }
}

try_from_str!(CustomToolchainName);

impl Display for CustomToolchainName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Thunk to avoid
///  = note: `fn(&'2 str) -> Result<CustomToolchainName, <CustomToolchainName as TryFrom<&'2 str>>::Error> {<CustomToolchainName as TryFrom<&'2 str>>::try_from}` must implement `FnOnce<(&'1 str,)>`, for any lifetime `'1`...
/// = note: ...but it actually implements `FnOnce<(&'2 str,)>`, for some specific lifetime `'2`
pub(crate) fn custom_toolchain_name_parser(
    value: &str,
) -> Result<CustomToolchainName, InvalidName> {
    CustomToolchainName::try_from(value)
}

/// An toolchain specified just via its path. Relative paths enable arbitrary
/// code execution in a rust dir, so as a partial mitigation is limited to
/// absolute paths.
#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub(crate) struct PathBasedToolchainName(PathBuf, String);

impl Display for PathBasedToolchainName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.display())
    }
}

impl TryFrom<&Path> for PathBasedToolchainName {
    type Error = InvalidName;

    fn try_from(value: &Path) -> std::result::Result<Self, Self::Error> {
        // if official || at least a single path component
        let as_str = value.display().to_string();
        if PartialToolchainDesc::from_str(&as_str).is_ok()
            || !(as_str.contains('/') || as_str.contains('\\'))
        {
            Err(InvalidName::PathToolchain(as_str))
        } else {
            // Perform minimal validation; there should at least be a `bin/` that might
            // contain things for us to run.
            if !value.is_absolute() {
                Err(InvalidName::PathToolchainRelative(as_str))
            } else if !value.join("bin").is_dir() {
                Err(InvalidName::ToolchainPath(as_str))
            } else {
                Ok(PathBasedToolchainName(value.into(), as_str))
            }
        }
    }
}

impl TryFrom<&LocalToolchainName> for PathBasedToolchainName {
    type Error = InvalidName;

    fn try_from(value: &LocalToolchainName) -> std::result::Result<Self, Self::Error> {
        match value {
            LocalToolchainName::Named(_) => Err(InvalidName::PathToolchain(format!("{value}"))),
            LocalToolchainName::Path(n) => Ok(n.clone()),
        }
    }
}

impl Deref for PathBasedToolchainName {
    type Target = PathBuf;

    fn deref(&self) -> &PathBuf {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use proptest::{collection::vec, prelude::*, string::string_regex};

    use crate::{
        dist::dist::PartialToolchainDesc,
        toolchain::names::{CustomToolchainName, ResolvableToolchainName, ToolchainName},
    };

    //Duplicated from triple.rs as a pragmatic step. TODO: remove duplication.
    static LIST_ARCHS: &[&str] = &[
        "i386",
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
        "powerpc64le",
        "riscv64gc",
        "s390x",
        "loongarch64",
    ];
    static LIST_OSES: &[&str] = &[
        "pc-windows",
        "unknown-linux",
        "apple-darwin",
        "unknown-netbsd",
        "apple-ios",
        "linux",
        "rumprun-netbsd",
        "unknown-freebsd",
        "unknown-illumos",
    ];
    static LIST_ENVS: &[&str] = &[
        "gnu",
        "gnux32",
        "msvc",
        "gnueabi",
        "gnueabihf",
        "gnuabi64",
        "androideabi",
        "android",
        "musl",
    ];

    fn partial_toolchain_desc_re() -> String {
        let triple_re = format!(
            r"(-({}))?(?:-({}))?(?:-({}))?",
            LIST_ARCHS.join("|"),
            LIST_OSES.join("|"),
            LIST_ENVS.join("|")
        );
        let partial_toolchain_desc_re = format!(
            r"(nightly|beta|stable|\d{{1}}\.\d{{1,3}}(\.\d{{1,2}})?)(-(\d{{4}}-\d{{2}}-\d{{2}}))?{triple_re}"
        );

        partial_toolchain_desc_re
    }

    prop_compose! {
        fn arb_partial_toolchain_desc()
            (s in string_regex(&partial_toolchain_desc_re()).unwrap()) -> String {
            s
        }
    }

    prop_compose! {
        fn arb_custom_name()
            (s in r"[^\\/]+") -> String {
                // perhaps need to filter 'none' and partial toolchains - but they won't typically be generated anyway.
                s
        }
    }

    prop_compose! {
        fn arb_resolvable_name()
            (case in (0..=1), desc in arb_custom_name(), name in arb_partial_toolchain_desc() ) -> String {
                match case  {
                    0 => name,
                    _d => desc
                }
            }
    }

    prop_compose! {
        fn arb_abspath_name()
            (case in (0..=1), segments in vec("[^\\/]", 0..5)) -> String {
                match case {
                    0 => format!("/{}", segments.join("/")),
                    _ => format!(r"c:\{}", segments.join(r"\"))
                }
        }
    }

    proptest! {
        #[test]
        fn test_parse_partial_desc(desc in arb_partial_toolchain_desc()) {
            PartialToolchainDesc::from_str(&desc).unwrap();
        }

        #[test]
        fn test_parse_custom(name in arb_custom_name()) {
            CustomToolchainName::try_from(name).unwrap();
        }

        #[test]
        fn test_parse_resolvable_name(name in arb_resolvable_name()) {
            ResolvableToolchainName::try_from(name).unwrap();
        }

        // TODO: This needs some thought
        // #[test]
        // fn test_parse_abs_path_name(name in arb_abspath_name()) {
        //     let tempdir = tempfile::Builder::new().tempdir().unwrap();
        //     let d = tempdir.into_path();
        //     fs::create_dir(d.create_directory("bin").unwrap()).unwrap();
        // // .into_path())

        //     PathBasedToolchainName::try_from(Path::new(&name)).unwrap();
        // }

    }

    #[test]
    fn test_toolchain_sort() {
        let expected = vec![
            "stable-x86_64-unknown-linux-gnu",
            "beta-x86_64-unknown-linux-gnu",
            "nightly-x86_64-unknown-linux-gnu",
            "1.0.0-x86_64-unknown-linux-gnu",
            "1.2.0-x86_64-unknown-linux-gnu",
            "1.8.0-x86_64-unknown-linux-gnu",
            "1.10.0-x86_64-unknown-linux-gnu",
            "bar(baz)",
            "foo#bar",
            "this.is.not-a+semver",
        ]
        .into_iter()
        .map(|s| ToolchainName::try_from(s).unwrap())
        .collect::<Vec<_>>();

        let mut v = vec![
            "1.8.0-x86_64-unknown-linux-gnu",
            "1.0.0-x86_64-unknown-linux-gnu",
            "nightly-x86_64-unknown-linux-gnu",
            "stable-x86_64-unknown-linux-gnu",
            "1.10.0-x86_64-unknown-linux-gnu",
            "beta-x86_64-unknown-linux-gnu",
            "1.2.0-x86_64-unknown-linux-gnu",
            // https://github.com/rust-lang/rustup/issues/3517
            "foo#bar",
            "bar(baz)",
            "this.is.not-a+semver",
        ]
        .into_iter()
        .map(|s| ToolchainName::try_from(s).unwrap())
        .collect::<Vec<_>>();

        super::toolchain_sort(&mut v);

        assert_eq!(expected, v);
    }
}
