
use install::InstallPrefix;

pub const CONFIG_FILE: &'static str = "config.toml";

pub struct Configuration(InstallPrefix);

impl Configuration {
	pub fn new(prefix: InstallPrefix) -> Option<Self> {
		if utils::is_file(prefix.manifest_file(PACKAGES_MANIFEST)) {
			Components::new(prefix).map(Manifestation)
		} else {
			None
		}
	}
}
