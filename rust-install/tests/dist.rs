// Tests of installation and updates from a v2 Rust distribution
// server (mocked on the file system)

extern crate rust_install;
extern crate rust_manifest;
extern crate tempdir;
extern crate tar;
extern crate openssl;
extern crate toml;
extern crate flate2;
extern crate walkdir;
extern crate itertools;
extern crate hyper;

use rust_install::{InstallPrefix, InstallType, Error, NotifyHandler};
use rust_install::mock::dist::*;
use rust_install::dist::ToolchainDesc;
use rust_install::download::DownloadCfg;
use rust_install::utils;
use rust_install::temp;
use rust_install::manifest::{Manifestation, UpdateStatus, Changes};
use rust_manifest::{Manifest, Component};
use hyper::Url;
use std::fs;
use std::io::Write;
use tempdir::TempDir;
use itertools::Itertools;

#[test]
fn mock_dist_server_smoke_test() {
    let tempdir = TempDir::new("multirust").unwrap();
    let path = tempdir.path();

    create_mock_dist_server(&path, None).write();

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
fn update_from_dist(dist_server: &Url,
                    toolchain: &ToolchainDesc,
                    prefix: &InstallPrefix,
                    add: &[Component],
                    remove: &[Component],
                    temp_cfg: &temp::Cfg,
                    notify_handler: NotifyHandler) -> Result<UpdateStatus, Error> {

    // Download the dist manifest and place it into the installation prefix
    let ref manifest_url = try!(make_manifest_url(dist_server, toolchain));
    let download = DownloadCfg {
        temp_cfg: temp_cfg,
        notify_handler: notify_handler.clone(),
        gpg_key: None,
    };
    let manifest_file = try!(download.get(&manifest_url.serialize()));
    let manifest_str = try!(utils::read_file("manifest", &manifest_file));
    let manifest = try!(Manifest::parse(&manifest_str));

    // Read the manifest to update the components
    let trip = try!(toolchain.target_triple().ok_or_else(|| Error::UnsupportedHost(toolchain.full_spec())));
    let manifestation = try!(Manifestation::open(prefix.clone(), &trip));

    let changes = Changes {
        add_extensions: add.to_owned(),
        remove_extensions: remove.to_owned(),
    };

    manifestation.update(&manifest, changes, temp_cfg, notify_handler.clone())
}

fn uninstall(toolchain: &ToolchainDesc, prefix: &InstallPrefix, temp_cfg: &temp::Cfg,
             notify_handler: NotifyHandler) -> Result<(), Error> {
    let trip = try!(toolchain.target_triple().ok_or_else(|| Error::UnsupportedHost(toolchain.full_spec())));
    let manifestation = try!(Manifestation::open(prefix.clone(), &trip));

    try!(manifestation.uninstall(temp_cfg, notify_handler.clone()));

    Ok(())
}

fn make_manifest_url(dist_server: &Url, toolchain: &ToolchainDesc) -> Result<Url, Error> {
    let mut url = dist_server.clone();
    if let Some(mut p) = url.path_mut() {
        p.push(format!("dist/channel-rust-{}.toml", toolchain.channel));
    } else {
        // FIXME
        panic!()
    }

    Ok(url)
}

fn setup(edit: Option<&Fn(&str, &mut MockPackage)>,
         f: &Fn(&Url, &ToolchainDesc, &InstallPrefix, &temp::Cfg)) {
    let dist_tempdir = TempDir::new("multirust").unwrap();
    create_mock_dist_server(dist_tempdir.path(), edit).write();

    let prefix_tempdir = TempDir::new("multirust").unwrap();

    let work_tempdir = TempDir::new("multirust").unwrap();
    let ref temp_cfg = temp::Cfg::new(work_tempdir.path().to_owned(), temp::SharedNotifyHandler::none());

    let ref url = Url::parse(&format!("file://{}", dist_tempdir.path().to_string_lossy())).unwrap();
    let ref toolchain = ToolchainDesc::from_str("x86_64-apple-darwin-nightly").unwrap();
    let ref prefix = InstallPrefix::from(prefix_tempdir.path().to_owned(), InstallType::Shared);

    f(url, toolchain, prefix, temp_cfg);
}

#[test]
fn initial_install() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();

        assert!(utils::path_exists(&prefix.path().join("bin/rustc")));
        assert!(utils::path_exists(&prefix.path().join("lib/libstd.rlib")));
    });
}

#[test]
fn test_uninstall() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();
        uninstall(toolchain, prefix, temp_cfg, NotifyHandler::none()).unwrap();

        assert!(!utils::path_exists(&prefix.path().join("bin/rustc")));
        assert!(!utils::path_exists(&prefix.path().join("lib/libstd.rlib")));
    });
}

#[test]
fn uninstall_removes_config_file() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();
        assert!(utils::path_exists(&prefix.manifest_file("multirust-config.toml")));
        uninstall(toolchain, prefix, temp_cfg, NotifyHandler::none()).unwrap();
        assert!(!utils::path_exists(&prefix.manifest_file("multirust-config.toml")));
    });
}

#[test]
fn upgrade() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();
        assert_eq!("2016-02-01", utils::raw::read_file(&prefix.path().join("bin/rustc")).unwrap());
        change_channel_date(url, "nightly", "2016-02-02");
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();
        assert_eq!("2016-02-02", utils::raw::read_file(&prefix.path().join("bin/rustc")).unwrap());
    });
}

#[test]
fn update_removes_components_that_dont_exist() {
    // On day 1 install the 'bonus' component, on day 2 its no londer a component
    let edit = &|date: &str, pkg: &mut MockPackage| {
        if date == "2016-02-01" {
            let mut tpkg = pkg.targets.iter_mut().find(|p| p.target == "x86_64-apple-darwin").unwrap();
            tpkg.components.push(MockComponent {
                name: "bonus",
                target: "x86_64-apple-darwin",
            });
        }
    };
    setup(Some(edit), &|url, toolchain, prefix, temp_cfg| {
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();
        assert!(utils::path_exists(&prefix.path().join("bin/bonus")));
        change_channel_date(url, "nightly", "2016-02-02");
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();
        assert!(!utils::path_exists(&prefix.path().join("bin/bonus")));
    });
}

#[test]
fn update_preserves_extensions() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-apple-darwin".to_string()
            },
            Component {
                pkg: "rust-std".to_string(), target: "i686-unknown-linux-gnu".to_string()
            }
            ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, NotifyHandler::none()).unwrap();

        assert!(utils::path_exists(&prefix.path().join("lib/i686-apple-darwin/libstd.rlib")));
        assert!(utils::path_exists(&prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")));

        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();

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
                name: "bonus",
                target: "x86_64-apple-darwin",
            });
        }
        if date == "2016-02-02" {
            let mut tpkg = pkg.targets.iter_mut().find(|p| p.target == "x86_64-apple-darwin").unwrap();
            tpkg.components.push(MockComponent {
                name: "bonus",
                target: "x86_64-apple-darwin",
            });
        }
    };
    setup(Some(edit), &|url, toolchain, prefix, temp_cfg| {
        let ref adds = vec![
            Component {
                pkg: "bonus".to_string(), target: "x86_64-apple-darwin".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, NotifyHandler::none()).unwrap();

        assert!(utils::path_exists(&prefix.path().join("bin/bonus")));

        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();
        assert!(utils::path_exists(&prefix.path().join("bin/bonus")));
    });
}

#[test]
fn update_preserves_components_that_became_extensions() {
    let edit = &|date: &str, pkg: &mut MockPackage| {
        if date == "2016-02-01" {
            let mut tpkg = pkg.targets.iter_mut().find(|p| p.target == "x86_64-apple-darwin").unwrap();
            tpkg.components.push(MockComponent {
                name: "bonus",
                target: "x86_64-apple-darwin",
            });
        }
        if date == "2016-02-02" {
            let mut tpkg = pkg.targets.iter_mut().find(|p| p.target == "x86_64-apple-darwin").unwrap();
            tpkg.extensions.push(MockComponent {
                name: "bonus",
                target: "x86_64-apple-darwin",
            });
        }
    };
    setup(Some(edit), &|url, toolchain, prefix, temp_cfg| {
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();
        assert!(utils::path_exists(&prefix.path().join("bin/bonus")));
        change_channel_date(url, "nightly", "2016-02-02");
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();
        assert!(utils::path_exists(&prefix.path().join("bin/bonus")));
    });
}

#[test]
fn update_makes_no_changes_for_identical_manifest() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let status = update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();
        assert_eq!(status, UpdateStatus::Changed);
        let status = update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();
        assert_eq!(status, UpdateStatus::Unchanged);
    });
}

#[test]
fn add_extensions_for_initial_install() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-apple-darwin".to_string()
            },
            Component {
                pkg: "rust-std".to_string(), target: "i686-unknown-linux-gnu".to_string()
            }
            ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, NotifyHandler::none()).unwrap();
        assert!(utils::path_exists(&prefix.path().join("lib/i686-apple-darwin/libstd.rlib")));
        assert!(utils::path_exists(&prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")));
    });
}

#[test]
fn add_extensions_for_same_manifest() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();

        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-apple-darwin".to_string()
            },
            Component {
                pkg: "rust-std".to_string(), target: "i686-unknown-linux-gnu".to_string()
            }
            ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, NotifyHandler::none()).unwrap();

        assert!(utils::path_exists(&prefix.path().join("lib/i686-apple-darwin/libstd.rlib")));
        assert!(utils::path_exists(&prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")));
    });
}

#[test]
fn add_extensions_for_upgrade() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();

        change_channel_date(url, "nightly", "2016-02-02");

        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-apple-darwin".to_string()
            },
            Component {
                pkg: "rust-std".to_string(), target: "i686-unknown-linux-gnu".to_string()
            }
            ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, NotifyHandler::none()).unwrap();

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
                pkg: "rust-bogus".to_string(), target: "i686-apple-darwin".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, NotifyHandler::none()).unwrap();
    });
}

#[test]
#[should_panic]
fn add_extension_that_is_required_component() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let ref adds = vec![
            Component {
                pkg: "rustc".to_string(), target: "x86_64-apple-darwin".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, NotifyHandler::none()).unwrap();
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
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();

        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-apple-darwin".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, NotifyHandler::none()).unwrap();

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
                pkg: "rustc".to_string(), target: "x86_64-apple-darwin".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, &[], removes, temp_cfg, NotifyHandler::none()).unwrap();
    });
}

#[test]
fn remove_extensions_for_same_manifest() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-apple-darwin".to_string()
            },
            Component {
                pkg: "rust-std".to_string(), target: "i686-unknown-linux-gnu".to_string()
            }
            ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, NotifyHandler::none()).unwrap();

        let ref removes = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-apple-darwin".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, &[], removes, temp_cfg, NotifyHandler::none()).unwrap();

        assert!(!utils::path_exists(&prefix.path().join("lib/i686-apple-darwin/libstd.rlib")));
        assert!(utils::path_exists(&prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")));
    });
}

#[test]
fn remove_extensions_for_upgrade() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-apple-darwin".to_string()
            },
            Component {
                pkg: "rust-std".to_string(), target: "i686-unknown-linux-gnu".to_string()
            }
            ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, NotifyHandler::none()).unwrap();

        change_channel_date(url, "nightly", "2016-02-02");

        let ref removes = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-apple-darwin".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, &[], removes, temp_cfg, NotifyHandler::none()).unwrap();

        assert!(!utils::path_exists(&prefix.path().join("lib/i686-apple-darwin/libstd.rlib")));
        assert!(utils::path_exists(&prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")));
    });
}

#[test]
#[should_panic]
fn remove_extension_not_in_manifest() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();

        change_channel_date(url, "nightly", "2016-02-02");

        let ref removes = vec![
            Component {
                pkg: "rust-bogus".to_string(), target: "i686-apple-darwin".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, &[], removes, temp_cfg, NotifyHandler::none()).unwrap();
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
                name: "bonus",
                target: "x86_64-apple-darwin",
            });
        }
    };
    setup(Some(edit), &|url, toolchain, prefix, temp_cfg| {
        let ref adds = vec![
            Component {
                pkg: "bonus".to_string(), target: "x86_64-apple-darwin".to_string()
            },
            ];
        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, NotifyHandler::none()).unwrap();
        assert!(utils::path_exists(&prefix.path().join("bin/bonus")));

        change_channel_date(url, "nightly", "2016-02-02");

        let ref removes = vec![
            Component {
                pkg: "bonus".to_string(), target: "x86_64-apple-darwin".to_string()
            },
            ];
        update_from_dist(url, toolchain, prefix, &[], removes, temp_cfg, NotifyHandler::none()).unwrap();
    });
}

#[test]
#[should_panic]
fn remove_extension_that_is_required_component() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();

        let ref removes = vec![
            Component {
                pkg: "rustc".to_string(), target: "x86_64-apple-darwin".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, &[], removes, temp_cfg, NotifyHandler::none()).unwrap();
    });
}

#[test]
#[should_panic]
fn remove_extension_not_installed() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();

        let ref removes = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-apple-darwin".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, &[], removes, temp_cfg, NotifyHandler::none()).unwrap();
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
                pkg: "rust-std".to_string(), target: "i686-apple-darwin".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, NotifyHandler::none()).unwrap();

        let ref removes = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-apple-darwin".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, &[], removes, temp_cfg, NotifyHandler::none()).unwrap();

        assert!(utils::path_exists(&prefix.path().join("bin/rustc")));
    });
}

#[test]
fn add_and_remove_for_upgrade() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-unknown-linux-gnu".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, NotifyHandler::none()).unwrap();

        change_channel_date(url, "nightly", "2016-02-02");

        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-apple-darwin".to_string()
            },
            ];

        let ref removes = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-unknown-linux-gnu".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, adds, removes, temp_cfg, NotifyHandler::none()).unwrap();

        assert!(utils::path_exists(&prefix.path().join("lib/i686-apple-darwin/libstd.rlib")));
        assert!(!utils::path_exists(&prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")));
    });
}

#[test]
fn add_and_remove() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-unknown-linux-gnu".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, adds, &[], temp_cfg, NotifyHandler::none()).unwrap();

        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-apple-darwin".to_string()
            },
            ];

        let ref removes = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-unknown-linux-gnu".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, adds, removes, temp_cfg, NotifyHandler::none()).unwrap();

        assert!(utils::path_exists(&prefix.path().join("lib/i686-apple-darwin/libstd.rlib")));
        assert!(!utils::path_exists(&prefix.path().join("lib/i686-unknown-linux-gnu/libstd.rlib")));
    });
}

#[test]
#[should_panic]
fn add_and_remove_same_component() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap();

        let ref adds = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-apple-darwin".to_string()
            },
            ];

        let ref removes = vec![
            Component {
                pkg: "rust-std".to_string(), target: "i686-apple_darwin".to_string()
            },
            ];

        update_from_dist(url, toolchain, prefix, adds, removes, temp_cfg, NotifyHandler::none()).unwrap();
    });
}

#[test]
fn bad_component_hash() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let path = url.to_file_path().unwrap();
        let path = path.join("dist/2016-02-01/rustc-nightly-x86_64-apple-darwin.tar.gz");
        utils::raw::write_file(&path, "bogus").unwrap();

        let err = update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap_err();

        match err {
            Error::ChecksumFailed { .. } => (),
            _ => panic!()
        }
    });
}

#[test]
fn unable_to_download_component() {
    setup(None, &|url, toolchain, prefix, temp_cfg| {
        let path = url.to_file_path().unwrap();
        let path = path.join("dist/2016-02-01/rustc-nightly-x86_64-apple-darwin.tar.gz");
        fs::remove_file(&path).unwrap();

        let err = update_from_dist(url, toolchain, prefix, &[], &[], temp_cfg, NotifyHandler::none()).unwrap_err();

        match err {
            Error::ComponentDownloadFailed(..) => (),
            _ => panic!()
        }
    });
}
