//! Rust distribution v2 manifests.
//!
//! This manifest describes the distributable artifacts for a single
//! release of Rust. They are toml files, typically downloaded from
//! e.g. static.rust-lang.org/dist/channel-rust-nightly.toml. They
//! describe where to download, for all platforms, each component of
//! the release, and their relationships to each other.
//!
//! Installers use this info to customize Rust installations.
//!
//! See tests/channel-rust-nightly-example.toml for an example.
//!
//! Docs: <https://forge.rust-lang.org/infra/channel-layout.html>

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

use anyhow::{anyhow, bail, Context, Result};

use crate::dist::dist::{PartialTargetTriple, Profile, TargetTriple};
use crate::errors::*;
use crate::utils::toml_utils::*;

pub(crate) const SUPPORTED_MANIFEST_VERSIONS: [&str; 1] = ["2"];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Manifest {
    manifest_version: String,
    pub date: String,
    pub packages: HashMap<String, Package>,
    pub renames: HashMap<String, String>,
    pub reverse_renames: HashMap<String, String>,
    profiles: HashMap<Profile, Vec<String>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Package {
    pub version: String,
    pub targets: PackageTargets,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PackageTargets {
    Wildcard(TargetedPackage),
    Targeted(HashMap<TargetTriple, TargetedPackage>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetedPackage {
    pub bins: Vec<(CompressionKind, HashedBinary)>,
    pub components: Vec<Component>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompressionKind {
    GZip,
    XZ,
    ZStd,
}

/// Each compression kind, in order of preference for use, from most desirable
/// to least desirable.
static COMPRESSION_KIND_PREFERENCE_ORDER: &[CompressionKind] = &[
    CompressionKind::ZStd,
    CompressionKind::XZ,
    CompressionKind::GZip,
];

impl CompressionKind {
    const fn key_prefix(self) -> &'static str {
        match self {
            Self::GZip => "",
            Self::XZ => "xz_",
            Self::ZStd => "zst_",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HashedBinary {
    pub url: String,
    pub hash: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialOrd)]
pub struct Component {
    pkg: String,
    pub target: Option<TargetTriple>,
    // Older Rustup distinguished between components (which are essential) and
    // extensions (which are not).
    is_extension: bool,
}

impl PartialEq for Component {
    fn eq(&self, other: &Self) -> bool {
        self.pkg == other.pkg && self.target == other.target
    }
}

impl Hash for Component {
    fn hash<H>(&self, hasher: &mut H)
    where
        H: Hasher,
    {
        self.pkg.hash(hasher);
        self.target.hash(hasher);
    }
}

impl Manifest {
    pub fn parse(data: &str) -> Result<Self> {
        let value = toml::from_str(data).context("error parsing manifest")?;
        let manifest = Self::from_toml(value, "")?;
        manifest.validate()?;

        Ok(manifest)
    }
    pub fn stringify(self) -> String {
        self.into_toml().to_string()
    }

    pub(crate) fn from_toml(mut table: toml::value::Table, path: &str) -> Result<Self> {
        let version = get_string(&mut table, "manifest-version", path)?;
        if !SUPPORTED_MANIFEST_VERSIONS.contains(&&*version) {
            bail!(RustupError::UnsupportedVersion(version));
        }
        let (renames, reverse_renames) = Self::table_to_renames(&mut table, path)?;
        Ok(Self {
            manifest_version: version,
            date: get_string(&mut table, "date", path)?,
            packages: Self::table_to_packages(&mut table, path)?,
            renames,
            reverse_renames,
            profiles: Self::table_to_profiles(&mut table, path)?,
        })
    }
    pub(crate) fn into_toml(self) -> toml::value::Table {
        let mut result = toml::value::Table::new();

        result.insert("date".to_owned(), toml::Value::String(self.date));
        result.insert(
            "manifest-version".to_owned(),
            toml::Value::String(self.manifest_version),
        );

        let renames = Self::renames_to_table(self.renames);
        result.insert("renames".to_owned(), toml::Value::Table(renames));

        let packages = Self::packages_to_table(self.packages);
        result.insert("pkg".to_owned(), toml::Value::Table(packages));

        let profiles = Self::profiles_to_table(self.profiles);
        result.insert("profiles".to_owned(), toml::Value::Table(profiles));

        result
    }

    fn table_to_packages(
        table: &mut toml::value::Table,
        path: &str,
    ) -> Result<HashMap<String, Package>> {
        let mut result = HashMap::new();
        let pkg_table = get_table(table, "pkg", path)?;

        for (k, v) in pkg_table {
            if let toml::Value::Table(t) = v {
                result.insert(k, Package::from_toml(t, path)?);
            }
        }

        Ok(result)
    }
    fn packages_to_table(packages: HashMap<String, Package>) -> toml::value::Table {
        let mut result = toml::value::Table::new();
        for (k, v) in packages {
            result.insert(k, toml::Value::Table(v.into_toml()));
        }
        result
    }

    fn table_to_renames(
        table: &mut toml::value::Table,
        path: &str,
    ) -> Result<(HashMap<String, String>, HashMap<String, String>)> {
        let mut renames = HashMap::new();
        let mut reverse_renames = HashMap::new();
        let renames_table = get_table(table, "renames", path)?;

        for (k, v) in renames_table {
            if let toml::Value::Table(mut t) = v {
                let to = get_string(&mut t, "to", path)?;
                renames.insert(k.to_owned(), to.clone());
                reverse_renames.insert(to, k.to_owned());
            }
        }

        Ok((renames, reverse_renames))
    }
    fn renames_to_table(renames: HashMap<String, String>) -> toml::value::Table {
        let mut result = toml::value::Table::new();
        for (from, to) in renames {
            let mut table = toml::value::Table::new();
            table.insert("to".to_owned(), toml::Value::String(to));
            result.insert(from, toml::Value::Table(table));
        }
        result
    }

    fn table_to_profiles(
        table: &mut toml::value::Table,
        path: &str,
    ) -> Result<HashMap<Profile, Vec<String>>> {
        let mut result = HashMap::new();
        let profile_table = match get_table(table, "profiles", path) {
            Ok(t) => t,
            Err(_) => return Ok(result),
        };

        for (k, v) in profile_table {
            if let toml::Value::Array(a) = v {
                let values = a
                    .into_iter()
                    .filter_map(|v| match v {
                        toml::Value::String(s) => Some(s),
                        _ => None,
                    })
                    .collect();
                result.insert(Profile::from_str(&k)?, values);
            }
        }

        Ok(result)
    }
    fn profiles_to_table(profiles: HashMap<Profile, Vec<String>>) -> toml::value::Table {
        let mut result = toml::value::Table::new();
        for (profile, values) in profiles {
            let array = values.into_iter().map(toml::Value::String).collect();
            result.insert(profile.to_string(), toml::Value::Array(array));
        }
        result
    }

    pub fn get_package(&self, name: &str) -> Result<&Package> {
        self.packages
            .get(name)
            .ok_or_else(|| anyhow!(format!("package not found: '{name}'")))
    }

    pub(crate) fn get_rust_version(&self) -> Result<&str> {
        self.get_package("rust").map(|p| &*p.version)
    }

    pub(crate) fn get_legacy_components(&self, target: &TargetTriple) -> Result<Vec<Component>> {
        // Build a profile from the components/extensions.
        let result = self
            .get_package("rust")?
            .get_target(Some(target))?
            .components
            .iter()
            .filter(|c| !c.is_extension && c.target.as_ref().map(|t| t == target).unwrap_or(true))
            .cloned()
            .collect();

        Ok(result)
    }
    pub fn get_profile_components(
        &self,
        profile: Profile,
        target: &TargetTriple,
    ) -> Result<Vec<Component>> {
        // An older manifest with no profiles section.
        if self.profiles.is_empty() {
            return self.get_legacy_components(target);
        }

        let profile = self
            .profiles
            .get(&profile)
            .ok_or_else(|| anyhow!(format!("profile not found: '{profile}'")))?;

        let rust_pkg = self.get_package("rust")?.get_target(Some(target))?;
        let result = profile
            .iter()
            .map(|s| {
                (
                    s,
                    rust_pkg.components.iter().find(|c| {
                        &c.pkg == s && c.target.as_ref().map(|t| t == target).unwrap_or(true)
                    }),
                )
            })
            .filter(|(_, c)| c.is_some())
            .map(|(s, c)| Component::new(s.to_owned(), c.and_then(|c| c.target.clone()), false))
            .collect();
        Ok(result)
    }

    fn validate_targeted_package(&self, tpkg: &TargetedPackage) -> Result<()> {
        for c in tpkg.components.iter() {
            let cpkg = self
                .get_package(&c.pkg)
                .with_context(|| RustupError::MissingPackageForComponent(c.short_name(self)))?;
            let _ctpkg = cpkg
                .get_target(c.target.as_ref())
                .with_context(|| RustupError::MissingPackageForComponent(c.short_name(self)))?;
        }
        Ok(())
    }

    fn validate(&self) -> Result<()> {
        // Every component mentioned must have an actual package to download
        for pkg in self.packages.values() {
            match pkg.targets {
                PackageTargets::Wildcard(ref tpkg) => {
                    self.validate_targeted_package(tpkg)?;
                }
                PackageTargets::Targeted(ref tpkgs) => {
                    for tpkg in tpkgs.values() {
                        self.validate_targeted_package(tpkg)?;
                    }
                }
            }
        }

        // The target of any renames must be an actual package. The subject of
        // renames is unconstrained.
        for name in self.renames.values() {
            if !self.packages.contains_key(name) {
                bail!(format!(
                    "server sent a broken manifest: missing package for the target of a rename {name}"
                ));
            }
        }

        Ok(())
    }

    // If the component should be renamed by this manifest, then return a new
    // component with the new name. If not, return `None`.
    pub(crate) fn rename_component(&self, component: &Component) -> Option<Component> {
        self.renames.get(&component.pkg).map(|r| {
            let mut c = component.clone();
            c.pkg = r.clone();
            c
        })
    }
}

impl Package {
    pub(crate) fn from_toml(mut table: toml::value::Table, path: &str) -> Result<Self> {
        Ok(Self {
            version: get_string(&mut table, "version", path)?,
            targets: Self::toml_to_targets(table, path)?,
        })
    }
    pub(crate) fn into_toml(self) -> toml::value::Table {
        let mut result = toml::value::Table::new();

        result.insert("version".to_owned(), toml::Value::String(self.version));

        let targets = Self::targets_to_toml(self.targets);
        result.insert("target".to_owned(), toml::Value::Table(targets));

        result
    }

    fn toml_to_targets(mut table: toml::value::Table, path: &str) -> Result<PackageTargets> {
        let mut target_table = get_table(&mut table, "target", path)?;

        if let Some(toml::Value::Table(t)) = target_table.remove("*") {
            Ok(PackageTargets::Wildcard(TargetedPackage::from_toml(
                t, path,
            )?))
        } else {
            let mut result = HashMap::new();
            for (k, v) in target_table {
                if let toml::Value::Table(t) = v {
                    result.insert(TargetTriple::new(&k), TargetedPackage::from_toml(t, path)?);
                }
            }
            Ok(PackageTargets::Targeted(result))
        }
    }
    fn targets_to_toml(targets: PackageTargets) -> toml::value::Table {
        let mut result = toml::value::Table::new();
        match targets {
            PackageTargets::Wildcard(tpkg) => {
                result.insert("*".to_owned(), toml::Value::Table(tpkg.into_toml()));
            }
            PackageTargets::Targeted(tpkgs) => {
                for (k, v) in tpkgs {
                    result.insert(k.to_string(), toml::Value::Table(v.into_toml()));
                }
            }
        }
        result
    }

    pub fn get_target(&self, target: Option<&TargetTriple>) -> Result<&TargetedPackage> {
        match self.targets {
            PackageTargets::Wildcard(ref tpkg) => Ok(tpkg),
            PackageTargets::Targeted(ref tpkgs) => {
                if let Some(t) = target {
                    tpkgs
                        .get(t)
                        .ok_or_else(|| anyhow!(format!("target '{t}' not found in channel.  \
                        Perhaps check https://doc.rust-lang.org/nightly/rustc/platform-support.html for available targets")))
                } else {
                    Err(anyhow!("no target specified"))
                }
            }
        }
    }
}

impl PackageTargets {
    pub(crate) fn get<'a>(&'a self, target: &TargetTriple) -> Option<&'a TargetedPackage> {
        match self {
            Self::Wildcard(tpkg) => Some(tpkg),
            Self::Targeted(tpkgs) => tpkgs.get(target),
        }
    }
    pub fn get_mut<'a>(&'a mut self, target: &TargetTriple) -> Option<&'a mut TargetedPackage> {
        match self {
            Self::Wildcard(tpkg) => Some(tpkg),
            Self::Targeted(tpkgs) => tpkgs.get_mut(target),
        }
    }
}

impl TargetedPackage {
    pub(crate) fn from_toml(mut table: toml::value::Table, path: &str) -> Result<Self> {
        let components = get_array(&mut table, "components", path)?;
        let extensions = get_array(&mut table, "extensions", path)?;

        let mut components =
            Self::toml_to_components(components, &format!("{}{}.", path, "components"), false)?;
        components.append(&mut Self::toml_to_components(
            extensions,
            &format!("{}{}.", path, "extensions"),
            true,
        )?);

        if get_bool(&mut table, "available", path)? {
            let mut bins = Vec::new();
            for kind in COMPRESSION_KIND_PREFERENCE_ORDER.iter().copied() {
                let url_key = format!("{}url", kind.key_prefix());
                let hash_key = format!("{}hash", kind.key_prefix());
                let url = get_string(&mut table, &url_key, path).ok();
                let hash = get_string(&mut table, &hash_key, path).ok();
                if let (Some(url), Some(hash)) = (url, hash) {
                    bins.push((kind, HashedBinary { url, hash }));
                }
            }
            Ok(Self { bins, components })
        } else {
            Ok(Self {
                bins: Vec::new(),
                components: Vec::new(),
            })
        }
    }
    pub(crate) fn into_toml(self) -> toml::value::Table {
        let mut result = toml::value::Table::new();
        let (components, extensions) = Self::components_to_toml(self.components);
        if !components.is_empty() {
            result.insert("components".to_owned(), toml::Value::Array(components));
        }
        if !extensions.is_empty() {
            result.insert("extensions".to_owned(), toml::Value::Array(extensions));
        }
        if self.bins.is_empty() {
            result.insert("available".to_owned(), toml::Value::Boolean(false));
        } else {
            for (kind, bin) in self.bins {
                let url_key = format!("{}url", kind.key_prefix());
                let hash_key = format!("{}hash", kind.key_prefix());
                result.insert(url_key, toml::Value::String(bin.url));
                result.insert(hash_key, toml::Value::String(bin.hash));
            }
            result.insert("available".to_owned(), toml::Value::Boolean(true));
        }
        result
    }

    pub fn available(&self) -> bool {
        !self.bins.is_empty()
    }

    fn toml_to_components(
        arr: toml::value::Array,
        path: &str,
        is_extension: bool,
    ) -> Result<Vec<Component>> {
        let mut result = Vec::new();

        for (i, v) in arr.into_iter().enumerate() {
            if let toml::Value::Table(t) = v {
                let path = format!("{path}[{i}]");
                result.push(Component::from_toml(t, &path, is_extension)?);
            }
        }

        Ok(result)
    }
    fn components_to_toml(data: Vec<Component>) -> (toml::value::Array, toml::value::Array) {
        let mut components = toml::value::Array::new();
        let mut extensions = toml::value::Array::new();
        for v in data {
            if v.is_extension {
                extensions.push(toml::Value::Table(v.into_toml()));
            } else {
                components.push(toml::Value::Table(v.into_toml()));
            }
        }
        (components, extensions)
    }
}

impl Component {
    pub fn new(pkg: String, target: Option<TargetTriple>, is_extension: bool) -> Self {
        Self {
            pkg,
            target,
            is_extension,
        }
    }

    pub(crate) fn new_with_target(pkg_with_target: &str, is_extension: bool) -> Option<Self> {
        for (pos, _) in pkg_with_target.match_indices('-') {
            let pkg = &pkg_with_target[0..pos];
            let target = &pkg_with_target[pos + 1..];
            if let Some(partial) = PartialTargetTriple::new(target) {
                if let Ok(triple) = TargetTriple::try_from(partial) {
                    return Some(Self {
                        pkg: pkg.to_string(),
                        target: Some(triple),
                        is_extension,
                    });
                }
            }
        }
        None
    }

    pub(crate) fn wildcard(&self) -> Self {
        Self {
            pkg: self.pkg.clone(),
            target: None,
            is_extension: false,
        }
    }
    pub(crate) fn from_toml(
        mut table: toml::value::Table,
        path: &str,
        is_extension: bool,
    ) -> Result<Self> {
        Ok(Self {
            pkg: get_string(&mut table, "pkg", path)?,
            target: get_string(&mut table, "target", path).map(|s| {
                if s == "*" {
                    None
                } else {
                    Some(TargetTriple::new(&s))
                }
            })?,
            is_extension,
        })
    }
    pub(crate) fn into_toml(self) -> toml::value::Table {
        let mut result = toml::value::Table::new();
        result.insert(
            "target".to_owned(),
            toml::Value::String(
                self.target
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "*".to_owned()),
            ),
        );
        result.insert("pkg".to_owned(), toml::Value::String(self.pkg));
        result
    }
    pub(crate) fn name(&self, manifest: &Manifest) -> String {
        let pkg = self.short_name(manifest);
        if let Some(ref t) = self.target {
            format!("{pkg}-{t}")
        } else {
            pkg
        }
    }
    pub(crate) fn short_name(&self, manifest: &Manifest) -> String {
        if let Some(from) = manifest.reverse_renames.get(&self.pkg) {
            from.to_owned()
        } else {
            self.pkg.clone()
        }
    }
    pub(crate) fn description(&self, manifest: &Manifest) -> String {
        let pkg = self.short_name(manifest);
        if let Some(ref t) = self.target {
            format!("'{pkg}' for target '{t}'")
        } else {
            format!("'{pkg}'")
        }
    }
    pub fn short_name_in_manifest(&self) -> &String {
        &self.pkg
    }
    pub(crate) fn name_in_manifest(&self) -> String {
        let pkg = self.short_name_in_manifest();
        if let Some(ref t) = self.target {
            format!("{pkg}-{t}")
        } else {
            pkg.to_string()
        }
    }
    pub(crate) fn target(&self) -> String {
        if let Some(t) = self.target.as_ref() {
            t.to_string()
        } else {
            String::new()
        }
    }

    pub(crate) fn contained_within(&self, components: &[Component]) -> bool {
        if components.contains(self) {
            // Yes, we're within the component set, move on
            true
        } else if self.target.is_none() {
            // We weren't in the given component set, but we're a package
            // which targets "*" and as such older rustups might have
            // accidentally made us target specific due to a bug in profiles.
            components
                .iter()
                // As such, if our target is None, it's sufficient to check pkg
                .any(|other| other.pkg == self.pkg)
        } else {
            // As a last ditch effort, we're contained within the component
            // set if the name matches and the other component's target
            // is None
            components
                .iter()
                .any(|other| other.pkg == self.pkg && other.target.is_none())
        }
    }
}
