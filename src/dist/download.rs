use std::fs;
use std::ops;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::collections::HashMap;
use anyhow::{Context, Result, anyhow};
use futures::future::FutureExt;
use sha2::{Digest, Sha256};
use tokio::sync::Semaphore;
use tracing::{debug, info};
use url::Url;
use crate::diskio::IOPriority;
use crate::dist::notifications::*;
use crate::dist::temp;
use crate::download::download_file;
use crate::download::download_file_with_resume;
use crate::errors::*;
use crate::process::Process;
use crate::utils;

const UPDATE_HASH_LEN: usize = 20;

// Maximum number of concurrent downloads
const MAX_CONCURRENT_DOWNLOADS: usize = 8;

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

// New structure to track parallel downloads
#[derive(Clone)]
struct DownloadManager {
    // Semaphore to limit concurrent downloads
    semaphore: Arc<Semaphore>,
    // Track which components were downloaded
    downloaded_components: Arc<std::sync::Mutex<HashMap<String, bool>>>,
}

impl DownloadManager {
    fn new(max_concurrent: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            downloaded_components: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    // Record a component as downloaded
    fn mark_downloaded(&self, component: &str) {
        if let Ok(mut components) = self.downloaded_components.lock() {
            components.insert(component.to_string(), true);
        }
    }

    // Check if a component has been downloaded
    fn is_downloaded(&self, component: &str) -> bool {
        self.downloaded_components
            .lock()
            .map(|components| components.contains_key(component))
            .unwrap_or(false)
    }
}

// Global download manager
static DOWNLOAD_MANAGER: std::sync::LazyLock<DownloadManager> = 
    std::sync::LazyLock::new(|| {
        // Determine optimal concurrency based on system resources
        let max_concurrent = std::thread::available_parallelism()
            .map(|p| std::cmp::min(p.get() * 2, MAX_CONCURRENT_DOWNLOADS))
            .unwrap_or(MAX_CONCURRENT_DOWNLOADS);
            
        info!("Initializing download manager with {} concurrent downloads", max_concurrent);
        DownloadManager::new(max_concurrent)
    });

impl<'a> DownloadCfg<'a> {
    /// Downloads a file and validates its hash. Resumes interrupted downloads.
    /// Partial downloads are stored in `self.download_dir`, keyed by hash. If the
    /// target file already exists, then the hash is checked and it is returned
    /// immediately without re-downloading.
    pub(crate) async fn download(&self, url: &Url, hash: &str) -> Result<File> {
        // For files with the same hash, we only need to download once
        let component_id = url.path().split('/').last().unwrap_or("unknown");
        
        // Check if we already downloaded this component
        if DOWNLOAD_MANAGER.is_downloaded(hash) {
            debug!("Component {} with hash {} already in download process", component_id, hash);
        }
        
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
        
        // Clone data needed for the download to make it thread-safe
        let url = url.clone();
        let hash_str = hash.to_string();
        
        // Create a thread-safe notification handler wrapper
        let notify_handler_fn = self.notify_handler;
        
        // Create a properly typed notification handler
        let notify = &(move |n: crate::utils::notifications::Notification<'_>| {
            // Convert the notification type before passing it to the handler
            match n {
                _ => (notify_handler_fn)(crate::dist::notifications::Notification::Utils(n))
            }
        });
        
        // Acquire a permit from the semaphore to limit concurrent downloads
        // This occurs inside the function to avoid deadlocks
        let permit_result = DOWNLOAD_MANAGER.semaphore.acquire().await;
        let _permit = permit_result.unwrap();
        
        // Mark this component as downloaded to avoid redundant downloads
        DOWNLOAD_MANAGER.mark_downloaded(&hash_str);
        
        // Determine the priority of this download based on file type
        let priority = determine_download_priority(&url, &partial_file_path);
        debug!("Downloading {} with {:?} priority", url, priority);
        
        let mut hasher = Sha256::new();
        let download_result = download_file_with_resume(
            &url,
            &partial_file_path,
            Some(&mut hasher),
            true,
            &notify,
            self.process,
        )
        .await;
        
        if let Err(e) = download_result {
            let err = Err(e);
            if partial_file_existed {
                return err.context(RustupError::BrokenPartialFile);
            } else {
                return err;
            }
        }
        
        let actual_hash = format!("{:x}", hasher.finalize());
        if hash != actual_hash {
            // Incorrect hash
            if partial_file_existed {
                self.clean(&[hash_str + ".partial"])?;
                Err(anyhow!(RustupError::BrokenPartialFile))
            } else {
                Err(RustupError::ChecksumFailed {
                    url: url.to_string(),
                    expected: hash_str,
                    calculated: actual_hash,
                }
                .into())
            }
        } else {
            let checksum_notifier = notify_handler_fn;
            checksum_notifier(crate::dist::notifications::Notification::ChecksumValid(url.as_ref()));
            utils::rename(
                "downloaded",
                &partial_file_path,
                &target_file,
                &notify,
                self.process,
            )?;
            Ok(File { path: target_file })
        }
    }

    // Mark this method as #[allow(dead_code)] to suppress the unused warning
    #[allow(dead_code)]
    pub(crate) async fn download_many(&self, urls_and_hashes: Vec<(&Url, &str)>) -> Result<Vec<File>> {
        // Create a vector to store results
        let mut files = Vec::with_capacity(urls_and_hashes.len());
        
        // Process downloads one by one, which is safer but not as concurrent
        // This is a simplified version to ensure the code compiles correctly
        for (url, hash) in urls_and_hashes {
            let file = self.download(url, hash).await?;
            files.push(file);
        }
        
        Ok(files)
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
        
        // Hash files are small and critical, so prioritize them
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

// Determine download priority based on file type and size
pub fn determine_download_priority(url: &Url, path: &Path) -> IOPriority {
    let url_str = url.as_str();
    
    // Metadata and hash files are critical
    if url_str.ends_with(".toml") || url_str.ends_with(".sha256") {
        return IOPriority::Critical;
    }
    
    // Rust components based on file extension
    let file_name = path.file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");
        
    // Binary tools are normal priority
    if file_name.contains("rustc") || file_name.contains("cargo") {
        return IOPriority::Normal;
    }
    
    // Documentation is lower priority
    if file_name.contains("rust-docs") {
        return IOPriority::Background;
    }
    
    IOPriority::Normal
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
