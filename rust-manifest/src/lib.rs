extern crate toml;

use std::collections::HashMap;
use std::fmt::{self, Display};

#[derive(Debug)]
pub enum Error {
	Parsing(Vec<toml::ParserError>),
	MissingKey(String),
	ExpectedType(&'static str, String),
	PackageNotFound(String),
	TargetNotFound(String),
	MissingRoot,
	UnsupportedVersion(String),
}

impl Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> ::std::result::Result<(), fmt::Error> {
		use self::Error::*;
		match *self {
			Parsing(ref n) => {
				for e in n {
					try!(e.fmt(f));
					try!(writeln!(f, ""));
				}
				Ok(())
			},
			MissingKey(ref n) => write!(f, "missing key: '{}'", n),
			ExpectedType(ref t, ref n) => write!(f, "expected type: '{}' for '{}'", t, n),
			PackageNotFound(ref n) => write!(f, "package not found: '{}'", n),
			TargetNotFound(ref n) => write!(f, "target not found: '{}'", n),
			MissingRoot => write!(f, "manifest has no root package"),
			UnsupportedVersion(ref v) => write!(f, "manifest version '{}' is not supported", v),
		}
	}
}

pub type Result<T> = std::result::Result<T, Error>;

pub const SUPPORTED_VERSIONS: [&'static str; 1] = ["2"];
pub const DEFAULT_VERSION: &'static str = "2";

pub struct Diff {
	pub package_urls: Vec<String>,
	pub to_install: Vec<Component>,
	pub to_uninstall: Vec<Component>,
}

#[derive(Clone, Debug)]
pub struct Manifest {
	pub manifest_version: String,
	pub date: String,
	pub root: Option<String>,
	pub packages: HashMap<String, Package>,
}

impl Manifest {
	fn get_packages(table: toml::Table, path: &str) -> Result<HashMap<String, Package>> {
		let mut result = HashMap::new();
		
		for (k, v) in table {
			if let toml::Value::Table(t) = v {
				let path = format!("{}{}.", path, &k);
				result.insert(k, try!(Package::from_toml(t, &path)));
			}
		}
		
		Ok(result)
	}
	pub fn from_toml(mut table: toml::Table, path: &str) -> Result<Self> {
		let version = try!(get_string(&mut table, "manifest_version", path));
		if !SUPPORTED_VERSIONS.contains(&&*version) {
			return Err(Error::UnsupportedVersion(version));
		}
		Ok(Manifest {
			manifest_version: version,
			date: try!(get_string(&mut table, "date", path)),
			root: try!(get_opt_string(&mut table, "root", path)),
			packages: try!(Self::get_packages(table, path)),
		})
	}
	fn set_packages(packages: HashMap<String, Package>) -> toml::Table {
		let mut result = toml::Table::new();
		for (k, v) in packages {
			result.insert(k, toml::Value::Table(v.to_toml()));
		}
		result
	}
	pub fn to_toml(self) -> toml::Table {
		let mut result = Self::set_packages(self.packages);
		result.insert("date".to_owned(), toml::Value::String(self.date));
		result.insert("manifest_version".to_owned(), toml::Value::String(self.manifest_version));
		result
	}
	pub fn get_package(&self, name: &str) -> Result<&Package> {
		self.packages.get(name).ok_or_else(|| Error::PackageNotFound(name.to_owned()))
	}
	
	pub fn for_root<F: Fn(&Component) -> bool>(&self, name: &str, target: &str, extensions: F) -> Result<Self> {
		// Clone the metadata
		let mut result = Manifest {
			manifest_version: DEFAULT_VERSION.to_owned(),
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
			package_urls: Vec::new(),
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
	pub fn init(root_package: &str, target: &str) -> Self {
		let mut result = Manifest {
			manifest_version: DEFAULT_VERSION.to_owned(),
			date: String::new(),
			root: Some(root_package.to_owned()),
			packages: HashMap::new(),
		};
		
		result.packages.insert(root_package.to_owned(), Package::init(target));
		
		result
	}
}

#[derive(Clone, Debug)]
pub struct Package {
	pub version: String,
	pub targets: HashMap<String, TargettedPackage>,
}

impl Package {
	fn get_targets(table: toml::Table, path: &str) -> Result<HashMap<String, TargettedPackage>> {
		let mut result = HashMap::new();
		
		for (k, v) in table {
			if let toml::Value::Table(t) = v {
				let path = format!("{}{}.", path, &k);
				result.insert(k, try!(TargettedPackage::from_toml(t, &path)));
			}
		}
		
		Ok(result)
	}
	pub fn from_toml(mut table: toml::Table, path: &str) -> Result<Self> {
		Ok(Package {
			version: try!(get_string(&mut table, "version", path)),
			targets: try!(Self::get_targets(table, path)),
		})
	}
	fn set_targets(targets: HashMap<String, TargettedPackage>) -> toml::Table {
		let mut result = toml::Table::new();
		for (k, v) in targets {
			result.insert(k, toml::Value::Table(v.to_toml()));
		}
		result
	}
	pub fn to_toml(self) -> toml::Table {
		let mut result = Self::set_targets(self.targets);
		result.insert("version".to_owned(), toml::Value::String(self.version));
		result
	}
	pub fn get_target(&self, target: &str) -> Result<&TargettedPackage> {
		self.targets.get(target).ok_or_else(|| Error::TargetNotFound(target.to_owned()))
	}
	pub fn for_target<F: Fn(&Component) -> bool>(&self, target: &str, extensions: F) -> Result<Self> {
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
	pub fn init(target: &str) -> Self {
		let mut result = Package {
			version: String::new(),
			targets: HashMap::new(),
		};
		
		result.targets.insert(target.to_owned(), TargettedPackage::init());
		
		result
	}
}

#[derive(Clone, Debug)]
pub struct TargettedPackage {
	pub url: String,
	pub hash: String,
	pub components: Vec<Component>,
	pub extensions: Vec<Component>,
}

impl TargettedPackage {
	fn get_components(arr: toml::Array, path: &str) -> Result<Vec<Component>> {
		let mut result = Vec::new();
		
		for (i, v) in arr.into_iter().enumerate() {
			if let toml::Value::Table(t) = v {
				let path = format!("{}[{}]", path, i);
				result.push(try!(Component::from_toml(t, &path)));
			}
		}
		
		Ok(result)
	}
	pub fn from_toml(mut table: toml::Table, path: &str) -> Result<Self> {
		let components = try!(get_array(&mut table, "components", path));
		let extensions = try!(get_array(&mut table, "extensions", path));
		Ok(TargettedPackage {
			url: try!(get_string(&mut table, "url", path)),
			hash: try!(get_string(&mut table, "hash", path)),
			components: try!(Self::get_components(components, &format!("{}{}.", path, "components"))),
			extensions: try!(Self::get_components(extensions, &format!("{}{}.", path, "extensions"))),
		})
	}
	fn set_components(components: Vec<Component>) -> toml::Array {
		let mut result = toml::Array::new();
		for v in components {
			result.push(toml::Value::Table(v.to_toml()));
		}
		result
	}
	pub fn to_toml(self) -> toml::Table {
		let extensions = Self::set_components(self.extensions);
		let components = Self::set_components(self.components);
		let mut result = toml::Table::new();
		if !extensions.is_empty() {
			result.insert("extensions".to_owned(), toml::Value::Array(extensions));
		}
		if !components.is_empty() {
			result.insert("components".to_owned(), toml::Value::Array(components));
		}
		result.insert("hash".to_owned(), toml::Value::String(self.hash));
		result.insert("url".to_owned(), toml::Value::String(self.url));
		result
	}
	pub fn with_extensions<F: Fn(&Component) -> bool>(&self, extensions: F) -> Self {
		TargettedPackage {
			url: self.url.clone(),
			hash: self.hash.clone(),
			components: self.components.clone(),
			extensions: self.extensions.iter().cloned().filter(extensions).collect()
		}
	}
	pub fn flatten_components(&self, v: &mut Vec<Component>) {
		v.extend(self.components.iter().cloned());
	}
	pub fn flatten_extensions(&self, v: &mut Vec<Component>) {
		v.extend(self.extensions.iter().cloned());
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
				diff.package_urls.push(self.url.clone());
			}
		}
	}
	pub fn compute_uninstall(&self, diff: &mut Diff) {
		self.flatten_components(&mut diff.to_uninstall);
	}
	pub fn compute_install(&self, diff: &mut Diff) {
		if !self.components.is_empty() {
			diff.package_urls.push(self.url.clone());
			self.flatten_components(&mut diff.to_install);
		}
	}
	pub fn init() -> Self {
		TargettedPackage {
			url: String::new(),
			hash: String::new(),
			components: Vec::new(),
			extensions: Vec::new(),
		}
	}
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Component {
	pub pkg: String,
	pub target: String,
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

fn get_value(table: &mut toml::Table, key: &str, path: &str) -> Result<toml::Value> {
	table.remove(key).ok_or_else(|| {
		Error::MissingKey(path.to_owned() + key)
	})
}

fn get_string(table: &mut toml::Table, key: &str, path: &str) -> Result<String> {
	get_value(table, key, path).and_then(|v| {
		if let toml::Value::String(s) = v {
			Ok(s)
		} else {
			Err(Error::ExpectedType("string", path.to_owned() + key))
		}
	})
}

fn get_opt_string(table: &mut toml::Table, key: &str, path: &str) -> Result<Option<String>> {
	if let Some(v) = table.remove(key) {
		if let toml::Value::String(s) = v {
			Ok(Some(s))
		} else {
			Err(Error::ExpectedType("string", path.to_owned() + key))
		}
	} else {
		Ok(None)
	}
}

fn get_array(table: &mut toml::Table, key: &str, path: &str) -> Result<toml::Array> {
	if let Some(v) = table.remove(key) {
		if let toml::Value::Array(s) = v {
			Ok(s)
		} else {
			Err(Error::ExpectedType("table", path.to_owned() + key))
		}
	} else {
		Ok(toml::Array::new())
	}
}

pub fn parse(data: &str) -> Result<Manifest> {
	let mut parser = toml::Parser::new(data);
	let value = try!(parser.parse().ok_or_else(move || {
		Error::Parsing(parser.errors)
	}));
	
	Manifest::from_toml(value, "")
}

pub fn stringify(manifest: Manifest) -> String {
	toml::Value::Table(manifest.to_toml()).to_string()
}
