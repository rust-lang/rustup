
use utils;
use install::InstallPrefix;
use rust_manifest::Config;
use errors::*;
use download::DownloadCfg;
use manifest;

pub const CONFIG_FILE: &'static str = "config.toml";

#[derive(Debug)]
pub struct Configuration {
    prefix: InstallPrefix,
    config: Config,
}

impl Configuration {
    pub fn new(prefix: InstallPrefix) -> Result<Option<Self>> {
        let path = prefix.manifest_file(CONFIG_FILE);
        if utils::is_file(&path) {
            let data = try!(utils::read_file("config", &path));
            Ok(Some(Configuration {
                prefix: prefix,
                config: try!(Config::parse(&data)),
            }))
        } else {
            Ok(None)
        }
    }

    pub fn init(prefix: InstallPrefix, config: Config) -> Result<Self> {
        try!(utils::ensure_dir_exists("manifest", &prefix.manifest_dir(),
                                      utils::NotifyHandler::none()));
        let path = prefix.manifest_file(CONFIG_FILE);
        try!(utils::write_file("config", &path, &config.stringify()));

        let data = try!(utils::read_file("config", &path));
        Ok(Configuration {
            prefix: prefix,
            config: try!(Config::parse(&data)),
        })
    }

    pub fn get_remote_url(&self) -> Option<String> {
        self.config.remote.as_ref().map(|r| r.url.clone())
    }

    pub fn update_dist_manifest(&self, url: &str, download: DownloadCfg) -> Result<()> {
        let new_dist = try!(download.get(url));

        try!(utils::rename_file("dist manifest",
                                &new_dist,
                                &self.prefix.manifest_file(manifest::DIST_MANIFEST)));

        Ok(())
    }
}
