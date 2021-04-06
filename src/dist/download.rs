use crate::config::PgpPublicKey;
use crate::dist::notifications::*;
use crate::dist::temp;
use crate::errors::*;
use crate::utils::utils;

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
    pub notify_handler: &'a dyn Fn(Notification<'_>),
    pub pgp_keys: &'a [PgpPublicKey],
}

pub struct File {
    path: PathBuf,
}

impl ops::Deref for File {
    type Target = Path;

    fn deref(&self) -> &Path {
        self.path.as_path()
    }
}

impl<'a> DownloadCfg<'a> {
    /// Downloads a file, validating its hash, and resuming interrupted downloads
    /// Partial downloads are stored in `self.download_dir`, keyed by hash. If the
    /// target file already exists, then the hash is checked and it is returned
    /// immediately without re-downloading.
    pub fn download(&self, url: &Url, hash: &str) -> Result<File> {
        utils::ensure_dir_exists(
            "Download Directory",
            &self.download_dir,
            &self.notify_handler,
        )?;
        let target_file = self.download_dir.join(Path::new(hash));

        if target_file.exists() {
            let cached_result = file_hash(&target_file, self.notify_handler)?;
            if hash == cached_result {
                (self.notify_handler)(Notification::FileAlreadyDownloaded);
                (self.notify_handler)(Notification::ChecksumValid(&url.to_string()));
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

        let partial_file_existed = partial_file_path.exists();

        let mut hasher = Sha256::new();

        if let Err(e) = utils::download_file_with_resume(
            &url,
            &partial_file_path,
            Some(&mut hasher),
            true,
            &|n| (self.notify_handler)(n.into()),
        ) {
            if partial_file_existed {
                return Err(e).chain_err(|| ErrorKind::BrokenPartialFile);
            } else {
                return Err(e);
            }
        };

        let actual_hash = format!("{:x}", hasher.finalize());

        if hash != actual_hash {
            // Incorrect hash
            if partial_file_existed {
                self.clean(&[hash.to_string() + &".partial".to_string()])?;
                Err(ErrorKind::BrokenPartialFile.into())
            } else {
                Err(ErrorKind::ChecksumFailed {
                    url: url.to_string(),
                    expected: hash.to_string(),
                    calculated: actual_hash,
                }
                .into())
            }
        } else {
            (self.notify_handler)(Notification::ChecksumValid(&url.to_string()));

            utils::rename_file(
                "downloaded",
                &partial_file_path,
                &target_file,
                self.notify_handler,
            )?;
            Ok(File { path: target_file })
        }
    }

    pub fn clean(&self, hashes: &[String]) -> Result<()> {
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

        utils::download_file(&hash_url, &hash_file, None, &|n| {
            (self.notify_handler)(n.into())
        })?;

        utils::read_file("hash", &hash_file).map(|s| s[0..64].to_owned())
    }

    fn download_signature(&self, url: &str) -> Result<String> {
        let sig_url = utils::parse_url(&(url.to_owned() + ".asc"))?;
        let sig_file = self.temp_cfg.new_file()?;

        utils::download_file(&sig_url, &sig_file, None, &|n| {
            (self.notify_handler)(n.into())
        })?;

        utils::read_file("signature", &sig_file)
    }

    fn check_signature(&self, url: &str, file: &temp::File<'_>) -> Result<&PgpPublicKey> {
        assert!(
            !self.pgp_keys.is_empty(),
            "At least the builtin key must be present"
        );

        let signature = self.download_signature(url).map_err(|e| {
            e.chain_err(|| ErrorKind::SignatureVerificationFailed {
                url: url.to_owned(),
            })
        })?;

        let file_path: &Path = &file;
        let content = std::fs::File::open(file_path).chain_err(|| ErrorKind::ReadingFile {
            name: "channel data",
            path: PathBuf::from(file_path),
        })?;

        let sig_result =
            crate::dist::signatures::verify_signature(content, &signature, &self.pgp_keys)?;
        if let Some(keyidx) = sig_result {
            let key = &self.pgp_keys[keyidx];
            Ok(key)
        } else {
            Err(ErrorKind::SignatureVerificationFailed {
                url: url.to_owned(),
            }
            .into())
        }
    }

    /// Downloads a file, sourcing its hash from the same url with a `.sha256` suffix.
    /// If `update_hash` is present, then that will be compared to the downloaded hash,
    /// and if they match, the download is skipped.
    /// Verifies the signature found at the same url with a `.asc` suffix, and prints a
    /// warning when the signature does not verify, or is not found.
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
                    (self.notify_handler)(Notification::CantReadUpdateHash(hash_file));
                }
            } else {
                (self.notify_handler)(Notification::NoUpdateHash(hash_file));
            }
        }

        let url = utils::parse_url(url_str)?;
        let file = self.temp_cfg.new_file_with_ext("", ext)?;

        let mut hasher = Sha256::new();
        utils::download_file(&url, &file, Some(&mut hasher), &|n| {
            (self.notify_handler)(n.into())
        })?;
        let actual_hash = format!("{:x}", hasher.finalize());

        if hash != actual_hash {
            // Incorrect hash
            return Err(ErrorKind::ChecksumFailed {
                url: url_str.to_owned(),
                expected: hash,
                calculated: actual_hash,
            }
            .into());
        } else {
            (self.notify_handler)(Notification::ChecksumValid(url_str));
        }

        // No signatures for tarballs for now.
        if !url_str.ends_with(".tar.gz") && !url_str.ends_with(".tar.xz") {
            match self.check_signature(&url_str, &file) {
                Ok(key) => (self.notify_handler)(Notification::SignatureValid(url_str, key)),
                Err(_) => (self.notify_handler)(Notification::SignatureInvalid(url_str)),
            }
        }

        Ok(Some((file, partial_hash)))
    }
}

fn file_hash(path: &Path, notify_handler: &dyn Fn(Notification<'_>)) -> Result<String> {
    let mut hasher = Sha256::new();
    let notification_converter = |notification: crate::utils::Notification<'_>| {
        notify_handler(notification.into());
    };
    let mut downloaded = utils::FileReaderWithProgress::new_file(&path, &notification_converter)?;
    use std::io::Read;
    let mut buf = vec![0; 32768];
    while let Ok(n) = downloaded.read(&mut buf) {
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}
