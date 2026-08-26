//! The v3 manifest layout against the fixture repository.

use std::{
    collections::{BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};

use crate::{
    dist::{ChannelToolchainName, TargetTuple, manifest::Manifest},
    download::DownloadOptions,
    process::TestProcess,
    tuf::{
        TufConfig, TufRepository, Verification,
        tests::{REPO, ignore_date},
    },
};

fn manifests(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            manifests(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "toml") {
            out.push(path);
        }
    }
}

#[tokio::test]
async fn verify_manifests() {
    let repo = Path::new(REPO);
    let src = repo.join("src");
    let tuf = repo.join("tuf");
    let home = tempfile::Builder::new()
        .prefix("rustup-tuf")
        .tempdir()
        .unwrap();

    let vars = HashMap::from([
        ("RUSTUP_TUF_ENABLE".to_owned(), "on".to_owned()),
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
    let config = TufConfig::from_env(home.path(), &tp.process);
    assert!(config.ignore_expiry_after.is_some());

    let options = DownloadOptions::try_from(&tp.process).unwrap();
    let mut repo = TufRepository::open(&config, options).await.unwrap();
    assert_eq!(repo.verify().await.unwrap(), Verification::Verified);

    let mut paths = Vec::new();
    manifests(&src.join("channels"), &mut paths);
    assert!(!paths.is_empty());

    for path in paths {
        let target = path
            .strip_prefix(&src)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let (bytes, verification) = repo.fetch_target(&target).await.unwrap();
        assert_eq!(verification, Verification::Verified, "{target}");
        assert_eq!(bytes, fs::read(&path).unwrap(), "{target}");
        Manifest::parse(std::str::from_utf8(&bytes).unwrap()).unwrap();
    }
}

#[test]
fn verify_manifest_paths() {
    let src = Path::new(REPO).join("src");
    let tp = TestProcess::with_vars(HashMap::new());
    let host = TargetTuple::from_host_or_build(&tp.process);

    let mut generated = BTreeSet::new();
    for toolchain in TOOLCHAINS {
        let desc = ChannelToolchainName::from_str(&format!("{toolchain}-{host}")).unwrap();
        let url = desc.manifest_v3_url("", &tp.process).unwrap();
        let path = src.join(url.trim_start_matches('/'));
        assert!(
            path.is_file(),
            "{toolchain}: {} does not exist",
            path.display()
        );
        generated.insert(path);
    }

    let mut existing = Vec::new();
    manifests(&src.join("channels"), &mut existing);
    let existing = existing.into_iter().collect::<BTreeSet<_>>();
    assert_eq!(
        generated, existing,
        "every manifest in tests/repo/src must be generated"
    );
}

/// Every toolchain with a manifest in `tests/repo/src`.
const TOOLCHAINS: &[&str] = &[
    "stable",
    "beta",
    "nightly",
    "1.98.1",
    "nightly-2026-06-01",
    "nightly-2026-09-16",
    "stable-2026-09-03",
    "beta-2026-09-11",
];
