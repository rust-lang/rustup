use std::fs;
use std::ops;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use sha2::{Digest, Sha256};
use url::Url;

use crate::dist::notifications::*;
use crate::dist::temp;
use crate::download::download_file;
use crate::download::download_file_with_resume;
use crate::errors::*;
use crate::process::Process;
use crate::utils;

const UPDATE_HASH_LEN: usize = 20;

#[derive(Copy, Clone)]
pub struct DownloadCfg<'a> {
    pub dist_root: &'a str,
    pub tmp_cx: &'a temp::Context,
    pub download_dir: &'a PathBuf,
    pub notify_handler: &'a dyn Fn(Notification<'_>),
    pub process: &'a Process,
}

pub(crate) struct File {
    path: PathBuf,
}

impl ops::Deref for File {
    type Target = Path;

    fn deref(&self) -> &Path {
        self.path.as_path()
    }
}

impl<'a> DownloadCfg<'a> {
    /// Downloads a file and validates its hash. Resumes interrupted downloads.
    /// Partial downloads are stored in `self.download_dir`, keyed by hash. If the
    /// target file already exists, then the hash is checked and it is returned
    /// immediately without re-downloading.
    pub(crate) async fn download(&self, url: &Url, hash: &str) -> Result<File> {
        utils::ensure_dir_exists(
            "Download Directory",
            self.download_dir,
            &self.notify_handler,
        )?;
        let target_file = self.download_dir.join(Path::new(hash));

        if target_file.exists() {
            let cached_result = file_hash(&target_file, self.notify_handler)?;
            if hash == cached_result {
                (self.notify_handler)(Notification::FileAlreadyDownloaded);
                (self.notify_handler)(Notification::ChecksumValid(url.as_ref()));
                return Ok(File { path: target_file });
            } else {
                (self.notify_handler)(Notification::CachedFileChecksumFailed);
                fs::remove_file(&target_file).context("cleaning up previous download")?;
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

        if let Err(e) = download_file_with_resume(
            url,
            &partial_file_path,
            Some(&mut hasher),
            true,
            &|n| (self.notify_handler)(n.into()),
            self.process,
        )
        .await
        {
            let err = Err(e);
            if partial_file_existed {
                return err.context(RustupError::BrokenPartialFile);
            } else {
                return err;
            }
        };

        let actual_hash = format!("{:x}", hasher.finalize());

        if hash != actual_hash {
            // Incorrect hash
            if partial_file_existed {
                self.clean(&[hash.to_string() + ".partial"])?;
                Err(anyhow!(RustupError::BrokenPartialFile))
            } else {
                Err(RustupError::ChecksumFailed {
                    url: url.to_string(),
                    expected: hash.to_string(),
                    calculated: actual_hash,
                }
                .into())
            }
        } else {
            (self.notify_handler)(Notification::ChecksumValid(url.as_ref()));

            utils::rename(
                "downloaded",
                &partial_file_path,
                &target_file,
                self.notify_handler,
                self.process,
            )?;
            Ok(File { path: target_file })
        }
    }

    pub(crate) fn clean(&self, hashes: &[String]) -> Result<()> {
        for hash in hashes.iter() {
            let used_file = self.download_dir.join(hash);
            if self.download_dir.join(&used_file).exists() {
                fs::remove_file(used_file).context("cleaning up cached downloads")?;
            }
        }
        Ok(())
    }

    async fn download_hash(&self, url: &str) -> Result<String> {
        let hash_url = utils::parse_url(&(url.to_owned() + ".sha256"))?;
        let hash_file = self.tmp_cx.new_file()?;

        download_file(
            &hash_url,
            &hash_file,
            None,
            &|n| (self.notify_handler)(n.into()),
            self.process,
        )
        .await?;

        utils::read_file("hash", &hash_file).map(|s| s[0..64].to_owned())
    }

    /// Downloads a file, sourcing its hash from the same url with a `.sha256` suffix.
    /// If `update_hash` is present, then that will be compared to the downloaded hash,
    /// and if they match, the download is skipped.
    /// Verifies the signature found at the same url with a `.asc` suffix, and prints a
    /// warning when the signature does not verify, or is not found.
    pub(crate) async fn download_and_check(
        &self,
        url_str: &str,
        update_hash: Option<&Path>,
        ext: &str,
    ) -> Result<Option<(temp::File<'a>, String)>> {
        let hash = self.download_hash(url_str).await?;
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
        let file = self.tmp_cx.new_file_with_ext("", ext)?;

        let mut hasher = Sha256::new();
        download_file(
            &url,
            &file,
            Some(&mut hasher),
            &|n| (self.notify_handler)(n.into()),
            self.process,
        )
        .await?;
        let actual_hash = format!("{:x}", hasher.finalize());

        if hash != actual_hash {
            // Incorrect hash
            return Err(RustupError::ChecksumFailed {
                url: url_str.to_owned(),
                expected: hash,
                calculated: actual_hash,
            }
            .into());
        } else {
            (self.notify_handler)(Notification::ChecksumValid(url_str));
        }

        Ok(Some((file, partial_hash)))
    }
}

fn file_hash(path: &Path, notify_handler: &dyn Fn(Notification<'_>)) -> Result<String> {
    let mut hasher = Sha256::new();
    let notification_converter = |notification: crate::utils::Notification<'_>| {
        notify_handler(notification.into());
    };
    let mut downloaded = utils::FileReaderWithProgress::new_file(path, &notification_converter)?;
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
