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
use toml_utils::*;

use std::collections::HashMap;
use dist::TargetTriple;

pub const SUPPORTED_MANIFEST_VERSIONS: [&'static str; 1] = ["2"];
pub const DEFAULT_MANIFEST_VERSION: &'static str = "2";

#[derive(Clone, Debug, PartialEq)]
pub struct Manifest {
    pub manifest_version: String,
    pub date: String,
    pub packages: HashMap<String, Package>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Package {
    pub version: String,
    pub targets: HashMap<TargetTriple, TargettedPackage>,
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
    pub target: TargetTriple,
}

impl Manifest {
    pub fn parse(data: &str) -> Result<Self> {
        let mut parser = toml::Parser::new(data);
        let value = try!(parser.parse().ok_or_else(move || ErrorKind::Parsing(parser.errors).unchained()));

        let manifest = try!(Self::from_toml(value, ""));
        try!(manifest.validate());

        Ok(manifest)
    }
    pub fn stringify(self) -> String {
        toml::Value::Table(self.to_toml()).to_string()
    }

    pub fn from_toml(mut table: toml::Table, path: &str) -> Result<Self> {
        let version = try!(get_string(&mut table, "manifest-version", path));
        if !SUPPORTED_MANIFEST_VERSIONS.contains(&&*version) {
            return Err(ErrorKind::UnsupportedVersion(version).unchained());
        }
        Ok(Manifest {
            manifest_version: version,
            date: try!(get_string(&mut table, "date", path)),
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
        self.packages.get(name).ok_or_else(|| ErrorKind::PackageNotFound(name.to_owned()).unchained())
    }

    fn validate(&self) -> Result<()> {
        // Every component mentioned must have an actual package to download
        for (_, pkg) in &self.packages {
            for (_, tpkg) in &pkg.targets {
                for c in tpkg.components.iter().chain(tpkg.extensions.iter()) {
                    let cpkg = try!(self.get_package(&c.pkg).chain_error(|| ErrorKind::MissingPackageForComponent(c.clone())));
                    let _ctpkg = try!(cpkg.get_target(&c.target).chain_error(|| ErrorKind::MissingPackageForComponent(c.clone())));
                }
            }
        }

        Ok(())
    }
}

impl Package {
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

    fn toml_to_targets(mut table: toml::Table, path: &str) -> Result<HashMap<TargetTriple, TargettedPackage>> {
        let mut result = HashMap::new();
        let target_table = try!(get_table(&mut table, "target", path));

        for (k, v) in target_table {
            if let toml::Value::Table(t) = v {
                result.insert(TargetTriple::from_str(&k), try!(TargettedPackage::from_toml(t, &path)));
            }
        }

        Ok(result)
    }
    fn targets_to_toml(targets: HashMap<TargetTriple, TargettedPackage>) -> toml::Table {
        let mut result = toml::Table::new();
        for (k, v) in targets {
            result.insert(k.to_string(), toml::Value::Table(v.to_toml()));
        }
        result
    }

    pub fn get_target(&self, target: &TargetTriple) -> Result<&TargettedPackage> {
        self.targets.get(target).ok_or_else(|| ErrorKind::TargetNotFound(target.clone()).unchained())
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
}

impl Component {
    pub fn from_toml(mut table: toml::Table, path: &str) -> Result<Self> {
        Ok(Component {
            pkg: try!(get_string(&mut table, "pkg", path)),
            target: try!(get_string(&mut table, "target", path).map(|s| TargetTriple::from_str(&s))),
        })
    }
    pub fn to_toml(self) -> toml::Table {
        let mut result = toml::Table::new();
        result.insert("target".to_owned(), toml::Value::String(self.target.to_string()));
        result.insert("pkg".to_owned(), toml::Value::String(self.pkg));
        result
    }
    pub fn name(&self) -> String {
        format!("{}-{}", self.pkg, self.target)
    }
}
