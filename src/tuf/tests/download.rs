//! `Download` going through TUF when started with an enabled [`TufConfig`].

use std::{collections::HashMap, fs, path::Path};

use sha2::{Digest, Sha256};
use tempfile::TempDir;
use url::Url;

use crate::{
    download::{DownloadError, DownloadOptions},
    process::TestProcess,
    tuf::{
        TufConfig,
        tests::{DIST_ROOT, REPO, ignore_date},
    },
};

fn tmp_dir() -> TempDir {
    tempfile::Builder::new()
        .prefix("rustup-tuf-download")
        .tempdir()
        .unwrap()
}

/// Download options and a `TufConfig` pointed at the fixture repository in
/// `src/tuf/tests/repo`, with `mode` set from `enable` and a home under `home`.
fn setup(home: &Path, enable: &str) -> (DownloadOptions, TufConfig) {
    let tuf = Path::new(REPO).join("tuf");
    let vars = HashMap::from([
        ("RUSTUP_TUF_ENABLE".to_owned(), enable.to_owned()),
        (
            "RUSTUP_TUF_DIST_SERVER".to_owned(),
            tuf.to_string_lossy().into_owned(),
        ),
        (
            "RUSTUP_TUF_ROOT".to_owned(),
            tuf.join("metadata/1.root.json")
                .to_string_lossy()
                .into_owned(),
        ),
        ("RUSTUP_TUF_IGNOREDATE".to_owned(), ignore_date(&tuf)),
    ]);
    let tp = TestProcess::with_vars(vars);
    let options = DownloadOptions::try_from(&tp.process).unwrap();
    (options, TufConfig::from_env(home, &tp.process))
}

#[tokio::test]
async fn fetches_verified_target() {
    let tmpdir = tmp_dir();
    let (options, tuf) = setup(&tmpdir.path().join("tuf-home"), "on");
    let url = Url::parse(&format!("{DIST_ROOT}/channels/current/stable.toml")).unwrap();
    let target_path = tmpdir.path().join("stable.toml");

    let mut hasher = Sha256::new();
    options
        .start(&url, &target_path, Some(&tuf))
        .with_hasher(&mut hasher)
        .download()
        .await
        .unwrap();

    let expected = fs::read(Path::new(REPO).join("src/channels/current/stable.toml")).unwrap();
    assert_eq!(fs::read(&target_path).unwrap(), expected);
    assert_eq!(hasher.finalize(), Sha256::digest(&expected));
}

/// TEMPORARY: BACK-COMPAT FOR OLD PATHS. A `dist/` directory in the URL is
/// dropped when naming the target.
#[tokio::test]
async fn strips_dist_prefix_from_target() {
    let tmpdir = tmp_dir();
    let (options, tuf) = setup(&tmpdir.path().join("tuf-home"), "on");
    let url = Url::parse(&format!("{DIST_ROOT}/dist/channels/current/stable.toml")).unwrap();
    let target_path = tmpdir.path().join("stable.toml");

    options
        .start(&url, &target_path, Some(&tuf))
        .download()
        .await
        .unwrap();

    let expected = fs::read(Path::new(REPO).join("src/channels/current/stable.toml")).unwrap();
    assert_eq!(fs::read(&target_path).unwrap(), expected);
}

#[tokio::test]
async fn rejects_unknown_target() {
    let tmpdir = tmp_dir();
    let (options, tuf) = setup(&tmpdir.path().join("tuf-home"), "on");
    let url = Url::parse(&format!("{DIST_ROOT}/channels/current/missing.toml")).unwrap();
    let target_path = tmpdir.path().join("missing.toml");

    let err = options
        .start(&url, &target_path, Some(&tuf))
        .download()
        .await
        .unwrap_err();

    assert!(
        matches!(
            err.downcast_ref::<DownloadError>(),
            Some(DownloadError::Tuf(_))
        ),
        "{err:#}"
    );
    assert!(!target_path.exists());
}

#[tokio::test]
async fn rejects_url_without_path() {
    let tmpdir = tmp_dir();
    let (options, tuf) = setup(&tmpdir.path().join("tuf-home"), "on");
    let url = Url::parse(&format!("{DIST_ROOT}/")).unwrap();
    let target_path = tmpdir.path().join("root");

    let err = options
        .start(&url, &target_path, Some(&tuf))
        .download()
        .await
        .unwrap_err();

    assert!(
        matches!(
            err.downcast_ref::<DownloadError>(),
            Some(DownloadError::Tuf(_))
        ),
        "{err:#}"
    );
    assert!(format!("{err:#}").contains("has no path"), "{err:#}");
}

#[tokio::test]
async fn bypassed_when_off() {
    let tmpdir = tmp_dir();
    let (options, tuf) = setup(&tmpdir.path().join("tuf-home"), "off");
    // Under the fixture's (unreachable) dist root, so the only way this
    // succeeds is by not going through TUF at all.
    let source = Path::new(REPO).join("src/channels/current/stable.toml");
    let url = Url::from_file_path(&source).unwrap();
    let target_path = tmpdir.path().join("stable.toml");

    options
        .start(&url, &target_path, Some(&tuf))
        .download()
        .await
        .unwrap();

    assert_eq!(fs::read(&target_path).unwrap(), fs::read(&source).unwrap());
}
