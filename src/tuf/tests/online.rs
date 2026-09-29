//! Tests against the live TUF repository, ignored by default.
//!
//! Run with `cargo test --features test --lib -- tuf::tests::online --ignored`.

use std::{collections::HashMap, fs, path::PathBuf, str::FromStr};

use tempfile::{NamedTempFile, TempDir};
use url::Url;

use crate::{
    config::Cfg,
    dist::{ChannelToolchainName, TargetTuple, download::DownloadCfg, manifest::Manifest},
    download::{DownloadError, DownloadOptions},
    process::TestProcess,
    tuf::{TufConfig, TufRepository, Verification},
};

const REPOSITORY: &str = "https://storage.googleapis.com/tufops";
const ROOT: &str = "https://storage.googleapis.com/tufops/metadata/1.root.json";
const IGNORE_DATE: &str = "2026-09-26T17:06:53Z";

/// URLs as rustup builds them; through TUF only the paths matter.
const STABLE_MANIFEST_URL: &str = "https://static.rust-lang.org/dist/channels/current/stable.toml";
const BETA_MANIFEST_URL: &str = "https://static.rust-lang.org/dist/channels/current/beta.toml";
const NIGHTLY_MANIFEST_URL: &str =
    "https://static.rust-lang.org/dist/channels/current/nightly.toml";
const RELEASE_FILE_URL: &str = "https://static.rust-lang.org/rustup/release-stable.toml";
const RUSTUP_INIT_URL: &str = "https://static.rust-lang.org/rustup/dist";

/// Toolchains resolved through `dl_v2_manifest`.
const TOOLCHAINS: &[&str] = &["stable", "beta", "nightly", "1.98.1", "nightly-2026-09-16"];

/// A `TufConfig` for the live repository, with its downloaded root.
struct Online {
    _root: NamedTempFile,
    home: TempDir,
    process: TestProcess,
    options: DownloadOptions,
    config: TufConfig,
}

impl Online {
    /// Downloads the trusted root and builds a config around it.
    async fn new() -> Self {
        let home = tempfile::Builder::new()
            .prefix("rustup-tuf-online")
            .tempdir()
            .unwrap();

        let plain = TestProcess::with_vars(HashMap::new());
        let options = DownloadOptions::try_from(&plain.process).unwrap();
        let root = tempfile::Builder::new()
            .prefix("rustup-tuf-root")
            .suffix(".json")
            .tempfile()
            .unwrap();
        options
            .start(&Url::parse(ROOT).unwrap(), root.path(), None)
            .download()
            .await
            .expect("trusted root downloads");

        let vars = Self::vars(home.path(), root.path().to_owned());
        let process = TestProcess::new(home.path(), &["rustup"], vars, "");
        let config = TufConfig::from_env(home.path(), &process.process);
        Self {
            _root: root,
            home,
            process,
            options,
            config,
        }
    }

    /// Environment with TUF enabled against the live repository.
    fn vars(home: &std::path::Path, root: PathBuf) -> HashMap<String, String> {
        HashMap::from([
            ("RUSTUP_HOME", home.join("rustup")),
            ("CARGO_HOME", home.join("cargo")),
            ("HOME", home.join("home")),
            ("RUSTUP_TUF_ENABLE", PathBuf::from("on")),
            ("RUSTUP_TUF_SERVER", PathBuf::from(REPOSITORY)),
            ("RUSTUP_TUF_ROOT", root),
            ("RUSTUP_TUF_IGNOREDATE", PathBuf::from(IGNORE_DATE)),
        ])
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_string_lossy().into_owned()))
        .collect()
    }

    /// A `Cfg` from the same environment, with the default dist server.
    fn cfg(&self) -> Cfg<'_> {
        Cfg::from_env(
            self.home.path().to_owned(),
            false,
            true,
            &self.process.process,
        )
        .unwrap()
    }

    fn host(&self) -> TargetTuple {
        TargetTuple::from_host_or_build(&self.process.process)
    }

    /// Downloads `url` through TUF and returns the bytes.
    async fn fetch(&self, url: &str) -> anyhow::Result<Vec<u8>> {
        let path = self.home.path().join("download");
        self.options
            .start(&Url::parse(url).unwrap(), &path, Some(&self.config))
            .download()
            .await?;
        Ok(fs::read(&path)?)
    }
}

#[tokio::test]
#[ignore = "online: reaches storage.googleapis.com"]
async fn root_downloads_as_root_metadata() {
    let online = Online::new().await;
    let root = fs::read_to_string(online._root.path()).unwrap();
    let compact = root.split_whitespace().collect::<String>();
    assert!(
        compact.contains("\"_type\":\"root\""),
        "not a root role: {root}"
    );
}

#[tokio::test]
#[ignore = "online: reaches storage.googleapis.com"]
async fn repository_verifies() {
    let online = Online::new().await;
    let mut repository = TufRepository::open(&online.config, online.options)
        .await
        .unwrap();
    assert_eq!(repository.verify().await.unwrap(), Verification::Verified);
}

#[tokio::test]
#[ignore = "online: reaches storage.googleapis.com"]
async fn stable_manifest_downloads_through_tuf() {
    let online = Online::new().await;
    let bytes = online.fetch(STABLE_MANIFEST_URL).await.unwrap();
    let manifest = Manifest::parse(std::str::from_utf8(&bytes).unwrap()).unwrap();
    assert!(!manifest.date.is_empty());
    assert!(manifest.get_rust_version().is_ok());
}

#[tokio::test]
#[ignore = "online: reaches storage.googleapis.com"]
async fn stable_manifest_through_download_cfg() {
    let online = Online::new().await;
    let cfg = online.cfg();
    let name = ChannelToolchainName::from_str(&format!("stable-{}", online.host())).unwrap();

    let downloaded = DownloadCfg::new(&cfg)
        .dl_v2_manifest(None, &name, &cfg)
        .await
        .unwrap()
        .expect("nothing on record, so the manifest is returned");
    assert!(downloaded.manifest.get_rust_version().is_ok());
    assert_eq!(downloaded.hash.len(), 20);
}

#[tokio::test]
#[ignore = "online: reaches storage.googleapis.com"]
async fn self_update_release_file_downloads_through_tuf() {
    let online = Online::new().await;
    let bytes = online.fetch(RELEASE_FILE_URL).await.unwrap();
    let release = std::str::from_utf8(&bytes).unwrap();
    assert!(release.contains("version"), "not a release file: {release}");
}

#[tokio::test]
#[ignore = "online: reaches storage.googleapis.com"]
async fn unknown_target_is_rejected() {
    let online = Online::new().await;
    let err = online
        .fetch("https://static.rust-lang.org/dist/channels/current/missing.toml")
        .await
        .unwrap_err();
    assert!(
        matches!(
            err.downcast_ref::<DownloadError>(),
            Some(DownloadError::Tuf(_))
        ),
        "{err:#}"
    );
}

#[tokio::test]
#[ignore = "online: reaches storage.googleapis.com"]
async fn beta_and_nightly_manifests_download_through_tuf() {
    let online = Online::new().await;
    for url in [BETA_MANIFEST_URL, NIGHTLY_MANIFEST_URL] {
        let bytes = online
            .fetch(url)
            .await
            .unwrap_or_else(|err| panic!("{url}: {err:#}"));
        let manifest = Manifest::parse(std::str::from_utf8(&bytes).unwrap()).unwrap();
        assert!(manifest.get_rust_version().is_ok(), "{url}");
    }
}

#[tokio::test]
#[ignore = "online: reaches storage.googleapis.com"]
async fn channel_manifests_through_download_cfg() {
    let online = Online::new().await;
    let cfg = online.cfg();
    let dl_cfg = DownloadCfg::new(&cfg);
    let host = online.host();
    for toolchain in TOOLCHAINS {
        let name = ChannelToolchainName::from_str(&format!("{toolchain}-{host}")).unwrap();
        let downloaded = dl_cfg
            .dl_v2_manifest(None, &name, &cfg)
            .await
            .unwrap_or_else(|err| panic!("{toolchain}: {err:#}"))
            .unwrap_or_else(|| panic!("{toolchain}: no manifest returned"));
        assert!(
            downloaded.manifest.get_rust_version().is_ok(),
            "{toolchain}"
        );
        assert_eq!(downloaded.hash.len(), 20, "{toolchain}");
    }
}

#[tokio::test]
#[ignore = "online: reaches storage.googleapis.com"]
async fn latest_rustup_release_through_download_cfg() {
    let online = Online::new().await;
    let cfg = online.cfg();
    let dl_cfg = DownloadCfg::new(&cfg);
    let (file, hash) = dl_cfg
        .download_and_check(RELEASE_FILE_URL, None, None, ".toml", Some(dl_cfg.tuf))
        .await
        .unwrap()
        .expect("nothing on record, so the file is returned");
    let release = fs::read_to_string(&*file).unwrap();
    let table = toml::from_str::<toml::Table>(&release).unwrap();
    assert!(
        table["version"].as_str().is_some_and(|v| !v.is_empty()),
        "{release}"
    );
    assert_eq!(hash.len(), 20);
}

#[tokio::test]
#[ignore = "online: reaches storage.googleapis.com"]
async fn latest_rustup_init_downloads_through_tuf() {
    let online = Online::new().await;
    let url = format!("{RUSTUP_INIT_URL}/{}/rustup-init", online.host());
    let bytes = online.fetch(&url).await.unwrap();
    assert!(
        bytes.len() > 1 << 20,
        "{url}: {} bytes is not a rustup binary",
        bytes.len()
    );
}
