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

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::{
    dist::{config::Config, Profile, TargetTriple, ToolchainDesc},
    errors::*,
    toolchain::DistributableToolchain,
};

/// Used by the `installed_components` function
pub(crate) struct ComponentStatus {
    pub component: Component,
    pub name: String,
    pub installed: bool,
    pub available: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct Manifest {
    pub(crate) manifest_version: ManifestVersion,
    pub date: String,
    #[serde(default, rename = "pkg")]
    pub packages: HashMap<String, Package>,
    #[serde(default)]
    pub renames: HashMap<String, Renamed>,
    #[serde(default, skip_serializing)]
    pub reverse_renames: HashMap<String, String>,
    #[serde(default)]
    pub profiles: HashMap<Profile, Vec<String>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Renamed {
    pub to: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Package {
    pub version: String,
    #[serde(rename = "target")]
    pub targets: PackageTargets,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(from = "TargetsMap", into = "TargetsMap")]
pub enum PackageTargets {
    Wildcard(TargetedPackage),
    Targeted(HashMap<TargetTriple, TargetedPackage>),
}

#[derive(Deserialize, Serialize)]
#[serde(transparent)]
struct TargetsMap(HashMap<TargetTriple, TargetedPackage>);

impl From<TargetsMap> for PackageTargets {
    fn from(mut map: TargetsMap) -> Self {
        let wildcard = TargetTriple::new("*");
        match (map.0.len(), map.0.entry(wildcard)) {
            (1, Entry::Occupied(entry)) => Self::Wildcard(entry.remove()),
            (_, _) => Self::Targeted(map.0),
        }
    }
}

impl From<PackageTargets> for TargetsMap {
    fn from(targets: PackageTargets) -> Self {
        match targets {
            PackageTargets::Wildcard(tpkg) => {
                let mut map = HashMap::new();
                map.insert(TargetTriple::new("*"), tpkg);
                Self(map)
            }
            PackageTargets::Targeted(tpkgs) => Self(tpkgs),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(from = "Target", into = "Target")]
pub struct TargetedPackage {
    #[serde(default)]
    pub bins: Vec<HashedBinary>,
    pub components: Vec<Component>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Target {
    available: bool,
    url: Option<String>,
    hash: Option<String>,
    xz_url: Option<String>,
    xz_hash: Option<String>,
    zst_url: Option<String>,
    zst_hash: Option<String>,
    components: Option<Vec<Component>>,
    extensions: Option<Vec<Component>>,
}

impl From<Target> for TargetedPackage {
    fn from(target: Target) -> Self {
        let mut components = target.components.unwrap_or_default();
        if let Some(extensions) = target.extensions {
            components.extend(extensions.into_iter().map(|mut c| {
                c.is_extension = true;
                c
            }));
        }

        let mut bins = Vec::new();
        if !target.available {
            return Self { bins, components };
        }

        if let (Some(url), Some(hash)) = (target.zst_url, target.zst_hash) {
            bins.push(HashedBinary {
                url,
                hash,
                compression: CompressionKind::ZStd,
            });
        }

        if let (Some(url), Some(hash)) = (target.xz_url, target.xz_hash) {
            bins.push(HashedBinary {
                url,
                hash,
                compression: CompressionKind::XZ,
            });
        }

        if let (Some(url), Some(hash)) = (target.url, target.hash) {
            bins.push(HashedBinary {
                url,
                hash,
                compression: CompressionKind::GZip,
            });
        }

        Self { bins, components }
    }
}

impl From<TargetedPackage> for Target {
    fn from(tpkg: TargetedPackage) -> Self {
        let (mut url, mut hash) = (None, None);
        let (mut xz_url, mut xz_hash) = (None, None);
        let (mut zst_url, mut zst_hash) = (None, None);
        let available = !tpkg.bins.is_empty();
        for bin in tpkg.bins {
            match bin.compression {
                CompressionKind::GZip => {
                    url = Some(bin.url);
                    hash = Some(bin.hash);
                }
                CompressionKind::XZ => {
                    xz_url = Some(bin.url);
                    xz_hash = Some(bin.hash);
                }
                CompressionKind::ZStd => {
                    zst_url = Some(bin.url);
                    zst_hash = Some(bin.hash);
                }
            }
        }

        let (mut components, mut extensions) =
            (Vec::with_capacity(tpkg.components.len()), Vec::new());
        for c in tpkg.components {
            match c.is_extension {
                true => &mut extensions,
                false => &mut components,
            }
            .push(c);
        }

        Self {
            available,
            url,
            hash,
            xz_url,
            xz_hash,
            zst_url,
            zst_hash,
            components: Some(components),
            extensions: Some(extensions),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CompressionKind {
    GZip,
    XZ,
    ZStd,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HashedBinary {
    pub url: String,
    pub hash: String,
    pub compression: CompressionKind,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialOrd, Serialize)]
pub struct Component {
    pub pkg: String,
    #[serde(with = "component_target")]
    pub target: Option<TargetTriple>,
    // Older Rustup distinguished between components (which are essential) and
    // extensions (which are not).
    #[serde(default)]
    pub is_extension: bool,
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

mod component_target {
    use super::*;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(
        target: &Option<TargetTriple>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(match target {
            Some(t) => t,
            None => "*",
        })
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<TargetTriple>, D::Error> {
        Ok(match Option::<String>::deserialize(deserializer)? {
            Some(s) if s != "*" => Some(TargetTriple::new(s)),
            _ => None,
        })
    }
}

impl Manifest {
    pub fn parse(data: &str) -> Result<Self> {
        let mut manifest = toml::from_str::<Self>(data).context("error parsing manifest")?;
        for (from, to) in manifest.renames.iter() {
            manifest.reverse_renames.insert(to.to.clone(), from.clone());
        }

        manifest.validate()?;
        Ok(manifest)
    }

    pub fn stringify(self) -> anyhow::Result<String> {
        Ok(toml::to_string(&self)?)
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
        for renamed in self.renames.values() {
            let name = &renamed.to;
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
            c.pkg.clone_from(&r.to);
            c
        })
    }

    /// Determine installed components from an installed manifest.
    pub(crate) fn query_components(
        &self,
        desc: &ToolchainDesc,
        config: &Config,
    ) -> Result<Vec<ComponentStatus>> {
        // Return all optional components of the "rust" package for the
        // toolchain's target triple.
        let mut res = Vec::new();

        let rust_pkg = self
            .packages
            .get("rust")
            .expect("manifest should contain a rust package");
        let targ_pkg = rust_pkg
            .targets
            .get(&desc.target)
            .expect("installed manifest should have a known target");

        for component in &targ_pkg.components {
            let installed = component.contained_within(&config.components);

            let component_target = TargetTriple::new(component.target());

            // Get the component so we can check if it is available
            let component_pkg = self
                .get_package(component.short_name_in_manifest())
                .unwrap_or_else(|_| {
                    panic!(
                        "manifest should contain component {}",
                        &component.short_name(self)
                    )
                });
            let component_target_pkg = component_pkg
                .targets
                .get(&component_target)
                .expect("component should have target toolchain");

            res.push(ComponentStatus {
                component: component.clone(),
                name: component.name(self),
                installed,
                available: component_target_pkg.available(),
            });
        }

        res.sort_by(|a, b| a.component.cmp(&b.component));

        Ok(res)
    }
}

impl Package {
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
    pub fn available(&self) -> bool {
        !self.bins.is_empty()
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

    pub(crate) fn try_new(
        name: &str,
        distributable: &DistributableToolchain<'_>,
        fallback_target: Option<&TargetTriple>,
    ) -> Result<Self> {
        let manifest = distributable.get_manifest()?;
        for component_status in distributable.components()? {
            let component = component_status.component;
            if name == component.name_in_manifest() || name == component.name(&manifest) {
                return Ok(component);
            }
        }

        Ok(Component::new(
            name.to_string(),
            fallback_target.cloned(),
            true,
        ))
    }

    pub(crate) fn wildcard(&self) -> Self {
        Self {
            pkg: self.pkg.clone(),
            target: None,
            is_extension: false,
        }
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

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) enum ManifestVersion {
    #[serde(rename = "2")]
    #[default]
    V2,
}

impl ManifestVersion {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::V2 => "2",
        }
    }
}

impl FromStr for ManifestVersion {
    type Err = RustupError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "2" => Ok(Self::V2),
            _ => Err(RustupError::UnsupportedVersion(s.to_owned())),
        }
    }
}

impl fmt::Display for ManifestVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use crate::dist::manifest::Manifest;
    use crate::dist::TargetTriple;
    use crate::RustupError;

    // Example manifest from https://public.etherpad-mozilla.org/p/Rust-infra-work-week
    static EXAMPLE: &str = include_str!("manifest/tests/channel-rust-nightly-example.toml");
    // From brson's live build-rust-manifest.py script
    static EXAMPLE2: &str = include_str!("manifest/tests/channel-rust-nightly-example2.toml");

    #[test]
    fn parse_smoke_test() {
        let x86_64_unknown_linux_gnu = TargetTriple::new("x86_64-unknown-linux-gnu");
        let x86_64_unknown_linux_musl = TargetTriple::new("x86_64-unknown-linux-musl");

        let pkg = Manifest::parse(EXAMPLE).unwrap();

        pkg.get_package("rust").unwrap();
        pkg.get_package("rustc").unwrap();
        pkg.get_package("cargo").unwrap();
        pkg.get_package("rust-std").unwrap();
        pkg.get_package("rust-docs").unwrap();

        let rust_pkg = pkg.get_package("rust").unwrap();
        assert!(rust_pkg.version.contains("1.3.0"));

        let rust_target_pkg = rust_pkg
            .get_target(Some(&x86_64_unknown_linux_gnu))
            .unwrap();
        assert!(rust_target_pkg.available());
        assert_eq!(rust_target_pkg.bins[0].url, "example.com");
        assert_eq!(rust_target_pkg.bins[0].hash, "...");

        let component = &rust_target_pkg.components[0];
        assert_eq!(component.short_name_in_manifest(), "rustc");
        assert_eq!(component.target.as_ref(), Some(&x86_64_unknown_linux_gnu));

        let component = &rust_target_pkg.components[4];
        assert_eq!(component.short_name_in_manifest(), "rust-std");
        assert_eq!(component.target.as_ref(), Some(&x86_64_unknown_linux_musl));

        let docs_pkg = pkg.get_package("rust-docs").unwrap();
        let docs_target_pkg = docs_pkg
            .get_target(Some(&x86_64_unknown_linux_gnu))
            .unwrap();
        assert_eq!(docs_target_pkg.bins[0].url, "example.com");
    }

    #[test]
    fn renames() {
        let manifest = Manifest::parse(EXAMPLE2).unwrap();
        assert_eq!(1, manifest.renames.len());
        assert_eq!(manifest.renames["cargo-old"].to, "cargo");
        assert_eq!(1, manifest.reverse_renames.len());
        assert_eq!(manifest.reverse_renames["cargo"], "cargo-old");
    }

    #[test]
    fn parse_round_trip() {
        let original = Manifest::parse(EXAMPLE).unwrap();
        let serialized = original.clone().stringify().unwrap();
        let new = Manifest::parse(&serialized).unwrap();
        assert_eq!(original, new);

        let original = Manifest::parse(EXAMPLE2).unwrap();
        let serialized = original.clone().stringify().unwrap();
        let new = Manifest::parse(&serialized).unwrap();
        assert_eq!(original, new);
    }

    #[test]
    fn validate_components_have_corresponding_packages() {
        let manifest = r#"
manifest-version = "2"
date = "2015-10-10"
[pkg.rust]
  version = "rustc 1.3.0 (9a92aaf19 2015-09-15)"
  [pkg.rust.target.x86_64-unknown-linux-gnu]
    available = true
    url = "example.com"
    hash = "..."
    [[pkg.rust.target.x86_64-unknown-linux-gnu.components]]
      pkg = "rustc"
      target = "x86_64-unknown-linux-gnu"
    [[pkg.rust.target.x86_64-unknown-linux-gnu.extensions]]
      pkg = "rust-std"
      target = "x86_64-unknown-linux-musl"
[pkg.rustc]
  version = "rustc 1.3.0 (9a92aaf19 2015-09-15)"
  [pkg.rustc.target.x86_64-unknown-linux-gnu]
    available = true
    url = "example.com"
    hash = "..."
"#;

        let err = Manifest::parse(manifest).unwrap_err();

        match err.downcast::<RustupError>().unwrap() {
            RustupError::MissingPackageForComponent(_) => {}
            _ => panic!(),
        }
    }

    // #248
    #[test]
    fn manifest_can_contain_unknown_targets() {
        let manifest = EXAMPLE.replace("x86_64-unknown-linux-gnu", "mycpu-myvendor-myos");

        assert!(Manifest::parse(&manifest).is_ok());
    }
}
