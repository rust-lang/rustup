// Tests of installation and updates from a v2 Rust distribution
// server (mocked on the file system)

extern crate rustup_dist;
extern crate rustup_utils;
extern crate rustup_mock;
extern crate tempdir;
extern crate tar;
extern crate toml;
extern crate flate2;
extern crate walkdir;
extern crate itertools;
extern crate url;

use rustup_mock::dist::*;
use rustup_mock::{MockCommand, MockInstallerBuilder};
use rustup_dist::prefix::InstallPrefix;
use rustup_dist::ErrorKind;
use rustup_dist::errors::Result;
use rustup_dist::dist::{ToolchainDesc, TargetTriple, DEFAULT_DIST_SERVER};
use rustup_dist::download::DownloadCfg;
use rustup_dist::Notification;
use rustup_utils::utils;
use rustup_utils::raw as utils_raw;
use rustup_dist::temp;
use rustup_dist::manifestation::{Manifestation, UpdateStatus, Changes};
use rustup_dist::manifest::{Manifest, Component};
use url::Url;
use std::fs;
use std::io::Write;
use std::path::Path;
use tempdir::TempDir;
use itertools::Itertools;

// Creates a mock dist server populated with some test data
pub fn create_mock_dist_server(path: &Path,
                               edit: Option<&Fn(&str, &mut MockPackage)>) -> MockDistServer {
    MockDistServer {
        path: path.to_owned(),
        channels: vec![
            create_mock_channel("nightly", "2016-02-01", edit),
            create_mock_channel("nightly", "2016-02-02", edit),
            ]
    }
}

pub fn create_mock_channel(channel: &str, date: &str,
                           edit: Option<&Fn(&str, &mut MockPackage)>) -> MockChannel {
    // Put the date in the files so they can be differentiated
    let contents = date.to_string().into_bytes();

    let rust_pkg = MockPackage {
        name: "rust",
        version: "1.0.0",
        targets: vec![
            MockTargetedPackage {
                target: "x86_64-apple-darwin".to_string(),
                available: true,
                components: vec![
                    MockComponent {
                        name: "rustc".to_string(),
                        target: "x86_64-apple-darwin".to_string(),
                    },
                    MockComponent {
                        name: "rust-std".to_string(),
                        target: "x86_64-apple-darwin".to_string(),
                    },
                    ],
                extensions: vec![
                    MockComponent {
                        name: "rust-std".to_string(),
                        target: "i686-apple-darwin".to_string(),
                    },
                    MockComponent {
                        name: "rust-std".to_string(),
                        target: "i686-unknown-linux-gnu".to_string(),
                    },
                    ],
                installer: MockInstallerBuilder {
                    components: vec![]
                }
            },
            MockTargetedPackage {
                target: "i686-apple-darwin".to_string(),
                available: true,
                components: vec![
                    MockComponent {
                        name: "rustc".to_string(),
                        target: "i686-apple-darwin".to_string(),
                    },
                    MockComponent {
                        name: "rust-std".to_string(),
                        target: "i686-apple-darwin".to_string(),
                    },
                    ],
                extensions: vec![],
                installer: MockInstallerBuilder {
                    components: vec![]
                }
            }
            ]
    };

    let rustc_pkg = MockPackage {
        name: "rustc",
        version: "1.0.0",
        targets: vec![
            MockTargetedPackage {
                target: "x86_64-apple-darwin".to_string(),
                available: true,
                components: vec![],
                extensions: vec![],
                installer: MockInstallerBuilder {
                    components: vec![
                        ("rustc".to_string(),
                         vec![
                             MockCommand::File("bin/rustc".to_string()),
                             ],
                         vec![
                             ("bin/rustc".to_string(), contents.clone())
                                 ],
                         ),
                        ]
                }
            },
            MockTargetedPackage {
                target: "i686-apple-darwin".to_string(),
                available: true,
                components: vec![],
                extensions: vec![],
                installer: MockInstallerBuilder {
                    components: vec![]
                }
            }
            ]
    };

    let std_pkg = MockPackage {
        name: "rust-std",
        version: "1.0.0",
        targets: vec![
            MockTargetedPackage {
                target: "x86_64-apple-darwin".to_string(),
                available: true,
                components: vec![],
                extensions: vec![],
                installer: MockInstallerBuilder {
                    components: vec![
                        ("rust-std-x86_64-apple-darwin".to_string(),
                         vec![
                             MockCommand::File("lib/libstd.rlib".to_string()),
                             ],
                         vec![
                             ("lib/libstd.rlib".to_string(), contents.clone())
                                 ],
                         ),
                        ]
                }
            },
            MockTargetedPackage {
                target: "i686-apple-darwin".to_string(),
                available: true,
                components: vec![],
                extensions: vec![],
                installer: MockInstallerBuilder {
                    components: vec![
                        ("rust-std-i686-apple-darwin".to_string(),
                         vec![
                             MockCommand::File("lib/i686-apple-darwin/libstd.rlib".to_string()),
                             ],
                         vec![
                             ("lib/i686-apple-darwin/libstd.rlib".to_string(), contents.clone())
                                 ],
                         ),
                        ]
                }
            },
            MockTargetedPackage {
                target: "i686-unknown-linux-gnu".to_string(),
                available: true,
                components: vec![],
                extensions: vec![],
                installer: MockInstallerBuilder {
                    components: vec![
                        ("rust-std-i686-unknown-linux-gnu".to_string(),
                         vec![
                             MockCommand::File("lib/i686-unknown-linux-gnu/libstd.rlib".to_string()),
                             ],
                         vec![
                             ("lib/i686-unknown-linux-gnu/libstd.rlib".to_string(), contents.clone())
                                 ],
                         ),
                        ]
                }
            },
            ]
    };

    // An extra package that can be used as a component of the other packages
    // for various tests
    let bonus_pkg = MockPackage {
        name: "bonus",
        version: "1.0.0",
        targets: vec![
            MockTargetedPackage {
                target: "x86_64-apple-darwin".to_string(),
                available: true,
                components: vec![],
                extensions: vec![],
                installer: MockInstallerBuilder {
                    components: vec![
                        ("bonus-x86_64-apple-darwin".to_string(),
                         vec![
                             MockCommand::File("bin/bonus".to_string()),
                             ],
                         vec![
                             ("bin/bonus".to_string(), contents.clone())
                                 ],
                         ),
                        ]
                }
            },
            ]
    };

    let mut rust_pkg = rust_pkg;
    if let Some(edit) = edit {
        edit(date, &mut rust_pkg);
    }

    MockChannel {
        name: channel.to_string(),
        date: date.to_string(),
        packages: vec![
            rust_pkg,
            rustc_pkg,
            std_pkg,
            bonus_pkg,
            ]
    }
}

#[test]
fn mock_dist_server_smoke_test() {
    let tempdir = TempDir::new("multirust").unwrap();
    let path = tempdir.path();

    create_mock_dist_server(&path, None).write(&[ManifestVersion::V2]);

    assert!(utils::path_exists(path.join("dist/2016-02-01/rustc-nightly-x86_64-apple-darwin.tar.gz")));
    assert!(utils::path_exists(path.join("dist/2016-02-01/rustc-nightly-i686-apple-darwin.tar.gz")));
    assert!(utils::path_exists(path.join("dist/2016-02-01/rust-std-nightly-x86_64-apple-darwin.tar.gz")));
    assert!(utils::path_exists(path.join("dist/2016-02-01/rust-std-nightly-i686-apple-darwin.tar.gz")));
    assert!(utils::path_exists(path.join("dist/2016-02-01/rustc-nightly-x86_64-apple-darwin.tar.gz.sha256")));
    assert!(utils::path_exists(path.join("dist/2016-02-01/rustc-nightly-i686-apple-darwin.tar.gz.sha256")));
    assert!(utils::path_exists(path.join("dist/2016-02-01/rust-std-nightly-x86_64-apple-darwin.tar.gz.sha256")));
    assert!(utils::path_exists(path.join("dist/2016-02-01/rust-std-nightly-i686-apple-darwin.tar.gz.sha256")));
    assert!(utils::path_exists(path.join("dist/channel-rust-nightly.toml")));
    assert!(utils::path_exists(path.join("dist/channel-rust-nightly.toml.sha256")));
}

// Installs or updates a toolchain from a dist server.  If an initial
// install then it will be installed with the default components.  If
// an upgrade then all the existing components will be upgraded.
// FIXME: Unify this with dist::update_from_dist
fn update_from_dist(dist_server: &Url,
                    toolchain: &ToolchainDesc,
                    prefix: &InstallPrefix,
                    add: &[Component],
                    remove: &[Component],
                    temp_cfg: &temp::Cfg,
                    notify_handler: &Fn(Notification)) -> Result<UpdateStatus> {

    // Download the dist manifest and place it into the installation prefix
    let ref manifest_url = try!(make_manifest_url(dist_server, toolchain));
    let download = DownloadCfg {
        temp_cfg: temp_cfg,
        notify_handler: notify_handler.clone(),
        gpg_key: None,
    };
    let manifest_file = try!(download.get(manifest_url.as_str()));
    let manifest_str = try!(utils::read_file("manifest", &manifest_file));
    let manifest = try!(Manifest::parse(&manifest_str));

    // Read the manifest to update the components
    let trip = toolchain.target.clone();
    let manifestation = try!(Manifestation::open(prefix.clone(), trip));

    let changes = Changes {
        add_extensions: add.to_owned(),
        remove_extensions: remove.to_owned(),
    };

    manifestation.update(&manifest, changes, temp_cfg, notify_handler.clone())
}

fn make_manifest_url(dist_server: &Url, toolchain: &ToolchainDesc) -> Result<Url> {
    let url = format!("{}/dist/channel-rust-{}.toml", dist_server, toolchain.channel);

    Ok(Url::parse(&url).unwrap())
}

fn uninstall(toolchain: &ToolchainDesc, prefix: &InstallPrefix, temp_cfg: &temp::Cfg,
             notify_handler: &Fn(Notification)) -> Result<()> {
    let trip = toolchain.target.clone();
    let manifestation = try!(Manifestation::open(prefix.clone(), trip));

    try!(manifestation.uninstall(temp_cfg, notify_handler.clone()));

    Ok(())
}

fn setup(edit: Option<&Fn(&str, &mut MockPackage)>,
         f: &Fn(&Url, &ToolchainDesc, &InstallPrefix, &temp::Cfg)) {
    let dist_tempdir = TempDir::new("multirust").unwrap();
    create_mock_dist_server(dist_tempdir.path(), edit).write(&[ManifestVersion::V2]);

    let prefix_tempdir = TempDir::new("multirust").unwrap();

    let work_tempdir = TempDir::new("multirust").unwrap();
    let ref temp_cfg = temp::Cfg::new(work_tempdir.path().to_owned(),
                                      DEFAULT_DIST_SERVER,
                                      Box::new(|_| ()));

    let ref url = Url::parse(&format!("file://{}", dist_tempdir.path().to_string_lossy())).unwrap();
    let ref toolchain = ToolchainDesc::from_str("nightly-x86_64-apple-darwin").unwrap();
    let ref prefix = InstallPrefix::from(prefix_tempdir.path().to_owned());

    f(url, toolchain, prefix, temp_cfg);
}

#[test]
fn initial_install() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, &|_| ()).unwrap();

        assert!(utils::path_exists(&prefix.path().join("bin/rustc")));
        assert!(utils::path_exists(&prefix.path().join("lib/libstd.rlib")));
    });
}

#[test]
fn test_uninstall() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, &|_| ()).unwrap();
        uninstall(toolchain, prefix, temp_cfg, &|_| ()).unwrap();

        assert!(!utils::path_exists(&prefix.path().join("bin/rustc")));
        assert!(!utils::path_exists(&prefix.path().join("lib/libstd.rlib")));
    });
}

#[test]
fn uninstall_removes_config_file() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, &|_| ()).unwrap();
        assert!(utils::path_exists(&prefix.manifest_file("multirust-config.toml")));
        uninstall(toolchain, prefix, temp_cfg, &|_| ()).unwrap();
        assert!(!utils::path_exists(&prefix.manifest_file("multirust-config.toml")));
    });
}

#[test]
fn upgrade() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        change_channel_date(url, "nightly", "2016-02-01");
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, &|_| ()).unwrap();
        assert_eq!("2016-02-01", utils_raw::read_file(&prefix.path().join("bin/rustc")).unwrap());
        change_channel_date(url, "nightly", "2016-02-02");
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, &|_| ()).unwrap();
        assert_eq!("2016-02-02", utils_raw::read_file(&prefix.path().join("bin/rustc")).unwrap());
    });
}

#[test]
fn update_removes_components_that_dont_exist() {
    // On day 1 install the 'bonus' component, on day 2 its no londer a component
    let edit = &|date: &str, pkg: &mut MockPackage| {
        if date == "2016-02-01" {
            let mut tpkg = pkg.targets.iter_mut().find(|p| p.target == "x86_64-apple-darwin").unwrap();
            tpkg.components.push(MockComponent {
                name: "bonus".to_string(),
                target: "x86_64-apple-darwin".to_string(),
            });
        }
    };
    setup(Some(edit), &|url, toolchain, prefix, temp_cfg| {
        change_channel_date(url, "nightly", "2016-02-01");
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, &|_| ()).unwrap();
        assert!(utils::path_exists(&prefix.path().join("bin/bonus")));
        change_channel_date(url, "nightly", "2016-02-02");
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, &|_| ()).unwrap();
        assert!(!utils::path_exists(&prefix.path().join("bin/bonus")));
    });
}

#[test]
fn update_preserves_extensions() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: Some(TargetTriple::from_str("i686-apple-darwin"))
            },
            Component {
                pkg: "rust-std".to_string(), target: Some(TargetTriple::from_str("i686-unknown-linux-gnu"))
            }
            ];

        change_channel_date(url, "nightly", "2016-02-01");
        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, &|_| ()).unwrap();

        assert!(utils::path_exists(&prefix.path().join("lib/i686-apple-darwin/libstd.rlib")));
        assert!(utils::path_exists(&prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")));

        change_channel_date(url, "nightly", "2016-02-02");
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, &|_| ()).unwrap();

        assert!(utils::path_exists(&prefix.path().join("lib/i686-apple-darwin/libstd.rlib")));
        assert!(utils::path_exists(&prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")));
    });
}

#[test]
fn update_preserves_extensions_that_became_components() {
    let edit = &|date: &str, pkg: &mut MockPackage| {
        if date == "2016-02-01" {
            let mut tpkg = pkg.targets.iter_mut().find(|p| p.target == "x86_64-apple-darwin").unwrap();
            tpkg.extensions.push(MockComponent {
                name: "bonus".to_string(),
                target: "x86_64-apple-darwin".to_string(),
            });
        }
        if date == "2016-02-02" {
            let mut tpkg = pkg.targets.iter_mut().find(|p| p.target == "x86_64-apple-darwin").unwrap();
            tpkg.components.push(MockComponent {
                name: "bonus".to_string(),
                target: "x86_64-apple-darwin".to_string(),
            });
        }
    };
    setup(Some(edit), &|url, toolchain, prefix, temp_cfg| {
        let ref adds = vec![
            Component {
                pkg: "bonus".to_string(), target: Some(TargetTriple::from_str("x86_64-apple-darwin"))
            },
            ];

        change_channel_date(url, "nightly", "2016-02-01");
        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, &|_| ()).unwrap();

        assert!(utils::path_exists(&prefix.path().join("bin/bonus")));

        change_channel_date(url, "nightly", "2016-02-02");
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, &|_| ()).unwrap();
        assert!(utils::path_exists(&prefix.path().join("bin/bonus")));
    });
}

#[test]
fn update_preserves_components_that_became_extensions() {
    let edit = &|date: &str, pkg: &mut MockPackage| {
        if date == "2016-02-01" {
            let mut tpkg = pkg.targets.iter_mut().find(|p| p.target == "x86_64-apple-darwin").unwrap();
            tpkg.components.push(MockComponent {
                name: "bonus".to_string(),
                target: "x86_64-apple-darwin".to_string(),
            });
        }
        if date == "2016-02-02" {
            let mut tpkg = pkg.targets.iter_mut().find(|p| p.target == "x86_64-apple-darwin").unwrap();
            tpkg.extensions.push(MockComponent {
                name: "bonus".to_string(),
                target: "x86_64-apple-darwin".to_string(),
            });
        }
    };
    setup(Some(edit), &|url, toolchain, prefix, temp_cfg| {
        change_channel_date(url, "nightly", "2016-02-01");
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, &|_| ()).unwrap();
        assert!(utils::path_exists(&prefix.path().join("bin/bonus")));
        change_channel_date(url, "nightly", "2016-02-02");
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, &|_| ()).unwrap();
        assert!(utils::path_exists(&prefix.path().join("bin/bonus")));
    });
}

#[test]
fn update_makes_no_changes_for_identical_manifest() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let status = update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, &|_| ()).unwrap();
        assert_eq!(status, UpdateStatus::Changed);
        let status = update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, &|_| ()).unwrap();
        assert_eq!(status, UpdateStatus::Unchanged);
    });
}

#[test]
fn add_extensions_for_initial_install() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: Some(TargetTriple::from_str("i686-apple-darwin"))
            },
            Component {
                pkg: "rust-std".to_string(), target: Some(TargetTriple::from_str("i686-unknown-linux-gnu"))
            }
        ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, &|_| ()).unwrap();
        assert!(utils::path_exists(&prefix.path().join("lib/i686-apple-darwin/libstd.rlib")));
        assert!(utils::path_exists(&prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")));
    });
}

#[test]
fn add_extensions_for_same_manifest() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, &|_| ()).unwrap();

        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: Some(TargetTriple::from_str("i686-apple-darwin"))
            },
            Component {
                pkg: "rust-std".to_string(), target: Some(TargetTriple::from_str("i686-unknown-linux-gnu"))
            }
        ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, &|_| ()).unwrap();

        assert!(utils::path_exists(&prefix.path().join("lib/i686-apple-darwin/libstd.rlib")));
        assert!(utils::path_exists(&prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")));
    });
}

#[test]
fn add_extensions_for_upgrade() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        change_channel_date(url, "nightly", "2016-02-01");

        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, &|_| ()).unwrap();

        change_channel_date(url, "nightly", "2016-02-02");

        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: Some(TargetTriple::from_str("i686-apple-darwin"))
            },
            Component {
                pkg: "rust-std".to_string(), target: Some(TargetTriple::from_str("i686-unknown-linux-gnu"))
            }
        ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, &|_| ()).unwrap();

        assert!(utils::path_exists(&prefix.path().join("lib/i686-apple-darwin/libstd.rlib")));
        assert!(utils::path_exists(&prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")));
    });
}

#[test]
#[should_panic]
fn add_extension_not_in_manifest() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let ref adds = vec![
            Component {
                pkg: "rust-bogus".to_string(), target: Some(TargetTriple::from_str("i686-apple-darwin"))
            },
        ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, &|_| ()).unwrap();
    });
}

#[test]
#[should_panic]
fn add_extension_that_is_required_component() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let ref adds = vec![
            Component {
                pkg: "rustc".to_string(), target: Some(TargetTriple::from_str("x86_64-apple-darwin"))
            },
        ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, &|_| ()).unwrap();
    });
}

#[test]
#[ignore]
fn add_extensions_for_same_manifest_does_not_reinstall_other_components() {
}

#[test]
#[ignore]
fn add_extensions_for_same_manifest_when_extension_already_installed() {
}

#[test]
fn add_extensions_does_not_remove_other_components() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, &|_| ()).unwrap();

        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: Some(TargetTriple::from_str("i686-apple-darwin"))
            },
        ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, &|_| ()).unwrap();

        assert!(utils::path_exists(&prefix.path().join("bin/rustc")));
    });
}

// Asking to remove extensions on initial install is nonsese.
#[test]
#[should_panic]
fn remove_extensions_for_initial_install() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let ref removes = vec![
            Component {
                pkg: "rustc".to_string(), target: Some(TargetTriple::from_str("x86_64-apple-darwin"))
            },
        ];

        update_from_dist(url, toolchain, prefix, &[], removes, temp_cfg, &|_| ()).unwrap();
    });
}

#[test]
fn remove_extensions_for_same_manifest() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: Some(TargetTriple::from_str("i686-apple-darwin"))
            },
            Component {
                pkg: "rust-std".to_string(), target: Some(TargetTriple::from_str("i686-unknown-linux-gnu"))
            }
        ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, &|_| ()).unwrap();

        let ref removes = vec![
            Component {
                pkg: "rust-std".to_string(), target: Some(TargetTriple::from_str("i686-apple-darwin"))
            },
            ];

        update_from_dist(url, toolchain, prefix, &[], removes, temp_cfg, &|_| ()).unwrap();

        assert!(!utils::path_exists(&prefix.path().join("lib/i686-apple-darwin/libstd.rlib")));
        assert!(utils::path_exists(&prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")));
    });
}

#[test]
fn remove_extensions_for_upgrade() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        change_channel_date(url, "nightly", "2016-02-01");

        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: Some(TargetTriple::from_str("i686-apple-darwin"))
            },
            Component {
                pkg: "rust-std".to_string(), target: Some(TargetTriple::from_str("i686-unknown-linux-gnu"))
            }
        ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, &|_| ()).unwrap();

        change_channel_date(url, "nightly", "2016-02-02");

        let ref removes = vec![
            Component {
                pkg: "rust-std".to_string(), target: Some(TargetTriple::from_str("i686-apple-darwin"))
            },
        ];

        update_from_dist(url, toolchain, prefix, &[], removes, temp_cfg, &|_| ()).unwrap();

        assert!(!utils::path_exists(&prefix.path().join("lib/i686-apple-darwin/libstd.rlib")));
        assert!(utils::path_exists(&prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")));
    });
}

#[test]
#[should_panic]
fn remove_extension_not_in_manifest() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        change_channel_date(url, "nightly", "2016-02-01");

        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, &|_| ()).unwrap();

        change_channel_date(url, "nightly", "2016-02-02");

        let ref removes = vec![
            Component {
                pkg: "rust-bogus".to_string(), target: Some(TargetTriple::from_str("i686-apple-darwin"))
            },
        ];

        update_from_dist(url, toolchain, prefix, &[], removes, temp_cfg, &|_| ()).unwrap();
    });
}

// Extensions that don't exist in the manifest may still exist on disk
// from a previous manifest. The can't be requested to be removed though;
// only things in the manifest can.
#[test]
#[should_panic]
fn remove_extension_not_in_manifest_but_is_already_installed() {
    let edit = &|date: &str, pkg: &mut MockPackage| {
        if date == "2016-02-01" {
            let mut tpkg = pkg.targets.iter_mut().find(|p| p.target == "x86_64-apple-darwin").unwrap();
            tpkg.extensions.push(MockComponent {
                name: "bonus".to_string(),
                target: "x86_64-apple-darwin".to_string(),
            });
        }
    };
    setup(Some(edit), &|url, toolchain, prefix, temp_cfg| {
        change_channel_date(url, "nightly", "2016-02-01");

        let ref adds = vec![
            Component {
                pkg: "bonus".to_string(), target: Some(TargetTriple::from_str("x86_64-apple-darwin"))
            },
        ];
        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, &|_| ()).unwrap();
        assert!(utils::path_exists(&prefix.path().join("bin/bonus")));

        change_channel_date(url, "nightly", "2016-02-02");

        let ref removes = vec![
            Component {
                pkg: "bonus".to_string(), target: Some(TargetTriple::from_str("x86_64-apple-darwin"))
            },
        ];
        update_from_dist(url, toolchain, prefix, &[], removes, temp_cfg, &|_| ()).unwrap();
    });
}

#[test]
#[should_panic]
fn remove_extension_that_is_required_component() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, &|_| ()).unwrap();

        let ref removes = vec![
            Component {
                pkg: "rustc".to_string(), target: Some(TargetTriple::from_str("x86_64-apple-darwin"))
            },
        ];

        update_from_dist(url, toolchain, prefix, &[], removes, temp_cfg, &|_| ()).unwrap();
    });
}

#[test]
#[should_panic]
fn remove_extension_not_installed() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, &|_| ()).unwrap();

        let ref removes = vec![
            Component {
                pkg: "rust-std".to_string(), target: Some(TargetTriple::from_str("i686-apple-darwin"))
            },
        ];

        update_from_dist(url, toolchain, prefix, &[], removes, temp_cfg, &|_| ()).unwrap();
    });
}

#[test]
#[ignore]
fn remove_extensions_for_same_manifest_does_not_reinstall_other_components() {
}

#[test]
fn remove_extensions_does_not_remove_other_components() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: Some(TargetTriple::from_str("i686-apple-darwin"))
            },
        ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, &|_| ()).unwrap();

        let ref removes = vec![
            Component {
                pkg: "rust-std".to_string(), target: Some(TargetTriple::from_str("i686-apple-darwin"))
            },
        ];

        update_from_dist(url, toolchain, prefix, &[], removes, temp_cfg, &|_| ()).unwrap();

        assert!(utils::path_exists(&prefix.path().join("bin/rustc")));
    });
}

#[test]
fn add_and_remove_for_upgrade() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        change_channel_date(url, "nightly", "2016-02-01");

        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: Some(TargetTriple::from_str("i686-unknown-linux-gnu"))
            },
        ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, &|_| ()).unwrap();

        change_channel_date(url, "nightly", "2016-02-02");

        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: Some(TargetTriple::from_str("i686-apple-darwin"))
            },
        ];

        let ref removes = vec![
            Component {
                pkg: "rust-std".to_string(), target: Some(TargetTriple::from_str("i686-unknown-linux-gnu"))
            },
        ];

        update_from_dist(url, toolchain, prefix, adds, removes, temp_cfg, &|_| ()).unwrap();

        assert!(utils::path_exists(&prefix.path().join("lib/i686-apple-darwin/libstd.rlib")));
        assert!(!utils::path_exists(&prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")));
    });
}

#[test]
fn add_and_remove() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: Some(TargetTriple::from_str("i686-unknown-linux-gnu"))
            },
        ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, &|_| ()).unwrap();

        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: Some(TargetTriple::from_str("i686-apple-darwin"))
            },
        ];

        let ref removes = vec![
            Component {
                pkg: "rust-std".to_string(), target: Some(TargetTriple::from_str("i686-unknown-linux-gnu"))
            },
        ];

        update_from_dist(url, toolchain, prefix, adds, removes, temp_cfg, &|_| ()).unwrap();

        assert!(utils::path_exists(&prefix.path().join("lib/i686-apple-darwin/libstd.rlib")));
        assert!(!utils::path_exists(&prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")));
    });
}

#[test]
#[should_panic]
fn add_and_remove_same_component() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, &|_| ()).unwrap();

        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: Some(TargetTriple::from_str("i686-apple-darwin"))
            },
        ];

        let ref removes = vec![
            Component {
                pkg: "rust-std".to_string(), target: Some(TargetTriple::from_str("i686-apple_darwin"))
            },
        ];

        update_from_dist(url, toolchain, prefix, adds, removes, temp_cfg, &|_| ()).unwrap();
    });
}

#[test]
fn bad_component_hash() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let path = url.to_file_path().unwrap();
        let path = path.join("dist/2016-02-02/rustc-nightly-x86_64-apple-darwin.tar.gz");
        utils_raw::write_file(&path, "bogus").unwrap();

        let err = update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, &|_| ()).unwrap_err();

        match *err.kind() {
            ErrorKind::ChecksumFailed { .. } => (),
            _ => panic!()
        }
    });
}

#[test]
fn unable_to_download_component() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let path = url.to_file_path().unwrap();
        let path = path.join("dist/2016-02-02/rustc-nightly-x86_64-apple-darwin.tar.gz");
        fs::remove_file(&path).unwrap();

        let err = update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, &|_| ()).unwrap_err();

        match *err.kind() {
            ErrorKind::ComponentDownloadFailed(..) => (),
            _ => panic!()
        }
    });
}
