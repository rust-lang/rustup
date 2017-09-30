//! Rust distribution v2 manifests.
//!
//! This manifest describes the distributable artifacts for a single
//! release of Rust. They are toml files, typically downloaded from
//! e.g. static.rust-lang.org/dist/channel-rust-nightly.toml. They
//! describe where to download, for all platforms, each component of
//! the a release, and their relationships to each other.
//!
//! Installers use this info to customize Rust installations.
//!
//! See tests/channel-rust-nightly-example.toml for an example.

use errors::*;
use toml;
use rustup_utils::toml_utils::*;

use std::collections::HashMap;
use dist::TargetTriple;

pub const SUPPORTED_MANIFEST_VERSIONS: [&'static str; 1] = ["2"];
pub const DEFAULT_MANIFEST_VERSION: &'static str = "2";

#[derive(Clone, Debug, PartialEq)]
pub struct Manifest {
    pub manifest_version: String,
    pub date: String,
    pub packages: HashMap<String, Package>,
    pub renames: HashMap<String, String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Package {
    pub version: String,
    pub targets: PackageTargets,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PackageTargets {
    Wildcard(TargetedPackage),
    Targeted(HashMap<TargetTriple, TargetedPackage>)
}

#[derive(Clone, Debug, PartialEq)]
pub struct TargetedPackage {
    pub bins: Option<PackageBins>,
    pub components: Vec<Component>,
    pub extensions: Vec<Component>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PackageBins {
    pub url: String,
    pub hash: String,
    pub xz_url: Option<String>,
    pub xz_hash: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Component {
    pub pkg: String,
    pub target: Option<TargetTriple>,
}

impl Manifest {
    pub fn parse(data: &str) -> Result<Self> {
        let value = toml::from_str(data).map_err(ErrorKind::Parsing)?;
        let manifest = Self::from_toml(value, "")?;
        try!(manifest.validate());

        Ok(manifest)
    }
    pub fn stringify(self) -> String {
        toml::Value::Table(self.to_toml()).to_string()
    }

    pub fn from_toml(mut table: toml::value::Table, path: &str) -> Result<Self> {
        let version = try!(get_string(&mut table, "manifest-version", path));
        if !SUPPORTED_MANIFEST_VERSIONS.contains(&&*version) {
            return Err(ErrorKind::UnsupportedVersion(version).into());
        }
        Ok(Manifest {
            manifest_version: version,
            date: try!(get_string(&mut table, "date", path)),
            packages: try!(Self::table_to_packages(&mut table, path)),
            renames: try!(Self::table_to_renames(table, path)),
        })
    }
    pub fn to_toml(self) -> toml::value::Table {
        let mut result = toml::value::Table::new();

        result.insert("date".to_owned(), toml::Value::String(self.date));
        result.insert("manifest-version".to_owned(),
                      toml::Value::String(self.manifest_version));

        let renames = Self::renames_to_table(self.renames);
        result.insert("rename".to_owned(), toml::Value::Table(renames));

        let packages = Self::packages_to_table(self.packages);
        result.insert("pkg".to_owned(), toml::Value::Table(packages));

        result
    }

    fn table_to_packages(table: &mut toml::value::Table, path: &str) -> Result<HashMap<String, Package>> {
        let mut result = HashMap::new();
        let pkg_table = try!(get_table(table, "pkg", path));

        for (k, v) in pkg_table {
            if let toml::Value::Table(t) = v {
                result.insert(k, try!(Package::from_toml(t, &path)));
            }
        }

        Ok(result)
    }
    fn packages_to_table(packages: HashMap<String, Package>) -> toml::value::Table {
        let mut result = toml::value::Table::new();
        for (k, v) in packages {
            result.insert(k, toml::Value::Table(v.to_toml()));
        }
        result
    }

    fn table_to_renames(mut table: toml::value::Table, path: &str) -> Result<HashMap<String, String>> {
        let mut result = HashMap::new();
        let rename_table = try!(get_table(&mut table, "rename", path));

        for (k, v) in rename_table {
            if let toml::Value::Table(mut t) = v {
                result.insert(k.to_owned(), get_string(&mut t, "to", path)?);
            }
        }

        Ok(result)
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

    pub fn get_package(&self, name: &str) -> Result<&Package> {
        self.packages.get(name).ok_or_else(
            || format!("package not found: '{}'", name).into())
    }

    pub fn get_rust_version(&self) -> Result<&str> {
        self.get_package("rust").map(|p| &*p.version)
    }

    fn validate_targeted_package(&self, tpkg: &TargetedPackage) -> Result<()> {
        for c in tpkg.components.iter().chain(tpkg.extensions.iter()) {
            let cpkg = try!(self.get_package(&c.pkg).chain_err(|| ErrorKind::MissingPackageForComponent(c.clone())));
            let _ctpkg = try!(cpkg.get_target(c.target.as_ref()).chain_err(|| ErrorKind::MissingPackageForComponent(c.clone())));
        }
        Ok(())
    }

    fn validate(&self) -> Result<()> {
        // Every component mentioned must have an actual package to download
        for (_, pkg) in &self.packages {
            match pkg.targets {
                PackageTargets::Wildcard(ref tpkg) => {
                    try!(self.validate_targeted_package(tpkg));
                },
                PackageTargets::Targeted(ref tpkgs) => {
                    for (_, tpkg) in tpkgs {
                        try!(self.validate_targeted_package(tpkg));
                    }
                }
            }
        }

        // The target of any renames must be an actual package. The subject of
        // renames is unconstrained.
        for name in self.renames.values() {
            if !self.packages.contains_key(name) {
                return Err(ErrorKind::MissingPackageForRename(name.clone()).into());
            }
        }

        Ok(())
    }
}

impl Package {
    pub fn from_toml(mut table: toml::value::Table, path: &str) -> Result<Self> {
        Ok(Package {
            version: try!(get_string(&mut table, "version", path)),
            targets: try!(Self::toml_to_targets(table, path)),
        })
    }
    pub fn to_toml(self) -> toml::value::Table {
        let mut result = toml::value::Table::new();

        result.insert("version".to_owned(), toml::Value::String(self.version));

        let targets = Self::targets_to_toml(self.targets);
        result.insert("target".to_owned(), toml::Value::Table(targets));

        result
    }

    fn toml_to_targets(mut table: toml::value::Table, path: &str) -> Result<PackageTargets> {
        let mut target_table = try!(get_table(&mut table, "target", path));

        if let Some(toml::Value::Table(t)) = target_table.remove("*") {
            Ok(PackageTargets::Wildcard(try!(TargetedPackage::from_toml(t, &path))))
        } else {
            let mut result = HashMap::new();
            for (k, v) in target_table {
                if let toml::Value::Table(t) = v {
                    result.insert(TargetTriple::from_str(&k), try!(TargetedPackage::from_toml(t, &path)));
                }
            }
            Ok(PackageTargets::Targeted(result))
        }
    }
    fn targets_to_toml(targets: PackageTargets) -> toml::value::Table {
        let mut result = toml::value::Table::new();
        match targets {
            PackageTargets::Wildcard(tpkg) => {
                result.insert("*".to_owned(), toml::Value::Table(tpkg.to_toml()));
            },
            PackageTargets::Targeted(tpkgs) => {
                for (k, v) in tpkgs {
                    result.insert(k.to_string(), toml::Value::Table(v.to_toml()));
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
                    tpkgs.get(t).ok_or_else(
                        || format!("target not found: '{}'", t).into())
                } else {
                    Err("no target specified".into())
                }
            }
        }
    }
}

impl PackageTargets {
    pub fn get<'a>(&'a self, target: &TargetTriple) -> Option<&'a TargetedPackage> {
        match *self {
            PackageTargets::Wildcard(ref tpkg) => Some(tpkg),
            PackageTargets::Targeted(ref tpkgs) => tpkgs.get(target)
        }
    }
    pub fn get_mut<'a>(&'a mut self, target: &TargetTriple) -> Option<&'a mut TargetedPackage> {
        match *self {
            PackageTargets::Wildcard(ref mut tpkg) => Some(tpkg),
            PackageTargets::Targeted(ref mut tpkgs) => tpkgs.get_mut(target)
        }
    }
}

impl TargetedPackage {
    pub fn from_toml(mut table: toml::value::Table, path: &str) -> Result<Self> {
        let components = try!(get_array(&mut table, "components", path));
        let extensions = try!(get_array(&mut table, "extensions", path));

        if try!(get_bool(&mut table, "available", path)) {
            Ok(TargetedPackage {
                bins: Some(PackageBins {
                    url: try!(get_string(&mut table, "url", path)),
                    hash: try!(get_string(&mut table, "hash", path)),
                    xz_url: get_string(&mut table, "xz_url", path).ok(),
                    xz_hash: get_string(&mut table, "xz_hash", path).ok(),
                }),
                components: try!(Self::toml_to_components(components,
                                                          &format!("{}{}.", path, "components"))),
                extensions: try!(Self::toml_to_components(extensions,
                                                          &format!("{}{}.", path, "extensions"))),
            })
        } else {
            Ok(TargetedPackage {
                bins: None,
                components: vec![],
                extensions: vec![],
            })
        }
    }
    pub fn to_toml(self) -> toml::value::Table {
        let extensions = Self::components_to_toml(self.extensions);
        let components = Self::components_to_toml(self.components);
        let mut result = toml::value::Table::new();
        if !extensions.is_empty() {
            result.insert("extensions".to_owned(), toml::Value::Array(extensions));
        }
        if !components.is_empty() {
            result.insert("components".to_owned(), toml::Value::Array(components));
        }
        if let Some(bins) = self.bins.clone() {
            result.insert("hash".to_owned(), toml::Value::String(bins.hash));
            result.insert("url".to_owned(), toml::Value::String(bins.url));
            if let (Some(xz_hash), Some(xz_url)) = (bins.xz_hash, bins.xz_url) {
                result.insert("xz_hash".to_owned(), toml::Value::String(xz_hash));
                result.insert("xz_url".to_owned(), toml::Value::String(xz_url));
            }
            result.insert("available".to_owned(), toml::Value::Boolean(true));
        } else {
            result.insert("available".to_owned(), toml::Value::Boolean(false));
        }
        result
    }

    pub fn available(&self) -> bool {
        self.bins.is_some()
    }

    fn toml_to_components(arr: toml::value::Array, path: &str) -> Result<Vec<Component>> {
        let mut result = Vec::new();

        for (i, v) in arr.into_iter().enumerate() {
            if let toml::Value::Table(t) = v {
                let path = format!("{}[{}]", path, i);
                result.push(try!(Component::from_toml(t, &path)));
            }
        }

        Ok(result)
    }
    fn components_to_toml(components: Vec<Component>) -> toml::value::Array {
        let mut result = toml::value::Array::new();
        for v in components {
            result.push(toml::Value::Table(v.to_toml()));
        }
        result
    }
}

impl Component {
    pub fn from_toml(mut table: toml::value::Table, path: &str) -> Result<Self> {
        Ok(Component {
            pkg: try!(get_string(&mut table, "pkg", path)),
            target: try!(get_string(&mut table, "target", path).map(|s| {
                if s == "*" {
                    None
                } else {
                    Some(TargetTriple::from_str(&s))
                }
            })),
        })
    }
    pub fn to_toml(self) -> toml::value::Table {
        let mut result = toml::value::Table::new();
        result.insert("target".to_owned(), toml::Value::String(
            self.target.map(|t| t.to_string()).unwrap_or_else(||"*".to_owned())
        ));
        result.insert("pkg".to_owned(), toml::Value::String(self.pkg));
        result
    }
    pub fn name(&self) -> String {
        if let Some(ref t) = self.target {
            format!("{}-{}", self.pkg, t)
        } else {
            format!("{}", self.pkg)
        }
    }
    pub fn description(&self) -> String {
        if let Some(ref t) = self.target {
            format!("'{}' for target '{}'", self.pkg, t)
        } else {
            format!("'{}'", self.pkg)
        }
    }
}
