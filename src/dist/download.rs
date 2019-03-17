use crate::dist::errors::*;
use crate::dist::temp;
use crate::utils::utils;
use crate::{Notification, Verbosity};
use log::{debug, warn};
use sha2::{Digest, Sha256};
use url::Url;

use std::fs;
use std::ops;
use std::path::{Path, PathBuf};

const UPDATE_HASH_LEN: usize = 20;

#[derive(Copy, Clone)]
pub struct DownloadCfg<'a> {
    pub dist_root: &'a str,
    pub temp_cfg: &'a temp::Cfg,
    pub download_dir: &'a PathBuf,
    pub verbosity: Verbosity,
    pub notify_handler: &'a dyn Fn(Notification<'_>),
}

pub struct File {
    path: PathBuf,
}

impl ops::Deref for File {
    type Target = Path;

    fn deref(&self) -> &Path {
        ops::Deref::deref(&self.path)
    }
}

impl<'a> DownloadCfg<'a> {
    /// Downloads a file, validating its hash, and resuming interrupted downloads
    /// Partial downloads are stored in `self.download_dir`, keyed by hash. If the
    /// target file already exists, then the hash is checked and it is returned
    /// immediately without re-downloading.
    pub fn download(&self, url: &Url, hash: &str) -> Result<File> {
        utils::ensure_dir_exists("Download Directory", &self.download_dir, self.verbosity)?;
        let target_file = self.download_dir.join(Path::new(hash));

        if target_file.exists() {
            let cached_result = file_hash(&target_file)?;
            if hash == cached_result {
                (self.notify_handler)(Notification::FileAlreadyDownloaded);
                match self.verbosity {
                    Verbosity::Verbose => debug!("checksum passed"),
                    Verbosity::NotVerbose => (),
                };
                return Ok(File { path: target_file });
            } else {
                (self.notify_handler)(Notification::CachedFileChecksumFailed);
                fs::remove_file(&target_file).chain_err(|| "cleaning up previous download")?;
            }
        }

        let partial_file_path = target_file.with_file_name(
            target_file
                .file_name()
                .map(|s| s.to_str().unwrap_or("_"))
                .unwrap_or("_")
                .to_owned()
                + ".partial",
        );

        let mut hasher = Sha256::new();

        utils::download_file_with_resume(
            &url,
            &partial_file_path,
            Some(&mut hasher),
            true,
            self.verbosity,
            &|n| (self.notify_handler)(n.into()),
        )?;

        let actual_hash = format!("{:x}", hasher.result());

        if hash != actual_hash {
            // Incorrect hash
            return Err(ErrorKind::ChecksumFailed {
                url: url.to_string(),
                expected: hash.to_string(),
                calculated: actual_hash,
            }
            .into());
        } else {
            match self.verbosity {
                Verbosity::Verbose => debug!("checksum passed"),
                Verbosity::NotVerbose => (),
            };

            utils::rename_file("downloaded", &partial_file_path, &target_file)?;
            return Ok(File { path: target_file });
        }
    }

    pub fn clean(&self, hashes: &Vec<String>) -> Result<()> {
        for hash in hashes.iter() {
            let used_file = self.download_dir.join(hash);
            if self.download_dir.join(&used_file).exists() {
                fs::remove_file(used_file).chain_err(|| "cleaning up cached downloads")?;
            }
        }
        Ok(())
    }

    fn download_hash(&self, url: &str) -> Result<String> {
        let hash_url = utils::parse_url(&(url.to_owned() + ".sha256"))?;
        let hash_file = self.temp_cfg.new_file()?;

        utils::download_file(&hash_url, &hash_file, None, self.verbosity, &|n| {
            (self.notify_handler)(n.into())
        })?;

        Ok(utils::read_file("hash", &hash_file).map(|s| s[0..64].to_owned())?)
    }

    /// Downloads a file, sourcing its hash from the same url with a `.sha256` suffix.
    /// If `update_hash` is present, then that will be compared to the downloaded hash,
    /// and if they match, the download is skipped.
    pub fn download_and_check(
        &self,
        url_str: &str,
        update_hash: Option<&Path>,
        ext: &str,
    ) -> Result<Option<(temp::File<'a>, String)>> {
        let hash = self.download_hash(url_str)?;
        let partial_hash: String = hash.chars().take(UPDATE_HASH_LEN).collect();

        if let Some(hash_file) = update_hash {
            if utils::is_file(hash_file) {
                if let Ok(contents) = utils::read_file("update hash", hash_file) {
                    if contents == partial_hash {
                        // Skip download, update hash matches
                        return Ok(None);
                    }
                } else {
                    warn!(
                        "can't read update hash file: '{}', can't skip update...",
                        hash_file.display()
                    )
                }
            } else {
                match self.verbosity {
                    Verbosity::Verbose => debug!("no update hash at: '{}'", hash_file.display()),
                    Verbosity::NotVerbose => (),
                };
            }
        }

        let url = utils::parse_url(url_str)?;
        let file = self.temp_cfg.new_file_with_ext("", ext)?;

        let mut hasher = Sha256::new();
        utils::download_file(&url, &file, Some(&mut hasher), self.verbosity, &|n| {
            (self.notify_handler)(n.into())
        })?;
        let actual_hash = format!("{:x}", hasher.result());

        if hash != actual_hash {
            // Incorrect hash
            return Err(ErrorKind::ChecksumFailed {
                url: url_str.to_owned(),
                expected: hash,
                calculated: actual_hash,
            }
            .into());
        } else {
            match self.verbosity {
                Verbosity::Verbose => debug!("checksum passed"),
                Verbosity::NotVerbose => (),
            };
        }

        // TODO: Check the signature of the file

        Ok(Some((file, partial_hash)))
    }
}

fn file_hash(path: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    use std::io::Read;
    let mut downloaded = fs::File::open(&path).chain_err(|| "opening already downloaded file")?;
    let mut buf = vec![0; 32768];
    loop {
        if let Ok(n) = downloaded.read(&mut buf) {
            if n == 0 {
                break;
            }
            hasher.input(&buf[..n]);
        } else {
            break;
        }
    }

    Ok(format!("{:x}", hasher.result()))
}
