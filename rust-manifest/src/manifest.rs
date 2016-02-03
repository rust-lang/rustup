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
use utils::*;

use std::collections::HashMap;

pub const SUPPORTED_MANIFEST_VERSIONS: [&'static str; 1] = ["2"];
pub const DEFAULT_MANIFEST_VERSION: &'static str = "2";

#[derive(Clone, Debug, PartialEq)]
pub struct Manifest {
    pub manifest_version: String,
    pub date: String,
    pub root: Option<String>,
    pub packages: HashMap<String, Package>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Package {
    pub version: String,
    pub targets: HashMap<String, TargettedPackage>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TargettedPackage {
    pub available: bool,
    pub url: String,
    pub hash: String,
    pub components: Vec<Component>,
    pub extensions: Vec<Component>,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Component {
    pub pkg: String,
    pub target: String,
}

// These secondary types are returned in operations on the manifest

#[derive(Clone, Debug)]
pub struct Diff {
    pub packages: Vec<RequiredPackage>,
    pub to_install: Vec<Component>,
    pub to_uninstall: Vec<Component>,
}

#[derive(Clone, Debug)]
pub struct RequiredPackage {
    pub url: String,
    pub hash: String,
}

impl Manifest {
    pub fn init(root_package: &str, target: &str) -> Self {
        let mut result = Manifest {
            manifest_version: DEFAULT_MANIFEST_VERSION.to_owned(),
            date: String::new(),
            root: Some(root_package.to_owned()),
            packages: HashMap::new(),
        };

        result.packages.insert(root_package.to_owned(), Package::init(target));

        result
    }

    pub fn parse(data: &str) -> Result<Self> {
        let mut parser = toml::Parser::new(data);
        let value = try!(parser.parse().ok_or_else(move || Error::Parsing(parser.errors)));

        Self::from_toml(value, "")
    }
    pub fn stringify(self) -> String {
        toml::Value::Table(self.to_toml()).to_string()
    }

    pub fn from_toml(mut table: toml::Table, path: &str) -> Result<Self> {
        let version = try!(get_string(&mut table, "manifest-version", path));
        if !SUPPORTED_MANIFEST_VERSIONS.contains(&&*version) {
            return Err(Error::UnsupportedVersion(version));
        }
        Ok(Manifest {
            manifest_version: version,
            date: try!(get_string(&mut table, "date", path)),
            root: try!(get_opt_string(&mut table, "root", path)),
            packages: try!(Self::table_to_packages(table, path)),
        })
    }
    pub fn to_toml(self) -> toml::Table {
        let mut result = toml::Table::new();

        result.insert("date".to_owned(), toml::Value::String(self.date));
        result.insert("manifest-version".to_owned(),
                      toml::Value::String(self.manifest_version));

        let packages = Self::packages_to_table(self.packages);
        result.insert("pkg".to_owned(), toml::Value::Table(packages));

        result
    }

    fn table_to_packages(mut table: toml::Table, path: &str) -> Result<HashMap<String, Package>> {
        let mut result = HashMap::new();
        let pkg_table = try!(get_table(&mut table, "pkg", path));

        for (k, v) in pkg_table {
            if let toml::Value::Table(t) = v {
                result.insert(k, try!(Package::from_toml(t, &path)));
            }
        }

        Ok(result)
    }
    fn packages_to_table(packages: HashMap<String, Package>) -> toml::Table {
        let mut result = toml::Table::new();
        for (k, v) in packages {
            result.insert(k, toml::Value::Table(v.to_toml()));
        }
        result
    }


    pub fn get_package(&self, name: &str) -> Result<&Package> {
        self.packages.get(name).ok_or_else(|| Error::PackageNotFound(name.to_owned()))
    }

    pub fn for_root<F: Fn(&Component) -> bool>(&self,
                                               name: &str,
                                               target: &str,
                                               extensions: F)
                                               -> Result<Self> {
        // Clone the metadata
        let mut result = Manifest {
            manifest_version: DEFAULT_MANIFEST_VERSION.to_owned(),
            date: self.date.clone(),
            root: Some(name.to_owned()),
            packages: HashMap::new(),
        };

        // Find the desired package
        let package = try!(self.get_package(name));

        // Filter to a single target and set of extensions
        let new_package = try!(package.for_target(target, extensions));

        // Add the package
        result.packages.insert(name.to_owned(), new_package);

        // Extensions require additional packages, so find all extensions
        let mut extensions = Vec::new();
        result.flatten_extensions(&mut extensions);

        // For each extension, add the package to which it belongs
        for e in extensions {
            let p = try!(self.get_package(&e.pkg));
            result.packages.insert(e.pkg, try!(p.for_target(&e.target, |_| false)));
        }

        // Done
        Ok(result)
    }
    pub fn flatten_components(&self, v: &mut Vec<Component>) {
        for (_, p) in &self.packages {
            p.flatten_components(v);
        }
    }
    pub fn flatten_extensions(&self, v: &mut Vec<Component>) {
        for (_, p) in &self.packages {
            p.flatten_extensions(v);
        }
    }
    pub fn flatten_urls(&self, v: &mut Vec<String>) {
        for (_, p) in &self.packages {
            p.flatten_urls(v);
        }
    }
    pub fn compute_diff(&self, prev: &Self) -> Diff {
        let mut result = Diff {
            packages: Vec::new(),
            to_install: Vec::new(),
            to_uninstall: Vec::new(),
        };
        for (k, p) in &prev.packages {
            if let Some(q) = self.packages.get(k) {
                q.compute_diff(p, &mut result);
            } else {
                p.compute_uninstall(&mut result);
            }
        }
        for (k, p) in &self.packages {
            if !self.packages.contains_key(k) {
                p.compute_install(&mut result);
            }
        }
        result
    }
    pub fn get_root(&self) -> Result<String> {
        self.root.clone().ok_or(Error::MissingRoot)
    }
}

impl Package {
    pub fn init(target: &str) -> Self {
        let mut result = Package {
            version: String::new(),
            targets: HashMap::new(),
        };

        result.targets.insert(target.to_owned(), TargettedPackage::init());

        result
    }

    pub fn from_toml(mut table: toml::Table, path: &str) -> Result<Self> {
        Ok(Package {
            version: try!(get_string(&mut table, "version", path)),
            targets: try!(Self::toml_to_targets(table, path)),
        })
    }
    pub fn to_toml(self) -> toml::Table {
        let mut result = toml::Table::new();

        result.insert("version".to_owned(), toml::Value::String(self.version));

        let targets = Self::targets_to_toml(self.targets);
        result.insert("target".to_owned(), toml::Value::Table(targets));

        result
    }

    fn toml_to_targets(mut table: toml::Table, path: &str) -> Result<HashMap<String, TargettedPackage>> {
        let mut result = HashMap::new();
        let target_table = try!(get_table(&mut table, "target", path));

        for (k, v) in target_table {
            if let toml::Value::Table(t) = v {
                result.insert(k, try!(TargettedPackage::from_toml(t, &path)));
            }
        }

        Ok(result)
    }
    fn targets_to_toml(targets: HashMap<String, TargettedPackage>) -> toml::Table {
        let mut result = toml::Table::new();
        for (k, v) in targets {
            result.insert(k, toml::Value::Table(v.to_toml()));
        }
        result
    }

    pub fn get_target(&self, target: &str) -> Result<&TargettedPackage> {
        self.targets.get(target).ok_or_else(|| Error::TargetNotFound(target.to_owned()))
    }
    pub fn for_target<F: Fn(&Component) -> bool>(&self,
                                                 target: &str,
                                                 extensions: F)
                                                 -> Result<Self> {
        let mut result = Package {
            version: self.version.clone(),
            targets: HashMap::new(),
        };

        let targetted_package = try!(self.get_target(target));
        let new_targetted_package = targetted_package.with_extensions(extensions);

        result.targets.insert(target.to_owned(), new_targetted_package);

        Ok(result)
    }

    pub fn flatten_components(&self, v: &mut Vec<Component>) {
        for (_, t) in &self.targets {
            t.flatten_components(v);
        }
    }
    pub fn flatten_extensions(&self, v: &mut Vec<Component>) {
        for (_, t) in &self.targets {
            t.flatten_extensions(v);
        }
    }
    pub fn flatten_urls(&self, v: &mut Vec<String>) {
        for (_, t) in &self.targets {
            v.push(t.url.clone());
        }
    }
    pub fn compute_diff(&self, prev: &Self, diff: &mut Diff) {
        for (k, t) in &prev.targets {
            if let Some(u) = self.targets.get(k) {
                u.compute_diff(t, diff);
            } else {
                t.compute_uninstall(diff);
            }
        }
        for (k, t) in &self.targets {
            if !self.targets.contains_key(k) {
                t.compute_install(diff);
            }
        }
    }
    pub fn compute_uninstall(&self, diff: &mut Diff) {
        for (_, t) in &self.targets {
            t.compute_uninstall(diff);
        }
    }
    pub fn compute_install(&self, diff: &mut Diff) {
        for (_, t) in &self.targets {
            t.compute_install(diff);
        }
    }
    pub fn root_target(&self) -> Result<String> {
        Ok(try!(self.targets.iter().next().ok_or(Error::MissingRoot)).0.clone())
    }
}

impl TargettedPackage {
    pub fn from_toml(mut table: toml::Table, path: &str) -> Result<Self> {
        let components = try!(get_array(&mut table, "components", path));
        let extensions = try!(get_array(&mut table, "extensions", path));
        Ok(TargettedPackage {
            available: try!(get_bool(&mut table, "available", path)),
            url: try!(get_string(&mut table, "url", path)),
            hash: try!(get_string(&mut table, "hash", path)),
            components: try!(Self::toml_to_components(components,
                                                      &format!("{}{}.", path, "components"))),
            extensions: try!(Self::toml_to_components(extensions,
                                                      &format!("{}{}.", path, "extensions"))),
        })
    }
    pub fn to_toml(self) -> toml::Table {
        let extensions = Self::components_to_toml(self.extensions);
        let components = Self::components_to_toml(self.components);
        let mut result = toml::Table::new();
        if !extensions.is_empty() {
            result.insert("extensions".to_owned(), toml::Value::Array(extensions));
        }
        if !components.is_empty() {
            result.insert("components".to_owned(), toml::Value::Array(components));
        }
        result.insert("hash".to_owned(), toml::Value::String(self.hash));
        result.insert("url".to_owned(), toml::Value::String(self.url));
        result.insert("available".to_owned(), toml::Value::Boolean(self.available));
        result
    }

    fn toml_to_components(arr: toml::Array, path: &str) -> Result<Vec<Component>> {
        let mut result = Vec::new();

        for (i, v) in arr.into_iter().enumerate() {
            if let toml::Value::Table(t) = v {
                let path = format!("{}[{}]", path, i);
                result.push(try!(Component::from_toml(t, &path)));
            }
        }

        Ok(result)
    }
    fn components_to_toml(components: Vec<Component>) -> toml::Array {
        let mut result = toml::Array::new();
        for v in components {
            result.push(toml::Value::Table(v.to_toml()));
        }
        result
    }

    pub fn with_extensions<F: Fn(&Component) -> bool>(&self, extensions: F) -> Self {
        TargettedPackage {
            available: self.available,
            url: self.url.clone(),
            hash: self.hash.clone(),
            components: self.components.clone(),
            extensions: self.extensions.iter().cloned().filter(extensions).collect(),
        }
    }
    pub fn flatten_components(&self, v: &mut Vec<Component>) {
        v.extend(self.components.iter().cloned());
    }
    pub fn flatten_extensions(&self, v: &mut Vec<Component>) {
        v.extend(self.extensions.iter().cloned());
    }
    fn add_requirement(&self, diff: &mut Diff) {
        diff.packages.push(RequiredPackage {
            url: self.url.clone(),
            hash: self.hash.clone(),
        });
    }
    pub fn compute_diff(&self, prev: &Self, diff: &mut Diff) {
        // If hash changes, then need to reinstall all components
        if self.hash != prev.hash {
            prev.compute_uninstall(diff);
            self.compute_install(diff);
        } else {
            // Otherwise compute which components need installing or uninstalling
            for c in &prev.components {
                if !self.components.contains(c) {
                    diff.to_uninstall.push(c.clone());
                }
            }
            let mut is_required = false;
            for c in &self.components {
                if !prev.components.contains(c) {
                    is_required = true;
                    diff.to_install.push(c.clone());
                }
            }
            // The package is only required if one or more components are to be
            // installed from it.
            if is_required {
                self.add_requirement(diff);
            }
        }
    }
    pub fn compute_uninstall(&self, diff: &mut Diff) {
        self.flatten_components(&mut diff.to_uninstall);
    }
    pub fn compute_install(&self, diff: &mut Diff) {
        if !self.components.is_empty() {
            self.add_requirement(diff);
            self.flatten_components(&mut diff.to_install);
        }
    }
    pub fn init() -> Self {
        TargettedPackage {
            available: false,
            url: String::new(),
            hash: String::new(),
            components: Vec::new(),
            extensions: Vec::new(),
        }
    }
}

impl Component {
    pub fn from_toml(mut table: toml::Table, path: &str) -> Result<Self> {
        Ok(Component {
            pkg: try!(get_string(&mut table, "pkg", path)),
            target: try!(get_string(&mut table, "target", path)),
        })
    }
    pub fn to_toml(self) -> toml::Table {
        let mut result = toml::Table::new();
        result.insert("target".to_owned(), toml::Value::String(self.target));
        result.insert("pkg".to_owned(), toml::Value::String(self.pkg));
        result
    }
    pub fn name(&self) -> String {
        format!("{}-{}", self.pkg, self.target)
    }
}

