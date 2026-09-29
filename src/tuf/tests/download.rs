//! `Download` with TUF enabled.

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

const STABLE_URL: &str = "https://dist.invalid/channels/current/stable.toml";
const STABLE_SRC: &str = "src/channels/current/stable.toml";

fn tmp_dir() -> TempDir {
    tempfile::Builder::new()
        .prefix("rustup-tuf-download")
        .tempdir()
        .unwrap()
}

/// Download options and a `TufConfig` for the repository at `tuf`.
fn setup(tuf: &Path, home: &Path, vars: &[(&str, &str)]) -> (DownloadOptions, TufConfig) {
    let mut vars = vars
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect::<HashMap<_, _>>();
    vars.insert(
        "RUSTUP_TUF_DIST_SERVER".to_owned(),
        tuf.to_string_lossy().into_owned(),
    );
    vars.insert(
        "RUSTUP_TUF_ROOT".to_owned(),
        tuf.join("metadata/1.root.json")
            .to_string_lossy()
            .into_owned(),
    );
    vars.insert("RUSTUP_TUF_IGNOREDATE".to_owned(), ignore_date(tuf));
    let tp = TestProcess::with_vars(vars);
    let options = DownloadOptions::try_from(&tp.process).unwrap();
    (options, TufConfig::from_env(home, &tp.process))
}

fn fixture() -> (TempDir, std::path::PathBuf) {
    let tmpdir = tmp_dir();
    (tmpdir, Path::new(REPO).join("tuf"))
}

/// A copy of the fixture whose `stable.toml` target was altered after signing.
fn tampered_fixture() -> (TempDir, std::path::PathBuf, Vec<u8>) {
    fn copy_dir(from: &Path, to: &Path) {
        fs::create_dir_all(to).unwrap();
        for entry in fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let dest = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_dir(&entry.path(), &dest);
            } else {
                fs::copy(entry.path(), dest).unwrap();
            }
        }
    }

    let tmpdir = tmp_dir();
    let tuf = tmpdir.path().join("tuf");
    copy_dir(&Path::new(REPO).join("tuf"), &tuf);

    let stable = fs::read_dir(tuf.join("targets/channels/current"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.to_string_lossy().ends_with(".stable.toml"))
        .expect("fixture publishes a stable.toml target");
    let mut tampered = fs::read(&stable).unwrap();
    tampered.extend_from_slice(b"\n# tampered after signing\n");
    fs::write(&stable, &tampered).unwrap();
    (tmpdir, tuf, tampered)
}

#[tokio::test]
async fn fetches_verified_target() {
    let (tmpdir, tuf) = fixture();
    let (options, config) = setup(
        &tuf,
        &tmpdir.path().join("home"),
        &[("RUSTUP_TUF_ENABLE", "on")],
    );
    let url = Url::parse(STABLE_URL).unwrap();
    let target_path = tmpdir.path().join("stable.toml");

    let mut hasher = Sha256::new();
    options
        .start(&url, &target_path, Some(&config))
        .with_hasher(&mut hasher)
        .download()
        .await
        .unwrap();

    let expected = fs::read(Path::new(REPO).join(STABLE_SRC)).unwrap();
    assert_eq!(fs::read(&target_path).unwrap(), expected);
    assert_eq!(hasher.finalize(), Sha256::digest(&expected));
}

/// TEMPORARY: BACK-COMPAT FOR OLD PATHS: `dist/` is dropped from the target name.
#[tokio::test]
async fn strips_dist_prefix_from_target() {
    let (tmpdir, tuf) = fixture();
    let (options, config) = setup(
        &tuf,
        &tmpdir.path().join("home"),
        &[("RUSTUP_TUF_ENABLE", "on")],
    );
    let url = Url::parse(&format!("{DIST_ROOT}/dist/channels/current/stable.toml")).unwrap();
    let target_path = tmpdir.path().join("stable.toml");

    options
        .start(&url, &target_path, Some(&config))
        .download()
        .await
        .unwrap();

    let expected = fs::read(Path::new(REPO).join(STABLE_SRC)).unwrap();
    assert_eq!(fs::read(&target_path).unwrap(), expected);
}

#[tokio::test]
async fn resume_is_ignored_through_tuf() {
    let (tmpdir, tuf) = fixture();
    let (options, config) = setup(
        &tuf,
        &tmpdir.path().join("home"),
        &[("RUSTUP_TUF_ENABLE", "on")],
    );
    let url = Url::parse(STABLE_URL).unwrap();
    let target_path = tmpdir.path().join("stable.toml.partial");
    fs::write(&target_path, b"stale partial contents").unwrap();

    let mut hasher = Sha256::new();
    options
        .start(&url, &target_path, Some(&config))
        .with_hasher(&mut hasher)
        .with_resume()
        .download()
        .await
        .unwrap();

    // The partial file is replaced wholesale.
    let expected = fs::read(Path::new(REPO).join(STABLE_SRC)).unwrap();
    assert_eq!(fs::read(&target_path).unwrap(), expected);
    assert_eq!(hasher.finalize(), Sha256::digest(&expected));
}

#[tokio::test]
async fn rejects_unknown_target() {
    let (tmpdir, tuf) = fixture();
    let (options, config) = setup(
        &tuf,
        &tmpdir.path().join("home"),
        &[("RUSTUP_TUF_ENABLE", "on")],
    );
    let url = Url::parse(&format!("{DIST_ROOT}/channels/current/missing.toml")).unwrap();
    let target_path = tmpdir.path().join("missing.toml");

    let err = options
        .start(&url, &target_path, Some(&config))
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
    let (tmpdir, tuf) = fixture();
    let (options, config) = setup(
        &tuf,
        &tmpdir.path().join("home"),
        &[("RUSTUP_TUF_ENABLE", "on")],
    );
    let url = Url::parse(&format!("{DIST_ROOT}/")).unwrap();
    let target_path = tmpdir.path().join("root");

    let err = options
        .start(&url, &target_path, Some(&config))
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
async fn on_mode_rejects_tampered_target() {
    let (tmpdir, tuf, _) = tampered_fixture();
    let (options, config) = setup(
        &tuf,
        &tmpdir.path().join("home"),
        &[("RUSTUP_TUF_ENABLE", "on")],
    );
    let url = Url::parse(STABLE_URL).unwrap();
    let target_path = tmpdir.path().join("stable.toml");

    let err = options
        .start(&url, &target_path, Some(&config))
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
async fn warn_mode_hands_out_tampered_target_unverified() {
    let (tmpdir, tuf, tampered) = tampered_fixture();
    let (options, config) = setup(
        &tuf,
        &tmpdir.path().join("home"),
        &[("RUSTUP_TUF_ENABLE", "warn")],
    );
    let url = Url::parse(STABLE_URL).unwrap();
    let target_path = tmpdir.path().join("stable.toml");

    options
        .start(&url, &target_path, Some(&config))
        .download()
        .await
        .unwrap();

    // Tolerated failure: the bytes come back as served.
    assert_eq!(fs::read(&target_path).unwrap(), tampered);
}

#[tokio::test]
async fn ignore_flag_tolerates_tampered_target() {
    let (tmpdir, tuf, tampered) = tampered_fixture();
    let (options, config) = setup(
        &tuf,
        &tmpdir.path().join("home"),
        &[("RUSTUP_TUF_ENABLE", "on"), ("RUSTUP_TUF_IGNORE", "1")],
    );
    let url = Url::parse(STABLE_URL).unwrap();
    let target_path = tmpdir.path().join("stable.toml");

    options
        .start(&url, &target_path, Some(&config))
        .download()
        .await
        .unwrap();

    assert_eq!(fs::read(&target_path).unwrap(), tampered);
}

#[tokio::test]
async fn bypassed_when_off() {
    let (tmpdir, tuf) = fixture();
    let (options, config) = setup(
        &tuf,
        &tmpdir.path().join("home"),
        &[("RUSTUP_TUF_ENABLE", "off")],
    );
    // Only a plain download can read a file URL.
    let source = Path::new(REPO).join(STABLE_SRC);
    let url = Url::from_file_path(&source).unwrap();
    let target_path = tmpdir.path().join("stable.toml");

    options
        .start(&url, &target_path, Some(&config))
        .download()
        .await
        .unwrap();

    assert_eq!(fs::read(&target_path).unwrap(), fs::read(&source).unwrap());
}
