//! Channel manifests through `dl_v2_manifest` with TUF enabled.

use std::{
    collections::{BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};

use sha2::{Digest, Sha256};
use tempfile::TempDir;

use crate::{
    config::Cfg,
    dist::{ChannelToolchainName, TargetTuple, download::DownloadCfg, manifest::Manifest},
    process::TestProcess,
    tuf::tests::{DIST_ROOT, REPO, ignore_date},
};

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

/// TUF enabled against the fixture, with an unreachable dist server.
fn test_process(root: PathBuf) -> TestProcess {
    let tuf = Path::new(REPO).join("tuf");
    let vars = HashMap::from([
        ("RUSTUP_HOME", root.join("rustup")),
        ("CARGO_HOME", root.join("cargo")),
        ("HOME", root.join("home")),
        ("RUSTUP_DIST_SERVER", PathBuf::from(DIST_ROOT)),
        ("RUSTUP_TUF_ENABLE", PathBuf::from("on")),
        ("RUSTUP_TUF_SERVER", tuf.clone()),
        ("RUSTUP_TUF_ROOT", tuf.join("metadata/1.root.json")),
        ("RUSTUP_TUF_IGNOREDATE", PathBuf::from(ignore_date(&tuf))),
    ])
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v.to_string_lossy().into_owned()))
    .collect();
    TestProcess::new(root, &["rustup"], vars, "")
}

fn tmp_dir() -> TempDir {
    tempfile::Builder::new()
        .prefix("rustup-tuf-manifest")
        .tempdir()
        .unwrap()
}

fn manifests(dir: &Path, out: &mut BTreeSet<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            manifests(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "toml") {
            out.insert(path);
        }
    }
}

#[tokio::test]
async fn manifests_come_from_the_repository() {
    let root = tmp_dir();
    let tp = test_process(root.path().to_owned());
    let cfg = Cfg::from_env(root.path().to_owned(), false, true, &tp.process).unwrap();
    let dl_cfg = DownloadCfg::new(&cfg);
    let host = TargetTuple::from_host_or_build(&tp.process);
    let src = Path::new(REPO).join("src");

    let mut fetched = BTreeSet::new();
    for toolchain in TOOLCHAINS {
        let name = ChannelToolchainName::from_str(&format!("{toolchain}-{host}")).unwrap();
        let downloaded = dl_cfg
            .dl_v2_manifest(None, &name, &cfg)
            .await
            .unwrap_or_else(|err| panic!("{toolchain}: {err:#}"))
            .unwrap_or_else(|| panic!("{toolchain}: no manifest returned"));

        // The published file this toolchain resolves to.
        let path = src.join(
            name.manifest_v3_url("", &tp.process)
                .unwrap()
                .trim_start_matches('/'),
        );
        let bytes = fs::read(&path).unwrap_or_else(|err| panic!("{toolchain}: {err}"));
        let expected = Manifest::parse(std::str::from_utf8(&bytes).unwrap()).unwrap();
        assert_eq!(downloaded.manifest.date, expected.date, "{toolchain}");
        assert_eq!(
            downloaded.hash,
            faster_hex::hex_string(&Sha256::digest(&bytes))[..downloaded.hash.len()],
            "{toolchain}: hash of the verified bytes"
        );
        fetched.insert(path);
    }

    let mut existing = BTreeSet::new();
    manifests(&src.join("channels"), &mut existing);
    assert_eq!(
        fetched, existing,
        "every manifest in tests/repo/src must be reachable through TUF"
    );
}

#[tokio::test]
async fn unchanged_manifest_is_skipped_by_update_hash() {
    let root = tmp_dir();
    let tp = test_process(root.path().to_owned());
    let cfg = Cfg::from_env(root.path().to_owned(), false, true, &tp.process).unwrap();
    let dl_cfg = DownloadCfg::new(&cfg);
    let host = TargetTuple::from_host_or_build(&tp.process);
    let name = ChannelToolchainName::from_str(&format!("stable-{host}")).unwrap();
    let update_hash = root.path().join("stable.hash");

    let first = dl_cfg
        .dl_v2_manifest(Some(&update_hash), &name, &cfg)
        .await
        .unwrap()
        .expect("nothing recorded yet, so the manifest is returned");
    fs::write(&update_hash, &first.hash).unwrap();

    let second = dl_cfg
        .dl_v2_manifest(Some(&update_hash), &name, &cfg)
        .await
        .unwrap();
    assert!(
        second.is_none(),
        "same hash on record, so nothing to update"
    );
}
