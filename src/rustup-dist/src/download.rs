use rustup_utils::{utils};
use errors::*;
use temp;
use notifications::*;
use sha2::{Sha256, Digest};
use url::Url;


use std::path::{Path, PathBuf};
use std::fs;
use std::ops;

const UPDATE_HASH_LEN: usize = 20;

#[derive(Copy, Clone)]
pub struct DownloadCfg<'a> {
    pub dist_root: &'a str,
    pub temp_cfg: &'a temp::Cfg,
    pub download_dir: &'a PathBuf,
    pub notify_handler: &'a Fn(Notification),
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

        try!(utils::ensure_dir_exists("Download Directory", &self.download_dir, &|n| (self.notify_handler)(n.into())));
        let target_file = self.download_dir.join(Path::new(hash));

        if target_file.exists() {
            let cached_result = try!(file_hash(&target_file));
            if hash == cached_result {
                (self.notify_handler)(Notification::FileAlreadyDownloaded);
                (self.notify_handler)(Notification::ChecksumValid(&url.to_string()));
                return Ok(File { path: target_file, });
            } else {
                (self.notify_handler)(Notification::CachedFileChecksumFailed);
                try!(fs::remove_file(&target_file).chain_err(|| "cleaning up previous download"));
            }
        }


        let partial_file_path =
            target_file.with_file_name(
                target_file.file_name().map(|s| {
                    s.to_str().unwrap_or("_")})
                    .unwrap_or("_")
                    .to_owned()
                + ".partial");

        let mut hasher = Sha256::new();

        try!(utils::download_file_with_resume(&url,
                                  &partial_file_path,
                                  Some(&mut hasher),
                                  true,
                                  &|n| (self.notify_handler)(n.into())));

        let actual_hash = hasher.result_str();

        if hash != actual_hash {
            // Incorrect hash
            return Err(ErrorKind::ChecksumFailed {
                url: url.to_string(),
                expected: hash.to_string(),
                calculated: actual_hash,
            }.into());
        } else {
            (self.notify_handler)(Notification::ChecksumValid(&url.to_string()));
            try!(fs::rename(&partial_file_path, &target_file));
            return Ok(File { path: target_file });
        }
    }

    pub fn clean(&self, hashes: &Vec<String>) -> Result<()> {
        for hash in hashes.iter() {
            let used_file = self.download_dir.join(hash);
            if self.download_dir.join(&used_file).exists() {
                try!(fs::remove_file(used_file).chain_err(|| "cleaning up cached downloads"));
            }
        }
        Ok(())
    }

    fn download_hash(&self, url: &str) -> Result<String> {
        let hash_url = try!(utils::parse_url(&(url.to_owned() + ".sha256")));
        let hash_file = try!(self.temp_cfg.new_file());

        try!(utils::download_file(&hash_url,
                                &hash_file,
                                None,
                                &|n| (self.notify_handler)(n.into())));

        Ok(try!(utils::read_file("hash", &hash_file).map(|s| s[0..64].to_owned())))
    }

    /// Downloads a file, sourcing its hash from the same url with a `.sha256` suffix.
    /// If `update_hash` is present, then that will be compared to the downloaded hash,
    /// and if they match, the download is skipped. 
    pub fn download_and_check(&self,
                                url_str: &str,
                                update_hash: Option<&Path>,
                                ext: &str)
                                -> Result<Option<(temp::File<'a>, String)>> {
        let hash = try!(self.download_hash(url_str));
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

        let url = try!(utils::parse_url(url_str));
        let file = try!(self.temp_cfg.new_file_with_ext("", ext));

        let mut hasher = Sha256::new();
        try!(utils::download_file(&url,
                                &file,
                                Some(&mut hasher),
                                &|n| (self.notify_handler)(n.into())));
        let actual_hash = hasher.result_str();

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

        // TODO: Check the signature of the file

        Ok(Some((file, partial_hash)))
    }
}


fn file_hash(path: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    use std::io::Read;
    let mut downloaded = try!(fs::File::open(&path).chain_err(|| "opening already downloaded file"));
    let mut buf = vec![0; 32768];
    loop {
        if let Ok(n) = downloaded.read(&mut buf) {
            if n == 0 { break; }
            hasher.input(&buf[..n]);
        } else {
            break;
        }
    }

    Ok(hasher.result_str())
}