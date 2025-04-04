// Tests of installation and updates from a v2 Rust distribution
// server (mocked on the file system)
#![allow(clippy::type_complexity)]

use std::{
    cell::Cell,
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};

use anyhow::{Result, anyhow};
use url::Url;

use crate::{
    dist::{
        DEFAULT_DIST_SERVER, Notification, Profile, TargetTriple, ToolchainDesc,
        download::DownloadCfg,
        manifest::{Component, Manifest},
        manifestation::{Changes, Manifestation, UpdateStatus},
        prefix::InstallPrefix,
        temp,
    },
    download::download_file,
    errors::RustupError,
    process::TestProcess,
    test::{
        dist::*,
        mock::{MockComponentBuilder, MockFile, MockInstallerBuilder},
    },
    utils::{self, raw as utils_raw},
};

const SHA256_HASH_LEN: usize = 64;

// Creates a mock dist server populated with some test data
fn create_mock_dist_server(
    path: &Path,
    edit: Option<&dyn Fn(&str, &mut MockChannel)>,
) -> MockDistServer {
    MockDistServer {
        path: path.to_owned(),
        channels: vec![
            create_mock_channel("nightly", "2016-02-01", edit),
            create_mock_channel("nightly", "2016-02-02", edit),
        ],
    }
}

fn create_mock_channel(
    channel: &str,
    date: &str,
    edit: Option<&dyn Fn(&str, &mut MockChannel)>,
) -> MockChannel {
    // Put the date in the files so they can be differentiated
    let contents = Arc::new(date.as_bytes().to_vec());

    let mut packages = Vec::with_capacity(5);

    packages.push(MockPackage {
        name: "rust",
        version: "1.0.0".to_string(),
        targets: vec![
            MockTargetedPackage {
                target: "x86_64-apple-darwin".to_string(),
                available: true,
                components: vec![
                    MockComponent {
                        name: "rustc".to_string(),
                        target: "x86_64-apple-darwin".to_string(),
                        is_extension: false,
                    },
                    MockComponent {
                        name: "cargo".to_string(),
                        target: "x86_64-apple-darwin".to_string(),
                        is_extension: false,
                    },
                    MockComponent {
                        name: "rust-std".to_string(),
                        target: "x86_64-apple-darwin".to_string(),
                        is_extension: false,
                    },
                    MockComponent {
                        name: "rust-std".to_string(),
                        target: "i686-apple-darwin".to_string(),
                        is_extension: false,
                    },
                    MockComponent {
                        name: "rust-std".to_string(),
                        target: "i686-unknown-linux-gnu".to_string(),
                        is_extension: false,
                    },
                ],
                installer: MockInstallerBuilder { components: vec![] },
            },
            MockTargetedPackage {
                target: "i686-apple-darwin".to_string(),
                available: true,
                components: vec![
                    MockComponent {
                        name: "rustc".to_string(),
                        target: "i686-apple-darwin".to_string(),
                        is_extension: false,
                    },
                    MockComponent {
                        name: "cargo".to_string(),
                        target: "i686-apple-darwin".to_string(),
                        is_extension: false,
                    },
                    MockComponent {
                        name: "rust-std".to_string(),
                        target: "i686-apple-darwin".to_string(),
                        is_extension: false,
                    },
                ],
                installer: MockInstallerBuilder { components: vec![] },
            },
        ],
    });

    for bin in &["bin/rustc", "bin/cargo"] {
        let pkg = &bin[4..];
        packages.push(MockPackage {
            name: pkg,
            version: "1.0.0".to_string(),
            targets: vec![
                MockTargetedPackage {
                    target: "x86_64-apple-darwin".to_string(),
                    available: true,
                    components: vec![],
                    installer: MockInstallerBuilder {
                        components: vec![MockComponentBuilder {
                            name: pkg.to_string(),
                            files: vec![MockFile::new_arc(*bin, contents.clone())],
                        }],
                    },
                },
                MockTargetedPackage {
                    target: "i686-apple-darwin".to_string(),
                    available: true,
                    components: vec![],
                    installer: MockInstallerBuilder { components: vec![] },
                },
            ],
        });
    }

    packages.push(MockPackage {
        name: "rust-std",
        version: "1.0.0".to_string(),
        targets: vec![
            MockTargetedPackage {
                target: "x86_64-apple-darwin".to_string(),
                available: true,
                components: vec![],
                installer: MockInstallerBuilder {
                    components: vec![MockComponentBuilder {
                        name: "rust-std-x86_64-apple-darwin".to_string(),
                        files: vec![MockFile::new_arc("lib/libstd.rlib", contents.clone())],
                    }],
                },
            },
            MockTargetedPackage {
                target: "i686-apple-darwin".to_string(),
                available: true,
                components: vec![],
                installer: MockInstallerBuilder {
                    components: vec![MockComponentBuilder {
                        name: "rust-std-i686-apple-darwin".to_string(),
                        files: vec![MockFile::new_arc(
                            "lib/i686-apple-darwin/libstd.rlib",
                            contents.clone(),
                        )],
                    }],
                },
            },
            MockTargetedPackage {
                target: "i686-unknown-linux-gnu".to_string(),
                available: true,
                components: vec![],
                installer: MockInstallerBuilder {
                    components: vec![MockComponentBuilder {
                        name: "rust-std-i686-unknown-linux-gnu".to_string(),
                        files: vec![MockFile::new_arc(
                            "lib/i686-unknown-linux-gnu/libstd.rlib",
                            contents.clone(),
                        )],
                    }],
                },
            },
        ],
    });

    // An extra package that can be used as a component of the other packages
    // for various tests
    packages.push(bonus_component("bonus", contents));

    let mut channel = MockChannel {
        name: channel.to_string(),
        date: date.to_string(),
        packages,
        renames: HashMap::new(),
    };

    if let Some(edit) = edit {
        edit(date, &mut channel);
    }

    channel
}

fn bonus_component(name: &'static str, contents: Arc<Vec<u8>>) -> MockPackage {
    MockPackage {
        name,
        version: "1.0.0".to_string(),
        targets: vec![MockTargetedPackage {
            target: "x86_64-apple-darwin".to_string(),
            available: true,
            components: vec![],
            installer: MockInstallerBuilder {
                components: vec![MockComponentBuilder {
                    name: format!("{name}-x86_64-apple-darwin"),
                    files: vec![MockFile::new_arc("bin/bonus", contents)],
                }],
            },
        }],
    }
}

#[test]
fn mock_dist_server_smoke_test() {
    let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let path = tempdir.path();

    create_mock_dist_server(path, None).write(&[MockManifestVersion::V2], false, false);

    assert!(utils::path_exists(path.join(
        "dist/2016-02-01/rustc-nightly-x86_64-apple-darwin.tar.gz"
    )));
    assert!(utils::path_exists(
        path.join("dist/2016-02-01/rustc-nightly-i686-apple-darwin.tar.gz")
    ));
    assert!(utils::path_exists(path.join(
        "dist/2016-02-01/rust-std-nightly-x86_64-apple-darwin.tar.gz"
    )));
    assert!(utils::path_exists(path.join(
        "dist/2016-02-01/rust-std-nightly-i686-apple-darwin.tar.gz"
    )));
    assert!(utils::path_exists(path.join(
        "dist/2016-02-01/rustc-nightly-x86_64-apple-darwin.tar.gz.sha256"
    )));
    assert!(utils::path_exists(path.join(
        "dist/2016-02-01/rustc-nightly-i686-apple-darwin.tar.gz.sha256"
    )));
    assert!(utils::path_exists(path.join(
        "dist/2016-02-01/rust-std-nightly-x86_64-apple-darwin.tar.gz.sha256"
    )));
    assert!(utils::path_exists(path.join(
        "dist/2016-02-01/rust-std-nightly-i686-apple-darwin.tar.gz.sha256"
    )));
    assert!(utils::path_exists(
        path.join("dist/channel-rust-nightly.toml")
    ));
    assert!(utils::path_exists(
        path.join("dist/channel-rust-nightly.toml.sha256")
    ));
}

// Test that a standard rename works - the component is installed with the old name, then renamed
// the next day to the new name.
#[tokio::test]
async fn rename_component() {
    let dist_tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let url = Url::parse(&format!("file://{}", dist_tempdir.path().to_string_lossy())).unwrap();

    let edit_1 = &|_: &str, chan: &mut MockChannel| {
        let tpkg = chan.packages[0]
            .targets
            .iter_mut()
            .find(|p| p.target == "x86_64-apple-darwin")
            .unwrap();
        tpkg.components.push(MockComponent {
            name: "bonus".to_string(),
            target: "x86_64-apple-darwin".to_string(),
            is_extension: true,
        });
    };
    let edit_2 = &|_: &str, chan: &mut MockChannel| {
        let tpkg = chan.packages[0]
            .targets
            .iter_mut()
            .find(|p| p.target == "x86_64-apple-darwin")
            .unwrap();
        tpkg.components.push(MockComponent {
            name: "bobo".to_string(),
            target: "x86_64-apple-darwin".to_string(),
            is_extension: true,
        });
    };

    let date_2 = "2016-02-02";
    let mut channel_2 = create_mock_channel("nightly", date_2, Some(edit_2));
    channel_2.packages[4] = bonus_component("bobo", Arc::new(date_2.as_bytes().to_vec()));
    channel_2
        .renames
        .insert("bonus".to_owned(), "bobo".to_owned());
    let mock_dist_server = MockDistServer {
        path: dist_tempdir.path().to_owned(),
        channels: vec![
            create_mock_channel("nightly", "2016-02-01", Some(edit_1)),
            channel_2,
        ],
    };

    let cx = TestContext::from_dist_server(mock_dist_server, url, GZOnly);

    let adds = [Component::new(
        "bonus".to_string(),
        Some(TargetTriple::new("x86_64-apple-darwin")),
        true,
    )];

    change_channel_date(&cx.url, "nightly", "2016-02-01");
    cx.update_from_dist(&adds, &[], false).await.unwrap();
    assert!(utils::path_exists(cx.prefix.path().join("bin/bonus")));
    assert!(!utils::path_exists(cx.prefix.path().join("bin/bobo")));
    change_channel_date(&cx.url, "nightly", "2016-02-02");
    cx.update_from_dist(&[], &[], false).await.unwrap();
    assert!(utils::path_exists(cx.prefix.path().join("bin/bonus")));
    assert!(!utils::path_exists(cx.prefix.path().join("bin/bobo")));
}

// Test that a rename is ignored if the component with the old name was never installed.
#[tokio::test]
async fn rename_component_new() {
    let dist_tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let url = Url::parse(&format!("file://{}", dist_tempdir.path().to_string_lossy())).unwrap();

    let date_2 = "2016-02-02";
    let mut channel_2 = create_mock_channel("nightly", date_2, None);
    // Replace the `bonus` component with a `bobo` component
    channel_2.packages[4] = bonus_component("bobo", Arc::new(date_2.as_bytes().to_vec()));
    // And allow a rename from `bonus` to `bobo`
    channel_2
        .renames
        .insert("bonus".to_owned(), "bobo".to_owned());
    let mock_dist_server = MockDistServer {
        path: dist_tempdir.path().to_owned(),
        channels: vec![
            create_mock_channel("nightly", "2016-02-01", None),
            channel_2,
        ],
    };

    let cx = TestContext::from_dist_server(mock_dist_server, url, GZOnly);

    let adds = [Component::new(
        "bobo".to_string(),
        Some(TargetTriple::new("x86_64-apple-darwin")),
        true,
    )];
    // Install the basics from day 1
    change_channel_date(&cx.url, "nightly", "2016-02-01");
    cx.update_from_dist(&[], &[], false).await.unwrap();
    // Neither bonus nor bobo are installed at this point.
    assert!(!utils::path_exists(cx.prefix.path().join("bin/bonus")));
    assert!(!utils::path_exists(cx.prefix.path().join("bin/bobo")));
    // Now we move to day 2, where bobo is part of the set of things we want
    // to have installed
    change_channel_date(&cx.url, "nightly", "2016-02-02");
    cx.update_from_dist(&adds, &[], false).await.unwrap();
    // As a result `bin/bonus` is present but not `bin/bobo` which we'd
    // expect since the bonus component installs `bin/bonus` regardless of
    // its name being `bobo`
    assert!(!utils::path_exists(cx.prefix.path().join("bin/bobo")));
    assert!(utils::path_exists(cx.prefix.path().join("bin/bonus")));
}

fn make_manifest_url(dist_server: &Url, toolchain: &ToolchainDesc) -> Result<Url> {
    let url = format!(
        "{}/dist/channel-rust-{}.toml",
        dist_server, toolchain.channel
    );

    Url::parse(&url).map_err(|e| anyhow!(format!("{e:?}")))
}

#[derive(Copy, Clone, Debug)]
enum Compressions {
    GZOnly,
    AddXZ,
    AddZStd,
}
use Compressions::*;

impl Compressions {
    fn enable_xz(self) -> bool {
        matches!(self, AddXZ)
    }

    fn enable_zst(self) -> bool {
        matches!(self, AddZStd)
    }
}

struct TestContext {
    url: Url,
    toolchain: ToolchainDesc,
    prefix: InstallPrefix,
    download_dir: PathBuf,
    tp: TestProcess,
    tmp_cx: temp::Context,
    _tempdirs: Vec<tempfile::TempDir>,
}

impl TestContext {
    fn new(edit: Option<&dyn Fn(&str, &mut MockChannel)>, comps: Compressions) -> Self {
        let dist_tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
        let mock_dist_server = create_mock_dist_server(dist_tempdir.path(), edit);
        let url = Url::parse(&format!("file://{}", dist_tempdir.path().to_string_lossy())).unwrap();

        let mut cx = Self::from_dist_server(mock_dist_server, url, comps);
        cx._tempdirs.push(dist_tempdir);
        cx
    }

    fn from_dist_server(server: MockDistServer, url: Url, comps: Compressions) -> Self {
        server.write(
            &[MockManifestVersion::V2],
            comps.enable_xz(),
            comps.enable_zst(),
        );

        let prefix_tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();

        let work_tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
        let tmp_cx = temp::Context::new(
            work_tempdir.path().to_owned(),
            DEFAULT_DIST_SERVER,
            Box::new(|_| ()),
        );

        let toolchain = ToolchainDesc::from_str("nightly-x86_64-apple-darwin").unwrap();
        let prefix = InstallPrefix::from(prefix_tempdir.path());
        let tp = TestProcess::new(
            env::current_dir().unwrap(),
            &["rustup"],
            HashMap::default(),
            "",
        );

        Self {
            url,
            toolchain,
            download_dir: prefix.path().join("downloads"),
            prefix,
            tp,
            tmp_cx,
            _tempdirs: vec![prefix_tempdir, work_tempdir],
        }
    }

    fn default_dl_cfg(&self) -> DownloadCfg<'_> {
        DownloadCfg {
            dist_root: "phony",
            tmp_cx: &self.tmp_cx,
            download_dir: &self.download_dir,
            notify_handler: &|event| println!("{event}"),
            process: &self.tp.process,
        }
    }

    // Installs or updates a toolchain from a dist server.  If an initial
    // install then it will be installed with the default components.  If
    // an upgrade then all the existing components will be upgraded.
    // FIXME: Unify this with dist::update_from_dist
    async fn update_from_dist(
        &self,
        add: &[Component],
        remove: &[Component],
        force: bool,
    ) -> Result<UpdateStatus> {
        self.update_from_dist_with_dl_cfg(add, remove, force, &self.default_dl_cfg())
            .await
    }

    async fn update_from_dist_with_dl_cfg(
        &self,
        add: &[Component],
        remove: &[Component],
        force: bool,
        dl_cfg: &DownloadCfg<'_>,
    ) -> Result<UpdateStatus> {
        // Download the dist manifest and place it into the installation prefix
        let manifest_url = make_manifest_url(&self.url, &self.toolchain)?;
        let manifest_file = self.tmp_cx.new_file()?;
        download_file(&manifest_url, &manifest_file, None, &|_| {}, dl_cfg.process).await?;
        let manifest_str = utils::read_file("manifest", &manifest_file)?;
        let manifest = Manifest::parse(&manifest_str)?;

        // Read the manifest to update the components
        let trip = self.toolchain.target.clone();
        let manifestation = Manifestation::open(self.prefix.clone(), trip.clone())?;

        // TODO on install, need to add profile components (but I guess we shouldn't test that logic here)
        let mut profile_components = manifest.get_profile_components(Profile::Default, &trip)?;
        let mut add_components = add.to_owned();
        add_components.append(&mut profile_components);

        let changes = Changes {
            explicit_add_components: add_components,
            remove_components: remove.to_owned(),
        };

        manifestation
            .update(
                &manifest,
                changes,
                force,
                dl_cfg,
                &self.toolchain.manifest_name(),
                true,
            )
            .await
    }

    fn uninstall(&self) -> Result<()> {
        let trip = self.toolchain.target.clone();
        let manifestation = Manifestation::open(self.prefix.clone(), trip)?;
        let manifest = manifestation.load_manifest()?.unwrap();

        manifestation.uninstall(&manifest, &self.tmp_cx, &|_| (), &self.tp.process)?;

        Ok(())
    }
}

async fn initial_install(comps: Compressions) {
    let cx = TestContext::new(None, comps);
    cx.update_from_dist(&[], &[], false).await.unwrap();

    assert!(utils::path_exists(cx.prefix.path().join("bin/rustc")));
    assert!(utils::path_exists(cx.prefix.path().join("lib/libstd.rlib")));
}

#[tokio::test]
async fn initial_install_gziponly() {
    initial_install(GZOnly).await;
}

#[tokio::test]
async fn initial_install_xz() {
    initial_install(AddXZ).await;
}

#[tokio::test]
async fn initial_install_zst() {
    initial_install(AddZStd).await;
}

#[tokio::test]
async fn test_uninstall() {
    let cx = TestContext::new(None, GZOnly);
    cx.update_from_dist(&[], &[], false).await.unwrap();
    cx.uninstall().unwrap();

    assert!(!utils::path_exists(cx.prefix.path().join("bin/rustc")));
    assert!(!utils::path_exists(
        cx.prefix.path().join("lib/libstd.rlib")
    ));
}

#[tokio::test]
async fn uninstall_removes_config_file() {
    let cx = TestContext::new(None, GZOnly);
    cx.update_from_dist(&[], &[], false).await.unwrap();
    assert!(utils::path_exists(
        cx.prefix.manifest_file("multirust-config.toml")
    ));
    cx.uninstall().unwrap();
    assert!(!utils::path_exists(
        cx.prefix.manifest_file("multirust-config.toml")
    ));
}

#[tokio::test]
async fn upgrade() {
    let cx = TestContext::new(None, GZOnly);
    change_channel_date(&cx.url, "nightly", "2016-02-01");
    cx.update_from_dist(&[], &[], false).await.unwrap();
    assert_eq!(
        "2016-02-01",
        fs::read_to_string(cx.prefix.path().join("bin/rustc")).unwrap()
    );
    change_channel_date(&cx.url, "nightly", "2016-02-02");
    cx.update_from_dist(&[], &[], false).await.unwrap();
    assert_eq!(
        "2016-02-02",
        fs::read_to_string(cx.prefix.path().join("bin/rustc")).unwrap()
    );
}

#[tokio::test]
async fn unavailable_component() {
    // On day 2 the bonus component is no longer available
    let edit = &|date: &str, chan: &mut MockChannel| {
        // Require the bonus component every day.
        {
            let tpkg = chan.packages[0]
                .targets
                .iter_mut()
                .find(|p| p.target == "x86_64-apple-darwin")
                .unwrap();
            tpkg.components.push(MockComponent {
                name: "bonus".to_string(),
                target: "x86_64-apple-darwin".to_string(),
                is_extension: true,
            });
        }

        // Mark the bonus package as unavailable in 2016-02-02
        if date == "2016-02-02" {
            let bonus_pkg = chan
                .packages
                .iter_mut()
                .find(|p| p.name == "bonus")
                .unwrap();

            for target in &mut bonus_pkg.targets {
                target.available = false;
            }
        }
    };

    let cx = TestContext::new(Some(edit), GZOnly);
    let adds = [Component::new(
        "bonus".to_string(),
        Some(TargetTriple::new("x86_64-apple-darwin")),
        true,
    )];

    change_channel_date(&cx.url, "nightly", "2016-02-01");
    // Update with bonus.
    cx.update_from_dist(&adds, &[], false).await.unwrap();
    assert!(utils::path_exists(cx.prefix.path().join("bin/bonus")));
    change_channel_date(&cx.url, "nightly", "2016-02-02");

    // Update without bonus, should fail.
    let err = cx.update_from_dist(&[], &[], false).await.unwrap_err();
    match err.downcast::<RustupError>() {
        Ok(RustupError::RequestedComponentsUnavailable {
            components,
            manifest,
            toolchain,
        }) => {
            assert_eq!(toolchain, "nightly");
            let descriptions = components
                .iter()
                .map(|c| c.description(&manifest))
                .collect::<Vec<_>>();
            assert_eq!(descriptions, ["'bonus' for target 'x86_64-apple-darwin'"])
        }
        _ => panic!(),
    }
}

// As unavailable_component, but the unavailable component is part of the profile.
#[tokio::test]
async fn unavailable_component_from_profile() {
    // On day 2 the rustc component is no longer available
    let edit = &|date: &str, chan: &mut MockChannel| {
        // Mark the rustc package as unavailable in 2016-02-02
        if date == "2016-02-02" {
            let rustc_pkg = chan
                .packages
                .iter_mut()
                .find(|p| p.name == "rustc")
                .unwrap();

            for target in &mut rustc_pkg.targets {
                target.available = false;
            }
        }
    };

    let cx = TestContext::new(Some(edit), GZOnly);
    change_channel_date(&cx.url, "nightly", "2016-02-01");
    // Update with rustc.
    cx.update_from_dist(&[], &[], false).await.unwrap();
    assert!(utils::path_exists(cx.prefix.path().join("bin/rustc")));
    change_channel_date(&cx.url, "nightly", "2016-02-02");

    // Update without rustc, should fail.
    let err = cx.update_from_dist(&[], &[], false).await.unwrap_err();
    match err.downcast::<RustupError>() {
        Ok(RustupError::RequestedComponentsUnavailable {
            components,
            manifest,
            toolchain,
        }) => {
            assert_eq!(toolchain, "nightly");
            let descriptions = components
                .iter()
                .map(|c| c.description(&manifest))
                .collect::<Vec<_>>();
            assert_eq!(descriptions, ["'rustc' for target 'x86_64-apple-darwin'"])
        }
        _ => panic!(),
    }

    cx.update_from_dist(&[], &[], true).await.unwrap();
}

#[tokio::test]
async fn removed_component() {
    // On day 1 install the 'bonus' component, on day 2 it's no longer a component
    let edit = &|date: &str, chan: &mut MockChannel| {
        if date == "2016-02-01" {
            let tpkg = chan.packages[0]
                .targets
                .iter_mut()
                .find(|p| p.target == "x86_64-apple-darwin")
                .unwrap();
            tpkg.components.push(MockComponent {
                name: "bonus".to_string(),
                target: "x86_64-apple-darwin".to_string(),
                is_extension: true,
            });
        } else {
            chan.packages.retain(|p| p.name != "bonus");
        }
    };

    let cx = TestContext::new(Some(edit), GZOnly);
    let adds = [Component::new(
        "bonus".to_string(),
        Some(TargetTriple::new("x86_64-apple-darwin")),
        true,
    )];

    // Update with bonus.
    change_channel_date(&cx.url, "nightly", "2016-02-01");
    cx.update_from_dist(&adds, &[], false).await.unwrap();
    assert!(utils::path_exists(cx.prefix.path().join("bin/bonus")));

    // Update without bonus, should fail with RequestedComponentsUnavailable
    change_channel_date(&cx.url, "nightly", "2016-02-02");
    let err = cx.update_from_dist(&[], &[], false).await.unwrap_err();
    match err.downcast::<RustupError>() {
        Ok(RustupError::RequestedComponentsUnavailable {
            components,
            manifest,
            toolchain,
        }) => {
            assert_eq!(toolchain, "nightly");
            let descriptions = components
                .iter()
                .map(|c| c.description(&manifest))
                .collect::<Vec<_>>();
            assert_eq!(descriptions, ["'bonus' for target 'x86_64-apple-darwin'"])
        }
        _ => panic!(),
    }
}

#[tokio::test]
async fn unavailable_components_is_target() {
    // On day 2 the rust-std component is no longer available
    let edit = &|date: &str, chan: &mut MockChannel| {
        // Mark the rust-std package as unavailable in 2016-02-02
        if date == "2016-02-02" {
            let pkg = chan
                .packages
                .iter_mut()
                .find(|p| p.name == "rust-std")
                .unwrap();

            for target in &mut pkg.targets {
                target.available = false;
            }
        }
    };

    let cx = TestContext::new(Some(edit), GZOnly);
    let adds = [
        Component::new(
            "rust-std".to_string(),
            Some(TargetTriple::new("i686-apple-darwin")),
            false,
        ),
        Component::new(
            "rust-std".to_string(),
            Some(TargetTriple::new("i686-unknown-linux-gnu")),
            false,
        ),
    ];

    // Update with rust-std
    change_channel_date(&cx.url, "nightly", "2016-02-01");
    cx.update_from_dist(&adds, &[], false).await.unwrap();
    assert!(utils::path_exists(
        cx.prefix.path().join("lib/i686-apple-darwin/libstd.rlib")
    ));
    assert!(utils::path_exists(
        cx.prefix
            .path()
            .join("lib/i686-unknown-linux-gnu/libstd.rlib")
    ));

    // Update without rust-std
    change_channel_date(&cx.url, "nightly", "2016-02-02");
    let err = cx.update_from_dist(&[], &[], false).await.unwrap_err();
    match err.downcast::<RustupError>() {
        Ok(RustupError::RequestedComponentsUnavailable {
            components,
            manifest,
            toolchain,
        }) => {
            assert_eq!(toolchain, "nightly");
            let descriptions = components
                .iter()
                .map(|c| c.description(&manifest))
                .collect::<Vec<_>>();
            assert_eq!(
                descriptions,
                [
                    "'rust-std' for target 'x86_64-apple-darwin'",
                    "'rust-std' for target 'i686-apple-darwin'",
                    "'rust-std' for target 'i686-unknown-linux-gnu'"
                ]
            );
        }
        _ => panic!(),
    }
}

#[tokio::test]
async fn unavailable_components_with_same_target() {
    // On day 2, the rust-std and rustc components are no longer available
    let edit = &|date: &str, chan: &mut MockChannel| {
        // Mark the rust-std package as unavailable in 2016-02-02
        if date == "2016-02-02" {
            let pkg = chan
                .packages
                .iter_mut()
                .find(|p| p.name == "rust-std")
                .unwrap();

            for target in &mut pkg.targets {
                target.available = false;
            }
        }

        // Mark the rustc package as unavailable in 2016-02-02
        if date == "2016-02-02" {
            let pkg = chan
                .packages
                .iter_mut()
                .find(|p| p.name == "rustc")
                .unwrap();

            for target in &mut pkg.targets {
                target.available = false;
            }
        }
    };

    let cx = TestContext::new(Some(edit), GZOnly);
    // Update with rust-std and rustc
    change_channel_date(&cx.url, "nightly", "2016-02-01");
    cx.update_from_dist(&[], &[], false).await.unwrap();
    assert!(utils::path_exists(cx.prefix.path().join("bin/rustc")));
    assert!(utils::path_exists(cx.prefix.path().join("lib/libstd.rlib")));

    // Update without rust-std and rustc
    change_channel_date(&cx.url, "nightly", "2016-02-02");
    let err = cx.update_from_dist(&[], &[], false).await.unwrap_err();
    match err.downcast::<RustupError>() {
        Ok(RustupError::RequestedComponentsUnavailable {
            components,
            manifest,
            toolchain,
        }) => {
            assert_eq!(toolchain, "nightly");
            let descriptions = components
                .iter()
                .map(|c| c.description(&manifest))
                .collect::<Vec<_>>();
            assert_eq!(
                descriptions,
                [
                    "'rustc' for target 'x86_64-apple-darwin'",
                    "'rust-std' for target 'x86_64-apple-darwin'"
                ]
            );
        }
        _ => panic!(),
    }
}

#[tokio::test]
async fn update_preserves_extensions() {
    let cx = TestContext::new(None, GZOnly);

    let adds = vec![
        Component::new(
            "rust-std".to_string(),
            Some(TargetTriple::new("i686-apple-darwin")),
            false,
        ),
        Component::new(
            "rust-std".to_string(),
            Some(TargetTriple::new("i686-unknown-linux-gnu")),
            false,
        ),
    ];

    change_channel_date(&cx.url, "nightly", "2016-02-01");
    cx.update_from_dist(&adds, &[], false).await.unwrap();

    assert!(utils::path_exists(
        cx.prefix.path().join("lib/i686-apple-darwin/libstd.rlib")
    ));
    assert!(utils::path_exists(
        cx.prefix
            .path()
            .join("lib/i686-unknown-linux-gnu/libstd.rlib")
    ));

    change_channel_date(&cx.url, "nightly", "2016-02-02");
    cx.update_from_dist(&[], &[], false).await.unwrap();

    assert!(utils::path_exists(
        cx.prefix.path().join("lib/i686-apple-darwin/libstd.rlib")
    ));
    assert!(utils::path_exists(
        cx.prefix
            .path()
            .join("lib/i686-unknown-linux-gnu/libstd.rlib")
    ));
}

#[tokio::test]
async fn update_makes_no_changes_for_identical_manifest() {
    let cx = TestContext::new(None, GZOnly);
    let status = cx.update_from_dist(&[], &[], false).await.unwrap();
    assert_eq!(status, UpdateStatus::Changed);
    let status = cx.update_from_dist(&[], &[], false).await.unwrap();
    assert_eq!(status, UpdateStatus::Unchanged);
}

#[tokio::test]
async fn add_extensions_for_initial_install() {
    let cx = TestContext::new(None, GZOnly);
    let adds = vec![
        Component::new(
            "rust-std".to_string(),
            Some(TargetTriple::new("i686-apple-darwin")),
            false,
        ),
        Component::new(
            "rust-std".to_string(),
            Some(TargetTriple::new("i686-unknown-linux-gnu")),
            false,
        ),
    ];

    cx.update_from_dist(&adds, &[], false).await.unwrap();
    assert!(utils::path_exists(
        cx.prefix.path().join("lib/i686-apple-darwin/libstd.rlib")
    ));
    assert!(utils::path_exists(
        cx.prefix
            .path()
            .join("lib/i686-unknown-linux-gnu/libstd.rlib")
    ));
}

#[tokio::test]
async fn add_extensions_for_same_manifest() {
    let cx = TestContext::new(None, GZOnly);
    cx.update_from_dist(&[], &[], false).await.unwrap();

    let adds = vec![
        Component::new(
            "rust-std".to_string(),
            Some(TargetTriple::new("i686-apple-darwin")),
            false,
        ),
        Component::new(
            "rust-std".to_string(),
            Some(TargetTriple::new("i686-unknown-linux-gnu")),
            false,
        ),
    ];

    cx.update_from_dist(&adds, &[], false).await.unwrap();

    assert!(utils::path_exists(
        cx.prefix.path().join("lib/i686-apple-darwin/libstd.rlib")
    ));
    assert!(utils::path_exists(
        cx.prefix
            .path()
            .join("lib/i686-unknown-linux-gnu/libstd.rlib")
    ));
}

#[tokio::test]
async fn add_extensions_for_upgrade() {
    let cx = TestContext::new(None, GZOnly);
    change_channel_date(&cx.url, "nightly", "2016-02-01");

    cx.update_from_dist(&[], &[], false).await.unwrap();

    change_channel_date(&cx.url, "nightly", "2016-02-02");

    let adds = vec![
        Component::new(
            "rust-std".to_string(),
            Some(TargetTriple::new("i686-apple-darwin")),
            false,
        ),
        Component::new(
            "rust-std".to_string(),
            Some(TargetTriple::new("i686-unknown-linux-gnu")),
            false,
        ),
    ];

    cx.update_from_dist(&adds, &[], false).await.unwrap();

    assert!(utils::path_exists(
        cx.prefix.path().join("lib/i686-apple-darwin/libstd.rlib")
    ));
    assert!(utils::path_exists(
        cx.prefix
            .path()
            .join("lib/i686-unknown-linux-gnu/libstd.rlib")
    ));
}

#[tokio::test]
#[should_panic]
async fn add_extension_not_in_manifest() {
    let cx = TestContext::new(None, GZOnly);
    let adds = vec![Component::new(
        "rust-bogus".to_string(),
        Some(TargetTriple::new("i686-apple-darwin")),
        true,
    )];

    cx.update_from_dist(&adds, &[], false).await.unwrap();
}

#[tokio::test]
#[should_panic]
async fn add_extension_that_is_required_component() {
    let cx = TestContext::new(None, GZOnly);
    let adds = vec![Component::new(
        "rustc".to_string(),
        Some(TargetTriple::new("x86_64-apple-darwin")),
        false,
    )];

    cx.update_from_dist(&adds, &[], false).await.unwrap();
}

#[test]
#[ignore]
fn add_extensions_for_same_manifest_does_not_reinstall_other_components() {}

#[test]
#[ignore]
fn add_extensions_for_same_manifest_when_extension_already_installed() {}

#[tokio::test]
async fn add_extensions_does_not_remove_other_components() {
    let cx = TestContext::new(None, GZOnly);
    cx.update_from_dist(&[], &[], false).await.unwrap();

    let adds = vec![Component::new(
        "rust-std".to_string(),
        Some(TargetTriple::new("i686-apple-darwin")),
        false,
    )];

    cx.update_from_dist(&adds, &[], false).await.unwrap();

    assert!(utils::path_exists(cx.prefix.path().join("bin/rustc")));
}

// Asking to remove extensions on initial install is nonsense.
#[tokio::test]
#[should_panic]
async fn remove_extensions_for_initial_install() {
    let cx = TestContext::new(None, GZOnly);
    let removes = vec![Component::new(
        "rustc".to_string(),
        Some(TargetTriple::new("x86_64-apple-darwin")),
        false,
    )];

    cx.update_from_dist(&[], &removes, false).await.unwrap();
}

#[tokio::test]
async fn remove_extensions_for_same_manifest() {
    let cx = TestContext::new(None, GZOnly);
    let adds = vec![
        Component::new(
            "rust-std".to_string(),
            Some(TargetTriple::new("i686-apple-darwin")),
            false,
        ),
        Component::new(
            "rust-std".to_string(),
            Some(TargetTriple::new("i686-unknown-linux-gnu")),
            false,
        ),
    ];

    cx.update_from_dist(&adds, &[], false).await.unwrap();

    let removes = vec![Component::new(
        "rust-std".to_string(),
        Some(TargetTriple::new("i686-apple-darwin")),
        false,
    )];

    cx.update_from_dist(&[], &removes, false).await.unwrap();

    assert!(!utils::path_exists(
        cx.prefix.path().join("lib/i686-apple-darwin/libstd.rlib")
    ));
    assert!(utils::path_exists(
        cx.prefix
            .path()
            .join("lib/i686-unknown-linux-gnu/libstd.rlib")
    ));
}

#[tokio::test]
async fn remove_extensions_for_upgrade() {
    let cx = TestContext::new(None, GZOnly);
    change_channel_date(&cx.url, "nightly", "2016-02-01");

    let adds = vec![
        Component::new(
            "rust-std".to_string(),
            Some(TargetTriple::new("i686-apple-darwin")),
            false,
        ),
        Component::new(
            "rust-std".to_string(),
            Some(TargetTriple::new("i686-unknown-linux-gnu")),
            false,
        ),
    ];

    cx.update_from_dist(&adds, &[], false).await.unwrap();

    change_channel_date(&cx.url, "nightly", "2016-02-02");

    let removes = vec![Component::new(
        "rust-std".to_string(),
        Some(TargetTriple::new("i686-apple-darwin")),
        false,
    )];

    cx.update_from_dist(&[], &removes, false).await.unwrap();

    assert!(!utils::path_exists(
        cx.prefix.path().join("lib/i686-apple-darwin/libstd.rlib")
    ));
    assert!(utils::path_exists(
        cx.prefix
            .path()
            .join("lib/i686-unknown-linux-gnu/libstd.rlib")
    ));
}

#[tokio::test]
#[should_panic]
async fn remove_extension_not_in_manifest() {
    let cx = TestContext::new(None, GZOnly);

    change_channel_date(&cx.url, "nightly", "2016-02-01");

    cx.update_from_dist(&[], &[], false).await.unwrap();

    change_channel_date(&cx.url, "nightly", "2016-02-02");

    let removes = vec![Component::new(
        "rust-bogus".to_string(),
        Some(TargetTriple::new("i686-apple-darwin")),
        true,
    )];

    cx.update_from_dist(&[], &removes, false).await.unwrap();
}

// Extensions that don't exist in the manifest may still exist on disk
// from a previous manifest.
#[tokio::test]
async fn remove_extension_not_in_manifest_but_is_already_installed() {
    let edit = &|date: &str, chan: &mut MockChannel| {
        if date == "2016-02-01" {
            let tpkg = chan.packages[0]
                .targets
                .iter_mut()
                .find(|p| p.target == "x86_64-apple-darwin")
                .unwrap();
            tpkg.components.push(MockComponent {
                name: "bonus".to_string(),
                target: "x86_64-apple-darwin".to_string(),
                is_extension: true,
            });
        } else {
            chan.packages.retain(|p| p.name != "bonus");
        }
    };

    let cx = TestContext::new(Some(edit), GZOnly);

    change_channel_date(&cx.url, "nightly", "2016-02-01");

    let adds = [Component::new(
        "bonus".to_string(),
        Some(TargetTriple::new("x86_64-apple-darwin")),
        true,
    )];
    cx.update_from_dist(&adds, &[], false).await.unwrap();
    assert!(utils::path_exists(cx.prefix.path().join("bin/bonus")));

    change_channel_date(&cx.url, "nightly", "2016-02-02");

    let removes = vec![Component::new(
        "bonus".to_string(),
        Some(TargetTriple::new("x86_64-apple-darwin")),
        true,
    )];
    cx.update_from_dist(&[], &removes, false).await.unwrap();
}

#[tokio::test]
#[should_panic]
async fn remove_extension_that_is_required_component() {
    let cx = TestContext::new(None, GZOnly);
    cx.update_from_dist(&[], &[], false).await.unwrap();

    let removes = vec![Component::new(
        "rustc".to_string(),
        Some(TargetTriple::new("x86_64-apple-darwin")),
        false,
    )];

    cx.update_from_dist(&[], &removes, false).await.unwrap();
}

#[tokio::test]
#[should_panic]
async fn remove_extension_not_installed() {
    let cx = TestContext::new(None, GZOnly);
    cx.update_from_dist(&[], &[], false).await.unwrap();

    let removes = vec![Component::new(
        "rust-std".to_string(),
        Some(TargetTriple::new("i686-apple-darwin")),
        false,
    )];

    cx.update_from_dist(&[], &removes, false).await.unwrap();
}

#[test]
#[ignore]
fn remove_extensions_for_same_manifest_does_not_reinstall_other_components() {}

#[tokio::test]
async fn remove_extensions_does_not_remove_other_components() {
    let cx = TestContext::new(None, GZOnly);
    let adds = vec![Component::new(
        "rust-std".to_string(),
        Some(TargetTriple::new("i686-apple-darwin")),
        false,
    )];

    cx.update_from_dist(&adds, &[], false).await.unwrap();

    let removes = vec![Component::new(
        "rust-std".to_string(),
        Some(TargetTriple::new("i686-apple-darwin")),
        false,
    )];

    cx.update_from_dist(&[], &removes, false).await.unwrap();

    assert!(utils::path_exists(cx.prefix.path().join("bin/rustc")));
}

#[tokio::test]
async fn add_and_remove_for_upgrade() {
    let cx = TestContext::new(None, GZOnly);
    change_channel_date(&cx.url, "nightly", "2016-02-01");

    let adds = vec![Component::new(
        "rust-std".to_string(),
        Some(TargetTriple::new("i686-unknown-linux-gnu")),
        false,
    )];

    cx.update_from_dist(&adds, &[], false).await.unwrap();

    change_channel_date(&cx.url, "nightly", "2016-02-02");

    let adds = vec![Component::new(
        "rust-std".to_string(),
        Some(TargetTriple::new("i686-apple-darwin")),
        false,
    )];

    let removes = vec![Component::new(
        "rust-std".to_string(),
        Some(TargetTriple::new("i686-unknown-linux-gnu")),
        false,
    )];

    cx.update_from_dist(&adds, &removes, false).await.unwrap();

    assert!(utils::path_exists(
        cx.prefix.path().join("lib/i686-apple-darwin/libstd.rlib")
    ));
    assert!(!utils::path_exists(
        cx.prefix
            .path()
            .join("lib/i686-unknown-linux-gnu/libstd.rlib")
    ));
}

#[tokio::test]
async fn add_and_remove() {
    let cx = TestContext::new(None, GZOnly);

    let adds = vec![Component::new(
        "rust-std".to_string(),
        Some(TargetTriple::new("i686-unknown-linux-gnu")),
        false,
    )];

    cx.update_from_dist(&adds, &[], false).await.unwrap();

    let adds = vec![Component::new(
        "rust-std".to_string(),
        Some(TargetTriple::new("i686-apple-darwin")),
        false,
    )];

    let removes = vec![Component::new(
        "rust-std".to_string(),
        Some(TargetTriple::new("i686-unknown-linux-gnu")),
        false,
    )];

    cx.update_from_dist(&adds, &removes, false).await.unwrap();

    assert!(utils::path_exists(
        cx.prefix.path().join("lib/i686-apple-darwin/libstd.rlib")
    ));
    assert!(!utils::path_exists(
        cx.prefix
            .path()
            .join("lib/i686-unknown-linux-gnu/libstd.rlib")
    ));
}

#[tokio::test]
async fn add_and_remove_same_component() {
    let cx = TestContext::new(None, GZOnly);
    cx.update_from_dist(&[], &[], false).await.unwrap();

    let adds = vec![Component::new(
        "rust-std".to_string(),
        Some(TargetTriple::new("i686-apple-darwin")),
        false,
    )];

    let removes = vec![Component::new(
        "rust-std".to_string(),
        Some(TargetTriple::new("i686-apple-darwin")),
        false,
    )];

    cx.update_from_dist(&adds, &removes, false)
        .await
        .expect_err("can't both add and remove components");
}

#[tokio::test]
async fn bad_component_hash() {
    let cx = TestContext::new(None, GZOnly);

    let path = cx.url.to_file_path().unwrap();
    let path = path.join("dist/2016-02-02/rustc-nightly-x86_64-apple-darwin.tar.gz");
    utils_raw::write_file(&path, "bogus").unwrap();

    let err = cx.update_from_dist(&[], &[], false).await.unwrap_err();

    match err.downcast::<RustupError>() {
        Ok(RustupError::ComponentDownloadFailed(..)) => (),
        _ => panic!(),
    }
}

#[tokio::test]
async fn unable_to_download_component() {
    let cx = TestContext::new(None, GZOnly);

    let path = cx.url.to_file_path().unwrap();
    let path = path.join("dist/2016-02-02/rustc-nightly-x86_64-apple-darwin.tar.gz");
    fs::remove_file(path).unwrap();

    let err = cx.update_from_dist(&[], &[], false).await.unwrap_err();

    match err.downcast::<RustupError>() {
        Ok(RustupError::ComponentDownloadFailed(..)) => (),
        _ => panic!(),
    }
}

fn prevent_installation(prefix: &InstallPrefix) {
    utils::ensure_dir_exists(
        "installation path",
        &prefix.path().join("lib"),
        &|_: Notification<'_>| {},
    )
    .unwrap();
    let install_blocker = prefix.path().join("lib").join("rustlib");
    utils::write_file("install-blocker", &install_blocker, "fail-installation").unwrap();
}

fn allow_installation(prefix: &InstallPrefix) {
    let install_blocker = prefix.path().join("lib").join("rustlib");
    utils::remove_file("install-blocker", &install_blocker).unwrap();
}

#[tokio::test]
async fn reuse_downloaded_file() {
    let cx = TestContext::new(None, GZOnly);
    prevent_installation(&cx.prefix);

    let reuse_notification_fired = Arc::new(Cell::new(false));
    let dl_cfg = DownloadCfg {
        notify_handler: &|n| {
            if let Notification::FileAlreadyDownloaded = n {
                reuse_notification_fired.set(true);
            }
        },
        ..cx.default_dl_cfg()
    };

    cx.update_from_dist_with_dl_cfg(&[], &[], false, &dl_cfg)
        .await
        .unwrap_err();
    assert!(!reuse_notification_fired.get());

    allow_installation(&cx.prefix);
    cx.update_from_dist_with_dl_cfg(&[], &[], false, &dl_cfg)
        .await
        .unwrap();

    assert!(reuse_notification_fired.get());
}

#[tokio::test]
async fn checks_files_hashes_before_reuse() {
    let cx = TestContext::new(None, GZOnly);

    let path = cx.url.to_file_path().unwrap();
    let target_hash = utils::read_file(
        "target hash",
        &path.join("dist/2016-02-02/rustc-nightly-x86_64-apple-darwin.tar.gz.sha256"),
    )
    .unwrap()[..64]
        .to_owned();
    let prev_download = cx.download_dir.join(target_hash);
    utils::ensure_dir_exists("download dir", &cx.download_dir, &|_: Notification<'_>| {}).unwrap();
    utils::write_file("bad previous download", &prev_download, "bad content").unwrap();
    println!("wrote previous download to {}", prev_download.display());

    let noticed_bad_checksum = Arc::new(Cell::new(false));
    let dl_cfg = DownloadCfg {
        notify_handler: &|n| {
            if let Notification::CachedFileChecksumFailed = n {
                noticed_bad_checksum.set(true);
            }
        },
        ..cx.default_dl_cfg()
    };

    cx.update_from_dist_with_dl_cfg(&[], &[], false, &dl_cfg)
        .await
        .unwrap();

    assert!(noticed_bad_checksum.get());
}

#[tokio::test]
async fn handle_corrupt_partial_downloads() {
    let cx = TestContext::new(None, GZOnly);

    // write a corrupt partial out
    let path = cx.url.to_file_path().unwrap();
    let target_hash = utils::read_file(
        "target hash",
        &path.join("dist/2016-02-02/rustc-nightly-x86_64-apple-darwin.tar.gz.sha256"),
    )
    .unwrap()[..SHA256_HASH_LEN]
        .to_owned();

    utils::ensure_dir_exists("download dir", &cx.download_dir, &|_: Notification<'_>| {}).unwrap();
    let partial_path = cx.download_dir.join(format!("{target_hash}.partial"));
    utils_raw::write_file(
        &partial_path,
        "file will be resumed from here and not match hash",
    )
    .unwrap();

    cx.update_from_dist(&[], &[], false).await.unwrap();

    assert!(utils::path_exists(cx.prefix.path().join("bin/rustc")));
    assert!(utils::path_exists(cx.prefix.path().join("lib/libstd.rlib")));
}
