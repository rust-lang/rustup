//! A TUF repository signed over the mock dist and self-update servers.

use std::{
    fs,
    path::{Path, PathBuf},
};

use futures_util::io::Cursor;
use tempfile::TempDir;
use tuf::{
    crypto::{Ed25519PrivateKey, PrivateKey},
    metadata::TargetPath,
    pouf::Pouf1,
    repo_builder::RepoBuilder,
    repository::FileSystemRepository,
};

/// A dist server that cannot be reached, so a manifest can only arrive through TUF.
const UNREACHABLE_DIST_SERVER: &str = "https://dist.invalid";
/// Likewise for rustup's own release file and binaries.
const UNREACHABLE_UPDATE_ROOT: &str = "https://dist.invalid/rustup";

/// A TUF repository over a mock dist server, and optionally a mock self-update
/// server, signed with throwaway keys.
///
/// Manifests are published in the `channels/` layout that rustup looks them up
/// in with TUF enabled, and self-update files under `rustup/`. Call
/// [`MockTufServer::publish`] again after changing the mocks; each publication
/// is a fresh repository and a fresh metadata cache.
pub struct MockTufServer {
    dist: PathBuf,
    self_dist: Option<PathBuf>,
    keys: Vec<Ed25519PrivateKey>,
    generation: usize,
    repo: String,
    root: String,
    home: String,
    _dir: TempDir,
}

impl MockTufServer {
    pub async fn new(dist: &Path, self_dist: Option<&Path>) -> Self {
        let dir = tempfile::Builder::new()
            .prefix("rustup-tuf-mock")
            .tempdir()
            .unwrap();
        let keys = (0..4)
            .map(|_| Ed25519PrivateKey::from_pkcs8(&Ed25519PrivateKey::pkcs8().unwrap()).unwrap())
            .collect();
        let mut server = Self {
            dist: dist.to_owned(),
            self_dist: self_dist.map(Path::to_owned),
            keys,
            generation: 0,
            repo: String::new(),
            root: String::new(),
            home: String::new(),
            _dir: dir,
        };
        server.publish().await;
        server
    }

    /// Signs the mocks' current contents into a new repository.
    pub async fn publish(&mut self) {
        self.generation += 1;
        let repo = self._dir.path().join(format!("repo-{}", self.generation));
        let home = self._dir.path().join(format!("home-{}", self.generation));
        fs::create_dir_all(&repo).unwrap();
        fs::create_dir_all(&home).unwrap();

        let mut storage = FileSystemRepository::<Pouf1>::new(&repo);
        let mut builder = RepoBuilder::create(&mut storage)
            .trusted_root_keys(&[&self.keys[0] as &dyn PrivateKey])
            .trusted_targets_keys(&[&self.keys[1] as &dyn PrivateKey])
            .trusted_snapshot_keys(&[&self.keys[2] as &dyn PrivateKey])
            .trusted_timestamp_keys(&[&self.keys[3] as &dyn PrivateKey])
            .stage_root()
            .unwrap();
        for (target, bytes) in self.targets() {
            builder = builder
                .add_target(TargetPath::new(target).unwrap(), Cursor::new(bytes))
                .await
                .unwrap();
        }
        builder.commit().await.unwrap();

        self.root = repo
            .join("metadata/1.root.json")
            .to_string_lossy()
            .into_owned();
        self.repo = repo.to_string_lossy().into_owned();
        self.home = home.to_string_lossy().into_owned();
    }

    /// Environment for a rustup command that must get its manifests, release
    /// file and binaries through this repository, verified in `mode`.
    pub fn env(&self, mode: &'static str) -> [(&str, &str); 6] {
        let [enable, server, root, home] = self.settings(mode);
        [
            enable,
            server,
            root,
            home,
            ("RUSTUP_DIST_SERVER", UNREACHABLE_DIST_SERVER),
            ("RUSTUP_UPDATE_ROOT", UNREACHABLE_UPDATE_ROOT),
        ]
    }

    /// Just the TUF settings, leaving the mock servers reachable.
    pub fn settings(&self, mode: &'static str) -> [(&str, &str); 4] {
        [
            ("RUSTUP_TUF_ENABLE", mode),
            ("RUSTUP_TUF_SERVER", &self.repo),
            ("RUSTUP_TUF_ROOT", &self.root),
            ("RUSTUP_TUF_HOME", &self.home),
        ]
    }

    /// Alters the published copy of `target` after signing, so it no longer
    /// matches the metadata.
    pub fn tamper(&self, target: &str) {
        let (dir, name) = target.rsplit_once('/').unwrap();
        let dir = Path::new(&self.repo).join("targets").join(dir);
        let path = fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .is_some_and(|file| file.to_string_lossy().ends_with(&format!(".{name}")))
            })
            .unwrap_or_else(|| panic!("no published copy of {target} in {}", dir.display()));
        let mut bytes = fs::read(&path).unwrap();
        bytes.extend_from_slice(b"\n# tampered after signing\n");
        fs::write(&path, bytes).unwrap();
    }

    /// Every target to publish, named as rustup will ask for it.
    fn targets(&self) -> Vec<(String, Vec<u8>)> {
        let mut targets = Vec::new();
        let dist = self.dist.join("dist");
        for entry in fs::read_dir(&dist).unwrap() {
            let path = entry.unwrap().path();
            let file = path.file_name().unwrap().to_string_lossy().into_owned();
            if path.is_dir() {
                // `<date>/channel-rust-<channel>.toml`
                let Some((year, month_day)) = file.split_once('-') else {
                    continue;
                };
                for entry in fs::read_dir(&path).unwrap() {
                    let path = entry.unwrap().path();
                    let file = path.file_name().unwrap().to_string_lossy().into_owned();
                    if let Some(channel) = manifest_channel(&file) {
                        targets.push((
                            format!("channels/nightly/{year}/{month_day}/{channel}.toml"),
                            fs::read(&path).unwrap(),
                        ));
                    }
                }
            } else if let Some(channel) = manifest_channel(&file) {
                targets.push((
                    format!("channels/current/{channel}.toml"),
                    fs::read(&path).unwrap(),
                ));
            }
        }

        if let Some(self_dist) = &self.self_dist {
            targets.push((
                "rustup/release-stable.toml".to_owned(),
                fs::read(self_dist.join("release-stable.toml")).unwrap(),
            ));
            // `archive/<version>/<target>/rustup-init` is published as
            // `<version>/<target>/rustup-init`.
            for version in fs::read_dir(self_dist.join("archive")).unwrap() {
                let version = version.unwrap().path();
                let release = version.file_name().unwrap().to_string_lossy().into_owned();
                for triple in fs::read_dir(&version).unwrap() {
                    let triple = triple.unwrap().path();
                    let name = triple.file_name().unwrap().to_string_lossy().into_owned();
                    for bin in fs::read_dir(&triple).unwrap() {
                        let bin = bin.unwrap().path();
                        let file = bin.file_name().unwrap().to_string_lossy().into_owned();
                        if !file.starts_with("rustup-init") || file.contains("tmp") {
                            continue;
                        }
                        targets.push((
                            format!("rustup/{release}/{name}/{file}"),
                            fs::read(&bin).unwrap(),
                        ));
                    }
                }
            }
        }
        targets
    }
}

/// The channel of a v2 manifest file name, `channel-rust-<channel>.toml`.
fn manifest_channel(file: &str) -> Option<&str> {
    file.strip_prefix("channel-rust-")?.strip_suffix(".toml")
}
