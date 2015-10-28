use install::InstallPrefix;
use utils;
use components::Components;

const DIST_MANIFEST: &'static str = "dist.toml";

#[derive(Copy, Clone)]
pub struct DistV2<'a> {
	prefix: &'a InstallPrefix,
}

impl<'a> DistV2<'a> {
	pub fn new(prefix: &'a InstallPrefix) -> Option<Self> {
		if utils::is_file(prefix.manifest_file(DIST_MANIFEST)) {
			Some(DistV2 { prefix: prefix })
		} else {
			None
		}
	}
	pub fn prefix(self) -> &'a InstallPrefix {
		self.prefix
	}
	pub fn components(self) -> Components<'a> {
		Components::new(self)
	}
}
