
use rust_manifest::{Component, Manifest, stringify, parse};
use component::{Components, Transaction, ChangeSet, TarGzPackage};
use temp;
use errors::*;
use utils;
use install::InstallPrefix;

pub struct Changes {
	pub add_extensions: Vec<Component>,
	pub remove_extensions: Vec<Component>,
}

pub fn init(prefix: InstallPrefix, root_package: &str, target: &str) -> Result<Components> {
	let packages_path = prefix.manifest_file("packages.toml");
	let new_manifest = Manifest::init(root_package, target);
	let new_manifest_str = stringify(new_manifest);
	
	try!(utils::write_file("packages manifest", &packages_path, &new_manifest_str));
	
	Components::init(prefix)
}

pub fn update(components: &Components, changes: Changes, temp_cfg: &temp::Cfg, notify_handler: NotifyHandler) -> Result<()> {
	// First load dist and packages manifests
	let prefix = components.prefix();
	let dist_path = prefix.manifest_file("dist.toml");
	let dist_manifest = try!(parse(&*try!(utils::read_file("dist manifest", &dist_path))));
	let rel_packages_path = prefix.rel_manifest_file("packages.toml");
	let packages_path = prefix.abs_path(&rel_packages_path);
	let packages_manifest = try!(parse(&*try!(utils::read_file("packages manifest", &packages_path))));
	
	// Find out which extensions are already installed
	let mut old_extensions = Vec::new();
	packages_manifest.flatten_extensions(&mut old_extensions);
	
	// Warn if trying to remove an extension which is not installed
	for e in &changes.remove_extensions {
		if !old_extensions.contains(e) {
			notify_handler.call(Notification::ExtensionNotInstalled(e));
		}
	}
	
	// Compute new set of extensions, given requested changes
	let mut new_extensions = old_extensions.clone();
	new_extensions.retain(|e| !changes.remove_extensions.contains(e));
	new_extensions.extend(changes.add_extensions.iter().cloned());
	
	// Find root package and target of existing installation
	let old_root = try!(packages_manifest.get_root());
	let old_package = try!(packages_manifest.get_package(&old_root));
	let old_target = try!(old_package.root_target());
	
	// Compute the updated packages manifest
	let new_manifest = try!(dist_manifest.for_root(&old_root, &old_target, |e| {
		new_extensions.contains(e)
	}));
	
	// Error out if any requested extensions were not added
	new_extensions.clear();
	new_manifest.flatten_extensions(&mut new_extensions);
	
	for e in &changes.add_extensions {
		if !old_extensions.contains(e) {
			return Err(Error::ExtensionNotFound(e.clone()));
		}
	}
	
	// Compute component-wise diff between the two manifests
	let diff = new_manifest.compute_diff(&packages_manifest);
	
	// Serialize new packages manifest
	let new_manifest_str = stringify(new_manifest);
	
	// Download required packages
	let mut change_set = ChangeSet::new();
	for url in diff.package_urls {
		// Download each package to temp file
		let temp_file = try!(temp_cfg.new_file());
		let url = try!(utils::parse_url(&url));
		try!(utils::download_file(url, &temp_file, None, ntfy!(&notify_handler)));
		
		// And tell components system where to find it
		let package = try!(TarGzPackage::new_file(&temp_file, temp_cfg));
		change_set.add_package(package);
	}
	
	// Mark required component changes
	for c in diff.to_install {
		change_set.install(c.name());
	}
	for c in diff.to_uninstall {
		change_set.uninstall(c.name());
	}
	
	// Begin transaction
	let mut tx = Transaction::new(prefix, temp_cfg, notify_handler);
	
	// Apply changes
	tx = try!(components.apply_change_set(&change_set, &old_target, tx));
	
	// Update packages manifest
	try!(tx.modify_file(rel_packages_path));
	try!(utils::write_file("packages manifest", &packages_path, &new_manifest_str));
	
	// End transaction
	tx.commit();
	
	Ok(())
}