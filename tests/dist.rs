// Tests of installation and updates from a v2 Rust distribution
// server (mocked on the file system)

pub mod mock;

use crate::mock::dist::*;
use crate::mock::{MockComponentBuilder, MockFile, MockInstallerBuilder};
use rustup::dist::dist::{Profile, TargetTriple, ToolchainDesc, DEFAULT_DIST_SERVER};
use rustup::dist::download::DownloadCfg;
use rustup::dist::manifest::{Component, Manifest};
use rustup::dist::manifestation::{Changes, Manifestation, UpdateStatus};
use rustup::dist::prefix::InstallPrefix;
use rustup::dist::temp;
use rustup::dist::Notification;
use rustup::errors::Result;
use rustup::utils::raw as utils_raw;
use rustup::utils::utils;
use rustup::ErrorKind;
use std::cell::Cell;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use url::Url;

// Creates a mock dist server populated with some test data
pub fn create_mock_dist_server(
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

pub fn create_mock_channel(
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
    packages.push(bonus_component("bonus", contents.clone()));

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
                    name: format!("{}-x86_64-apple-darwin", name),
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

    create_mock_dist_server(&path, None).write(&[ManifestVersion::V2], false);

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
#[test]
fn rename_component() {
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

    setup_from_dist_server(
        mock_dist_server,
        &url,
        false,
        &|url, toolchain, prefix, download_cfg, temp_cfg| {
            let adds = [Component::new(
                "bonus".to_string(),
                Some(TargetTriple::new("x86_64-apple-darwin")),
                true,
            )];

            change_channel_date(url, "nightly", "2016-02-01");
            update_from_dist(
                url,
                toolchain,
                prefix,
                &adds,
                &[],
                download_cfg,
                temp_cfg,
                false,
            )
            .unwrap();
            assert!(utils::path_exists(&prefix.path().join("bin/bonus")));
            assert!(!utils::path_exists(&prefix.path().join("bin/bobo")));
            change_channel_date(url, "nightly", "2016-02-02");
            update_from_dist(
                url,
                toolchain,
                prefix,
                &[],
                &[],
                download_cfg,
                temp_cfg,
                false,
            )
            .unwrap();
            assert!(utils::path_exists(&prefix.path().join("bin/bonus")));
            assert!(!utils::path_exists(&prefix.path().join("bin/bobo")));
        },
    );
}

// Test that a rename is ignored if the component with the old name was never installed.
#[test]
fn rename_component_new() {
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

    setup_from_dist_server(
        mock_dist_server,
        &url,
        false,
        &|url, toolchain, prefix, download_cfg, temp_cfg| {
            let adds = [Component::new(
                "bobo".to_string(),
                Some(TargetTriple::new("x86_64-apple-darwin")),
                true,
            )];
            // Install the basics from day 1
            change_channel_date(url, "nightly", "2016-02-01");
            update_from_dist(
                url,
                toolchain,
                prefix,
                &[],
                &[],
                download_cfg,
                temp_cfg,
                false,
            )
            .unwrap();
            // Neither bonus nor bobo are installed at this point.
            assert!(!utils::path_exists(&prefix.path().join("bin/bonus")));
            assert!(!utils::path_exists(&prefix.path().join("bin/bobo")));
            // Now we move to day 2, where bobo is part of the set of things we want
            // to have installed
            change_channel_date(url, "nightly", "2016-02-02");
            update_from_dist(
                url,
                toolchain,
                prefix,
                &adds,
                &[],
                download_cfg,
                temp_cfg,
                false,
            )
            .unwrap();
            // As a result `bin/bonus` is present but not `bin/bobo` which we'd
            // expect since the bonus component installs `bin/bonus` regardless of
            // its name being `bobo`
            assert!(!utils::path_exists(&prefix.path().join("bin/bobo")));
            assert!(utils::path_exists(&prefix.path().join("bin/bonus")));
        },
    );
}

// Installs or updates a toolchain from a dist server.  If an initial
// install then it will be installed with the default components.  If
// an upgrade then all the existing components will be upgraded.
// FIXME: Unify this with dist::update_from_dist
fn update_from_dist(
    dist_server: &Url,
    toolchain: &ToolchainDesc,
    prefix: &InstallPrefix,
    add: &[Component],
    remove: &[Component],
    download_cfg: &DownloadCfg<'_>,
    temp_cfg: &temp::Cfg,
    force: bool,
) -> Result<UpdateStatus> {
    // Download the dist manifest and place it into the installation prefix
    let manifest_url = make_manifest_url(dist_server, toolchain)?;
    let manifest_file = temp_cfg.new_file()?;
    utils::download_file(&manifest_url, &manifest_file, None, &|_| {})?;
    let manifest_str = utils::read_file("manifest", &manifest_file)?;
    let manifest = Manifest::parse(&manifest_str)?;

    // Read the manifest to update the components
    let trip = toolchain.target.clone();
    let manifestation = Manifestation::open(prefix.clone(), trip.clone())?;

    // TODO on install, need to add profile components (but I guess we shouldn't test that logic here)
    let mut profile_components = manifest.get_profile_components(Profile::Default, &trip)?;
    let mut add_components = add.to_owned();
    add_components.append(&mut profile_components);

    let changes = Changes {
        explicit_add_components: add_components,
        remove_components: remove.to_owned(),
    };

    manifestation.update(
        &manifest,
        changes,
        force,
        download_cfg,
        download_cfg.notify_handler,
        &toolchain.manifest_name(),
    )
}

fn make_manifest_url(dist_server: &Url, toolchain: &ToolchainDesc) -> Result<Url> {
    let url = format!(
        "{}/dist/channel-rust-{}.toml",
        dist_server, toolchain.channel
    );

    Ok(Url::parse(&url).unwrap())
}

fn uninstall(
    toolchain: &ToolchainDesc,
    prefix: &InstallPrefix,
    temp_cfg: &temp::Cfg,
    notify_handler: &dyn Fn(Notification<'_>),
) -> Result<()> {
    let trip = toolchain.target.clone();
    let manifestation = Manifestation::open(prefix.clone(), trip)?;
    let manifest = manifestation.load_manifest()?.unwrap();

    manifestation.uninstall(&manifest, temp_cfg, notify_handler)?;

    Ok(())
}

fn setup(
    edit: Option<&dyn Fn(&str, &mut MockChannel)>,
    enable_xz: bool,
    f: &dyn Fn(&Url, &ToolchainDesc, &InstallPrefix, &DownloadCfg<'_>, &temp::Cfg),
) {
    let dist_tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let mock_dist_server = create_mock_dist_server(dist_tempdir.path(), edit);
    let url = Url::parse(&format!("file://{}", dist_tempdir.path().to_string_lossy())).unwrap();
    setup_from_dist_server(mock_dist_server, &url, enable_xz, f);
}

fn setup_from_dist_server(
    server: MockDistServer,
    url: &Url,
    enable_xz: bool,
    f: &dyn Fn(&Url, &ToolchainDesc, &InstallPrefix, &DownloadCfg<'_>, &temp::Cfg),
) {
    server.write(&[ManifestVersion::V2], enable_xz);

    let prefix_tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();

    let work_tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let temp_cfg = temp::Cfg::new(
        work_tempdir.path().to_owned(),
        DEFAULT_DIST_SERVER,
        Box::new(|_| ()),
    );

    let toolchain = ToolchainDesc::from_str("nightly-x86_64-apple-darwin").unwrap();
    let prefix = InstallPrefix::from(prefix_tempdir.path().to_owned());
    let download_cfg = DownloadCfg {
        dist_root: "phony",
        temp_cfg: &temp_cfg,
        download_dir: &prefix.path().to_owned().join("downloads"),
        notify_handler: &|_| {},
    };

    f(url, &toolchain, &prefix, &download_cfg, &temp_cfg);
}

#[test]
fn initial_install() {
    setup(None, false, &|url,
                         toolchain,
                         prefix,
                         download_cfg,
                         temp_cfg| {
        update_from_dist(
            url,
            toolchain,
            prefix,
            &[],
            &[],
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();

        assert!(utils::path_exists(&prefix.path().join("bin/rustc")));
        assert!(utils::path_exists(&prefix.path().join("lib/libstd.rlib")));
    });
}

#[test]
fn initial_install_xz() {
    setup(None, true, &|url,
                        toolchain,
                        prefix,
                        download_cfg,
                        temp_cfg| {
        update_from_dist(
            url,
            toolchain,
            prefix,
            &[],
            &[],
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();

        assert!(utils::path_exists(&prefix.path().join("bin/rustc")));
        assert!(utils::path_exists(&prefix.path().join("lib/libstd.rlib")));
    });
}

#[test]
fn test_uninstall() {
    setup(None, false, &|url,
                         toolchain,
                         prefix,
                         download_cfg,
                         temp_cfg| {
        update_from_dist(
            url,
            toolchain,
            prefix,
            &[],
            &[],
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();
        uninstall(toolchain, prefix, temp_cfg, &|_| ()).unwrap();

        assert!(!utils::path_exists(&prefix.path().join("bin/rustc")));
        assert!(!utils::path_exists(&prefix.path().join("lib/libstd.rlib")));
    });
}

#[test]
fn uninstall_removes_config_file() {
    setup(None, false, &|url,
                         toolchain,
                         prefix,
                         download_cfg,
                         temp_cfg| {
        update_from_dist(
            url,
            toolchain,
            prefix,
            &[],
            &[],
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();
        assert!(utils::path_exists(
            &prefix.manifest_file("multirust-config.toml")
        ));
        uninstall(toolchain, prefix, temp_cfg, &|_| ()).unwrap();
        assert!(!utils::path_exists(
            &prefix.manifest_file("multirust-config.toml")
        ));
    });
}

#[test]
fn upgrade() {
    setup(None, false, &|url,
                         toolchain,
                         prefix,
                         download_cfg,
                         temp_cfg| {
        change_channel_date(url, "nightly", "2016-02-01");
        update_from_dist(
            url,
            toolchain,
            prefix,
            &[],
            &[],
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();
        assert_eq!(
            "2016-02-01",
            fs::read_to_string(&prefix.path().join("bin/rustc")).unwrap()
        );
        change_channel_date(url, "nightly", "2016-02-02");
        update_from_dist(
            url,
            toolchain,
            prefix,
            &[],
            &[],
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();
        assert_eq!(
            "2016-02-02",
            fs::read_to_string(&prefix.path().join("bin/rustc")).unwrap()
        );
    });
}

#[test]
fn unavailable_component() {
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

    setup(
        Some(edit),
        false,
        &|url, toolchain, prefix, download_cfg, temp_cfg| {
            let adds = [Component::new(
                "bonus".to_string(),
                Some(TargetTriple::new("x86_64-apple-darwin")),
                true,
            )];

            change_channel_date(url, "nightly", "2016-02-01");
            // Update with bonus.
            update_from_dist(
                url,
                toolchain,
                prefix,
                &adds,
                &[],
                download_cfg,
                temp_cfg,
                false,
            )
            .unwrap();
            assert!(utils::path_exists(&prefix.path().join("bin/bonus")));
            change_channel_date(url, "nightly", "2016-02-02");

            // Update without bonus, should fail.
            let err = update_from_dist(
                url,
                toolchain,
                prefix,
                &[],
                &[],
                download_cfg,
                temp_cfg,
                false,
            )
            .unwrap_err();
            match *err.kind() {
                ErrorKind::RequestedComponentsUnavailable(..) => {}
                _ => panic!(),
            }
        },
    );
}

// As unavailable_component, but the unavailable component is part of the profile.
#[test]
fn unavailable_component_from_profile() {
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

    setup(
        Some(edit),
        false,
        &|url, toolchain, prefix, download_cfg, temp_cfg| {
            change_channel_date(url, "nightly", "2016-02-01");
            // Update with rustc.
            update_from_dist(
                url,
                toolchain,
                prefix,
                &[],
                &[],
                download_cfg,
                temp_cfg,
                false,
            )
            .unwrap();
            assert!(utils::path_exists(&prefix.path().join("bin/rustc")));
            change_channel_date(url, "nightly", "2016-02-02");

            // Update without rustc, should fail.
            let err = update_from_dist(
                url,
                toolchain,
                prefix,
                &[],
                &[],
                download_cfg,
                temp_cfg,
                false,
            )
            .unwrap_err();
            match *err.kind() {
                ErrorKind::RequestedComponentsUnavailable(..) => {}
                _ => panic!(),
            }

            update_from_dist(
                url,
                toolchain,
                prefix,
                &[],
                &[],
                download_cfg,
                temp_cfg,
                true,
            )
            .unwrap();
        },
    );
}

#[test]
fn removed_component() {
    // On day 1 install the 'bonus' component, on day 2 its no longer a component
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

    setup(
        Some(edit),
        false,
        &|url, toolchain, prefix, download_cfg, temp_cfg| {
            let adds = [Component::new(
                "bonus".to_string(),
                Some(TargetTriple::new("x86_64-apple-darwin")),
                true,
            )];

            // Update with bonus.
            change_channel_date(url, "nightly", "2016-02-01");
            update_from_dist(
                url,
                toolchain,
                prefix,
                &adds,
                &[],
                &download_cfg,
                temp_cfg,
                false,
            )
            .unwrap();
            assert!(utils::path_exists(&prefix.path().join("bin/bonus")));

            // Update without bonus, should fail with RequestedComponentsUnavailable
            change_channel_date(url, "nightly", "2016-02-02");
            assert!(update_from_dist(
                url,
                toolchain,
                prefix,
                &[],
                &[],
                &download_cfg,
                temp_cfg,
                false
            )
            .is_err());
        },
    );
}

#[test]
fn update_preserves_extensions() {
    setup(None, false, &|url,
                         toolchain,
                         prefix,
                         download_cfg,
                         temp_cfg| {
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

        change_channel_date(url, "nightly", "2016-02-01");
        update_from_dist(
            url,
            toolchain,
            prefix,
            &adds,
            &[],
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();

        assert!(utils::path_exists(
            &prefix.path().join("lib/i686-apple-darwin/libstd.rlib")
        ));
        assert!(utils::path_exists(
            &prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")
        ));

        change_channel_date(url, "nightly", "2016-02-02");
        update_from_dist(
            url,
            toolchain,
            prefix,
            &[],
            &[],
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();

        assert!(utils::path_exists(
            &prefix.path().join("lib/i686-apple-darwin/libstd.rlib")
        ));
        assert!(utils::path_exists(
            &prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")
        ));
    });
}

#[test]
fn update_makes_no_changes_for_identical_manifest() {
    setup(None, false, &|url,
                         toolchain,
                         prefix,
                         download_cfg,
                         temp_cfg| {
        let status = update_from_dist(
            url,
            toolchain,
            prefix,
            &[],
            &[],
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();
        assert_eq!(status, UpdateStatus::Changed);
        let status = update_from_dist(
            url,
            toolchain,
            prefix,
            &[],
            &[],
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();
        assert_eq!(status, UpdateStatus::Unchanged);
    });
}

#[test]
fn add_extensions_for_initial_install() {
    setup(None, false, &|url,
                         toolchain,
                         prefix,
                         download_cfg,
                         temp_cfg| {
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

        update_from_dist(
            url,
            toolchain,
            prefix,
            &adds,
            &[],
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();
        assert!(utils::path_exists(
            &prefix.path().join("lib/i686-apple-darwin/libstd.rlib")
        ));
        assert!(utils::path_exists(
            &prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")
        ));
    });
}

#[test]
fn add_extensions_for_same_manifest() {
    setup(None, false, &|url,
                         toolchain,
                         prefix,
                         download_cfg,
                         temp_cfg| {
        update_from_dist(
            url,
            toolchain,
            prefix,
            &[],
            &[],
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();

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

        update_from_dist(
            url,
            toolchain,
            prefix,
            &adds,
            &[],
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();

        assert!(utils::path_exists(
            &prefix.path().join("lib/i686-apple-darwin/libstd.rlib")
        ));
        assert!(utils::path_exists(
            &prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")
        ));
    });
}

#[test]
fn add_extensions_for_upgrade() {
    setup(None, false, &|url,
                         toolchain,
                         prefix,
                         download_cfg,
                         temp_cfg| {
        change_channel_date(url, "nightly", "2016-02-01");

        update_from_dist(
            url,
            toolchain,
            prefix,
            &[],
            &[],
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();

        change_channel_date(url, "nightly", "2016-02-02");

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

        update_from_dist(
            url,
            toolchain,
            prefix,
            &adds,
            &[],
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();

        assert!(utils::path_exists(
            &prefix.path().join("lib/i686-apple-darwin/libstd.rlib")
        ));
        assert!(utils::path_exists(
            &prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")
        ));
    });
}

#[test]
#[should_panic]
fn add_extension_not_in_manifest() {
    setup(None, false, &|url,
                         toolchain,
                         prefix,
                         download_cfg,
                         temp_cfg| {
        let adds = vec![Component::new(
            "rust-bogus".to_string(),
            Some(TargetTriple::new("i686-apple-darwin")),
            true,
        )];

        update_from_dist(
            url,
            toolchain,
            prefix,
            &adds,
            &[],
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();
    });
}

#[test]
#[should_panic]
fn add_extension_that_is_required_component() {
    setup(None, false, &|url,
                         toolchain,
                         prefix,
                         download_cfg,
                         temp_cfg| {
        let adds = vec![Component::new(
            "rustc".to_string(),
            Some(TargetTriple::new("x86_64-apple-darwin")),
            false,
        )];

        update_from_dist(
            url,
            toolchain,
            prefix,
            &adds,
            &[],
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();
    });
}

#[test]
#[ignore]
fn add_extensions_for_same_manifest_does_not_reinstall_other_components() {}

#[test]
#[ignore]
fn add_extensions_for_same_manifest_when_extension_already_installed() {}

#[test]
fn add_extensions_does_not_remove_other_components() {
    setup(None, false, &|url,
                         toolchain,
                         prefix,
                         download_cfg,
                         temp_cfg| {
        update_from_dist(
            url,
            toolchain,
            prefix,
            &[],
            &[],
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();

        let adds = vec![Component::new(
            "rust-std".to_string(),
            Some(TargetTriple::new("i686-apple-darwin")),
            false,
        )];

        update_from_dist(
            url,
            toolchain,
            prefix,
            &adds,
            &[],
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();

        assert!(utils::path_exists(&prefix.path().join("bin/rustc")));
    });
}

// Asking to remove extensions on initial install is nonsese.
#[test]
#[should_panic]
fn remove_extensions_for_initial_install() {
    setup(None, false, &|url,
                         toolchain,
                         prefix,
                         download_cfg,
                         temp_cfg| {
        let removes = vec![Component::new(
            "rustc".to_string(),
            Some(TargetTriple::new("x86_64-apple-darwin")),
            false,
        )];

        update_from_dist(
            url,
            toolchain,
            prefix,
            &[],
            &removes,
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();
    });
}

#[test]
fn remove_extensions_for_same_manifest() {
    setup(None, false, &|url,
                         toolchain,
                         prefix,
                         download_cfg,
                         temp_cfg| {
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

        update_from_dist(
            url,
            toolchain,
            prefix,
            &adds,
            &[],
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();

        let removes = vec![Component::new(
            "rust-std".to_string(),
            Some(TargetTriple::new("i686-apple-darwin")),
            false,
        )];

        update_from_dist(
            url,
            toolchain,
            prefix,
            &[],
            &removes,
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();

        assert!(!utils::path_exists(
            &prefix.path().join("lib/i686-apple-darwin/libstd.rlib")
        ));
        assert!(utils::path_exists(
            &prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")
        ));
    });
}

#[test]
fn remove_extensions_for_upgrade() {
    setup(None, false, &|url,
                         toolchain,
                         prefix,
                         download_cfg,
                         temp_cfg| {
        change_channel_date(url, "nightly", "2016-02-01");

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

        update_from_dist(
            url,
            toolchain,
            prefix,
            &adds,
            &[],
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();

        change_channel_date(url, "nightly", "2016-02-02");

        let removes = vec![Component::new(
            "rust-std".to_string(),
            Some(TargetTriple::new("i686-apple-darwin")),
            false,
        )];

        update_from_dist(
            url,
            toolchain,
            prefix,
            &[],
            &removes,
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();

        assert!(!utils::path_exists(
            &prefix.path().join("lib/i686-apple-darwin/libstd.rlib")
        ));
        assert!(utils::path_exists(
            &prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")
        ));
    });
}

#[test]
#[should_panic]
fn remove_extension_not_in_manifest() {
    setup(None, false, &|url,
                         toolchain,
                         prefix,
                         download_cfg,
                         temp_cfg| {
        change_channel_date(url, "nightly", "2016-02-01");

        update_from_dist(
            url,
            toolchain,
            prefix,
            &[],
            &[],
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();

        change_channel_date(url, "nightly", "2016-02-02");

        let removes = vec![Component::new(
            "rust-bogus".to_string(),
            Some(TargetTriple::new("i686-apple-darwin")),
            true,
        )];

        update_from_dist(
            url,
            toolchain,
            prefix,
            &[],
            &removes,
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();
    });
}

// Extensions that don't exist in the manifest may still exist on disk
// from a previous manifest.
#[test]
fn remove_extension_not_in_manifest_but_is_already_installed() {
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
    setup(
        Some(edit),
        false,
        &|url, toolchain, prefix, download_cfg, temp_cfg| {
            change_channel_date(url, "nightly", "2016-02-01");

            let adds = [Component::new(
                "bonus".to_string(),
                Some(TargetTriple::new("x86_64-apple-darwin")),
                true,
            )];
            update_from_dist(
                url,
                toolchain,
                prefix,
                &adds,
                &[],
                download_cfg,
                temp_cfg,
                false,
            )
            .unwrap();
            assert!(utils::path_exists(&prefix.path().join("bin/bonus")));

            change_channel_date(url, "nightly", "2016-02-02");

            let removes = vec![Component::new(
                "bonus".to_string(),
                Some(TargetTriple::new("x86_64-apple-darwin")),
                true,
            )];
            update_from_dist(
                url,
                toolchain,
                prefix,
                &[],
                &removes,
                download_cfg,
                temp_cfg,
                false,
            )
            .unwrap();
        },
    );
}

#[test]
#[should_panic]
fn remove_extension_that_is_required_component() {
    setup(None, false, &|url,
                         toolchain,
                         prefix,
                         download_cfg,
                         temp_cfg| {
        update_from_dist(
            url,
            toolchain,
            prefix,
            &[],
            &[],
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();

        let removes = vec![Component::new(
            "rustc".to_string(),
            Some(TargetTriple::new("x86_64-apple-darwin")),
            false,
        )];

        update_from_dist(
            url,
            toolchain,
            prefix,
            &[],
            &removes,
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();
    });
}

#[test]
#[should_panic]
fn remove_extension_not_installed() {
    setup(None, false, &|url,
                         toolchain,
                         prefix,
                         download_cfg,
                         temp_cfg| {
        update_from_dist(
            url,
            toolchain,
            prefix,
            &[],
            &[],
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();

        let removes = vec![Component::new(
            "rust-std".to_string(),
            Some(TargetTriple::new("i686-apple-darwin")),
            false,
        )];

        update_from_dist(
            url,
            toolchain,
            prefix,
            &[],
            &removes,
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();
    });
}

#[test]
#[ignore]
fn remove_extensions_for_same_manifest_does_not_reinstall_other_components() {}

#[test]
fn remove_extensions_does_not_remove_other_components() {
    setup(None, false, &|url,
                         toolchain,
                         prefix,
                         download_cfg,
                         temp_cfg| {
        let adds = vec![Component::new(
            "rust-std".to_string(),
            Some(TargetTriple::new("i686-apple-darwin")),
            false,
        )];

        update_from_dist(
            url,
            toolchain,
            prefix,
            &adds,
            &[],
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();

        let removes = vec![Component::new(
            "rust-std".to_string(),
            Some(TargetTriple::new("i686-apple-darwin")),
            false,
        )];

        update_from_dist(
            url,
            toolchain,
            prefix,
            &[],
            &removes,
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();

        assert!(utils::path_exists(&prefix.path().join("bin/rustc")));
    });
}

#[test]
fn add_and_remove_for_upgrade() {
    setup(None, false, &|url,
                         toolchain,
                         prefix,
                         download_cfg,
                         temp_cfg| {
        change_channel_date(url, "nightly", "2016-02-01");

        let adds = vec![Component::new(
            "rust-std".to_string(),
            Some(TargetTriple::new("i686-unknown-linux-gnu")),
            false,
        )];

        update_from_dist(
            url,
            toolchain,
            prefix,
            &adds,
            &[],
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();

        change_channel_date(url, "nightly", "2016-02-02");

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

        update_from_dist(
            url,
            toolchain,
            prefix,
            &adds,
            &removes,
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();

        assert!(utils::path_exists(
            &prefix.path().join("lib/i686-apple-darwin/libstd.rlib")
        ));
        assert!(!utils::path_exists(
            &prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")
        ));
    });
}

#[test]
fn add_and_remove() {
    setup(None, false, &|url,
                         toolchain,
                         prefix,
                         download_cfg,
                         temp_cfg| {
        let adds = vec![Component::new(
            "rust-std".to_string(),
            Some(TargetTriple::new("i686-unknown-linux-gnu")),
            false,
        )];

        update_from_dist(
            url,
            toolchain,
            prefix,
            &adds,
            &[],
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();

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

        update_from_dist(
            url,
            toolchain,
            prefix,
            &adds,
            &removes,
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();

        assert!(utils::path_exists(
            &prefix.path().join("lib/i686-apple-darwin/libstd.rlib")
        ));
        assert!(!utils::path_exists(
            &prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")
        ));
    });
}

#[test]
#[should_panic]
fn add_and_remove_same_component() {
    setup(None, false, &|url,
                         toolchain,
                         prefix,
                         download_cfg,
                         temp_cfg| {
        update_from_dist(
            url,
            toolchain,
            prefix,
            &[],
            &[],
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();

        let adds = vec![Component::new(
            "rust-std".to_string(),
            Some(TargetTriple::new("i686-apple-darwin")),
            false,
        )];

        let removes = vec![Component::new(
            "rust-std".to_string(),
            Some(TargetTriple::new("i686-apple_darwin")),
            false,
        )];

        update_from_dist(
            url,
            toolchain,
            prefix,
            &adds,
            &removes,
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();
    });
}

#[test]
fn bad_component_hash() {
    setup(None, false, &|url,
                         toolchain,
                         prefix,
                         download_cfg,
                         temp_cfg| {
        let path = url.to_file_path().unwrap();
        let path = path.join("dist/2016-02-02/rustc-nightly-x86_64-apple-darwin.tar.gz");
        utils_raw::write_file(&path, "bogus").unwrap();

        let err = update_from_dist(
            url,
            toolchain,
            prefix,
            &[],
            &[],
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap_err();

        match *err.kind() {
            ErrorKind::ComponentDownloadFailed(_) => (),
            _ => panic!(),
        }
    });
}

#[test]
fn unable_to_download_component() {
    setup(None, false, &|url,
                         toolchain,
                         prefix,
                         download_cfg,
                         temp_cfg| {
        let path = url.to_file_path().unwrap();
        let path = path.join("dist/2016-02-02/rustc-nightly-x86_64-apple-darwin.tar.gz");
        fs::remove_file(&path).unwrap();

        let err = update_from_dist(
            url,
            toolchain,
            prefix,
            &[],
            &[],
            download_cfg,
            temp_cfg,
            false,
        )
        .unwrap_err();

        match *err.kind() {
            ErrorKind::ComponentDownloadFailed(..) => (),
            _ => panic!(),
        }
    });
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

#[test]
fn reuse_downloaded_file() {
    setup(None, false, &|url,
                         toolchain,
                         prefix,
                         download_cfg,
                         temp_cfg| {
        prevent_installation(prefix);

        let reuse_notification_fired = Arc::new(Cell::new(false));

        let download_cfg = DownloadCfg {
            dist_root: download_cfg.dist_root,
            temp_cfg: download_cfg.temp_cfg,
            download_dir: download_cfg.download_dir,
            notify_handler: &|n| {
                if let Notification::FileAlreadyDownloaded = n {
                    reuse_notification_fired.set(true);
                }
            },
        };

        update_from_dist(
            url,
            toolchain,
            prefix,
            &[],
            &[],
            &download_cfg,
            temp_cfg,
            false,
        )
        .unwrap_err();
        assert!(!reuse_notification_fired.get());

        allow_installation(&prefix);

        update_from_dist(
            url,
            toolchain,
            prefix,
            &[],
            &[],
            &download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();

        assert!(reuse_notification_fired.get());
    })
}

#[test]
fn checks_files_hashes_before_reuse() {
    setup(None, false, &|url,
                         toolchain,
                         prefix,
                         download_cfg,
                         temp_cfg| {
        let path = url.to_file_path().unwrap();
        let target_hash = utils::read_file(
            "target hash",
            &path.join("dist/2016-02-02/rustc-nightly-x86_64-apple-darwin.tar.gz.sha256"),
        )
        .unwrap()[..64]
            .to_owned();
        let prev_download = download_cfg.download_dir.join(target_hash);
        utils::ensure_dir_exists(
            "download dir",
            &download_cfg.download_dir,
            &|_: Notification<'_>| {},
        )
        .unwrap();
        utils::write_file("bad previous download", &prev_download, "bad content").unwrap();
        println!("wrote previous download to {}", prev_download.display());

        let noticed_bad_checksum = Arc::new(Cell::new(false));
        let download_cfg = DownloadCfg {
            dist_root: download_cfg.dist_root,
            temp_cfg: download_cfg.temp_cfg,
            download_dir: download_cfg.download_dir,
            notify_handler: &|n| {
                if let Notification::CachedFileChecksumFailed = n {
                    noticed_bad_checksum.set(true);
                }
            },
        };

        update_from_dist(
            url,
            toolchain,
            prefix,
            &[],
            &[],
            &download_cfg,
            temp_cfg,
            false,
        )
        .unwrap();

        assert!(noticed_bad_checksum.get());
    })
}
