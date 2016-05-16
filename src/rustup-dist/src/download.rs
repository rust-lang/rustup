use errors::*;
use notifications::*;
use rustup_utils::utils;
use temp;

use sha2::{Sha256, Digest};

use std::path::Path;
use std::process::Command;

pub struct DownloadCfg<'a> {
    pub temp_cfg: &'a temp::Cfg,
    pub notify_handler: &'a Fn(Notification),
    pub gpg_key: Option<&'a str>,
}

impl<'a> DownloadCfg<'a> {
    pub fn get(&self, url: &str) -> Result<temp::File<'a>> {
        if let Some(key) = self.gpg_key {
            // Download and verify with GPG key

            let sig_url = try!(utils::parse_url(&format!("{}.asc", url)));
            let sig_file = try!(self.temp_cfg.new_file());
            try!(utils::download_file(&sig_url, &sig_file, None,
                                      &|n| (self.notify_handler)(n.into())));

            let target_url = try!(utils::parse_url(url));
            let target_file = try!(self.temp_cfg.new_file());

            {
                let target_filename: &Path = &target_file;
                try!(utils::download_file(&target_url,
                                          &target_file,
                                          None,
                                          &|n| (self.notify_handler)(n.into())));

                let key_file = try!(self.temp_cfg.new_file());
                let key_filename: &Path = &key_file;
                try!(utils::write_file("key", &key_file, key));

                let gpg = try!(utils::find_cmd(&["gpg2", "gpg"])
                               .ok_or("could not find 'gpg' on PATH"));

                try!(utils::cmd_status("gpg",
                                       Command::new(gpg)
                                           .arg("--no-permission-warning")
                                           .arg("--dearmor")
                                           .arg(key_filename)));

                try!(utils::cmd_status("gpg",
                                       Command::new(gpg)
                                           .arg("--no-permission-warning")
                                           .arg("--keyring")
                                           .arg(&key_filename.with_extension("gpg"))
                                           .arg("--verify")
                                           .arg(target_filename)));
            }

            Ok(target_file)
        } else {
            // Download and verify with checksum

            let hash_url = try!(utils::parse_url(&format!("{}.sha256", url)));
            let hash_file = try!(self.temp_cfg.new_file());
            try!(utils::download_file(&hash_url, &hash_file, None,
                                      &|n| (self.notify_handler)(n.into())));

            let hash = try!(utils::read_file("hash", &hash_file).map(|s| s[0..64].to_owned()));
            let mut hasher = Sha256::new();

            let target_url = try!(utils::parse_url(url));
            let target_file = try!(self.temp_cfg.new_file());
            try!(utils::download_file(&target_url,
                                      &target_file,
                                      Some(&mut hasher),
                                      &|n| (self.notify_handler)(n.into())));

            let actual_hash = hasher.result_str();

            if hash != actual_hash {
                // Incorrect hash
                return Err(ErrorKind::ChecksumFailed {
                    url: url.to_owned(),
                    expected: hash,
                    calculated: actual_hash,
                }.into());
            } else {
                (self.notify_handler)(Notification::ChecksumValid(url));
            }

            Ok(target_file)
        }

    }
}
