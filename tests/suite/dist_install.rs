use std::fs::File;
use std::io::Write;
use std::sync::Arc;

use rustup::dist::component::Components;
use rustup::dist::component::Transaction;
use rustup::dist::component::{DirectoryPackage, Package};
use rustup::dist::prefix::InstallPrefix;
use rustup::notifications::Notification;
use rustup::test::{DistContext, MockComponentBuilder, MockFile, MockInstallerBuilder};
use rustup::utils;

// Just testing that the mocks work
#[test]
fn mock_smoke_test() {
    let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();

    let mock = MockInstallerBuilder {
        components: vec![
            MockComponentBuilder {
                name: "mycomponent".to_string(),
                files: vec![
                    MockFile::new("bin/foo", b"foo"),
                    MockFile::new("lib/bar", b"bar"),
                    MockFile::new_dir("doc/stuff", &[("doc1", b"", false), ("doc2", b"", false)]),
                ],
            },
            MockComponentBuilder {
                name: "mycomponent2".to_string(),
                files: vec![MockFile::new("bin/quux", b"quux")],
            },
        ],
    };

    mock.build(tempdir.path());

    assert!(tempdir.path().join("components").exists());
    assert!(tempdir.path().join("mycomponent/manifest.in").exists());
    assert!(tempdir.path().join("mycomponent/bin/foo").exists());
    assert!(tempdir.path().join("mycomponent/lib/bar").exists());
    assert!(tempdir.path().join("mycomponent/doc/stuff/doc1").exists());
    assert!(tempdir.path().join("mycomponent/doc/stuff/doc2").exists());
    assert!(tempdir.path().join("mycomponent2/manifest.in").exists());
    assert!(tempdir.path().join("mycomponent2/bin/quux").exists());
}

#[test]
fn package_contains() {
    let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();

    let mock = MockInstallerBuilder {
        components: vec![
            MockComponentBuilder {
                name: "mycomponent".to_string(),
                files: vec![MockFile::new("bin/foo", b"foo")],
            },
            MockComponentBuilder {
                name: "mycomponent2".to_string(),
                files: vec![MockFile::new("bin/bar", b"bar")],
            },
        ],
    };

    mock.build(tempdir.path());

    let package = DirectoryPackage::new(tempdir.path().to_owned(), true).unwrap();
    assert!(package.contains("mycomponent", None));
    assert!(package.contains("mycomponent2", None));
}

#[test]
fn package_bad_version() {
    let tempdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();

    let mock = MockInstallerBuilder {
        components: vec![MockComponentBuilder {
            name: "mycomponent".to_string(),
            files: vec![MockFile::new("bin/foo", b"foo")],
        }],
    };

    mock.build(tempdir.path());

    let mut ver = File::create(tempdir.path().join("rust-installer-version")).unwrap();
    writeln!(ver, "100").unwrap();

    assert!(DirectoryPackage::new(tempdir.path().to_owned(), true).is_err());
}

#[test]
fn basic_install() {
    let mock = MockInstallerBuilder {
        components: vec![MockComponentBuilder {
            name: "mycomponent".to_string(),
            files: vec![
                MockFile::new("bin/foo", b"foo"),
                MockFile::new("lib/bar", b"bar"),
                MockFile::new_dir("doc/stuff", &[("doc1", b"", false), ("doc2", b"", false)]),
            ],
        }],
    };

    let cx = DistContext::new(Some(mock)).unwrap();
    let (tx, components, pkg) = cx.start().unwrap();
    let tx = pkg.install(&components, "mycomponent", None, tx).unwrap();
    tx.commit();

    assert!(utils::path_exists(cx.inst_dir.path().join("bin/foo")));
    assert!(utils::path_exists(cx.inst_dir.path().join("lib/bar")));
    assert!(utils::path_exists(
        cx.inst_dir.path().join("doc/stuff/doc1")
    ));
    assert!(utils::path_exists(
        cx.inst_dir.path().join("doc/stuff/doc2")
    ));

    assert!(components.find("mycomponent").unwrap().is_some());
}

#[test]
fn multiple_component_install() {
    let mock = MockInstallerBuilder {
        components: vec![
            MockComponentBuilder {
                name: "mycomponent".to_string(),
                files: vec![MockFile::new("bin/foo", b"foo")],
            },
            MockComponentBuilder {
                name: "mycomponent2".to_string(),
                files: vec![MockFile::new("lib/bar", b"bar")],
            },
        ],
    };

    let cx = DistContext::new(Some(mock)).unwrap();
    let (tx, components, pkg) = cx.start().unwrap();
    let tx = pkg.install(&components, "mycomponent", None, tx).unwrap();
    let tx = pkg.install(&components, "mycomponent2", None, tx).unwrap();
    tx.commit();

    assert!(utils::path_exists(cx.inst_dir.path().join("bin/foo")));
    assert!(utils::path_exists(cx.inst_dir.path().join("lib/bar")));

    assert!(components.find("mycomponent").unwrap().is_some());
    assert!(components.find("mycomponent2").unwrap().is_some());
}

#[test]
fn uninstall() {
    let mock = MockInstallerBuilder {
        components: vec![
            MockComponentBuilder {
                name: "mycomponent".to_string(),
                files: vec![
                    MockFile::new("bin/foo", b"foo"),
                    MockFile::new("lib/bar", b"bar"),
                    MockFile::new_dir("doc/stuff", &[("doc1", b"", false), ("doc2", b"", false)]),
                ],
            },
            MockComponentBuilder {
                name: "mycomponent2".to_string(),
                files: vec![MockFile::new("lib/quux", b"quux")],
            },
        ],
    };

    let cx = DistContext::new(Some(mock)).unwrap();
    let (tx, components, pkg) = cx.start().unwrap();
    let tx = pkg.install(&components, "mycomponent", None, tx).unwrap();
    let tx = pkg.install(&components, "mycomponent2", None, tx).unwrap();
    tx.commit();

    // Now uninstall
    let notify = |_: Notification<'_>| ();
    let mut tx = Transaction::new(
        cx.prefix.clone(),
        Arc::new(cx.cx),
        Arc::new(notify),
        Arc::new(cx.tp.process.clone()),
    );
    for component in components.list().unwrap() {
        tx = component.uninstall(tx, &cx.tp.process).unwrap();
    }
    tx.commit();

    assert!(!utils::path_exists(cx.inst_dir.path().join("bin/foo")));
    assert!(!utils::path_exists(cx.inst_dir.path().join("lib/bar")));
    assert!(!utils::path_exists(
        cx.inst_dir.path().join("doc/stuff/doc1")
    ));
    assert!(!utils::path_exists(
        cx.inst_dir.path().join("doc/stuff/doc2")
    ));
    assert!(!utils::path_exists(cx.inst_dir.path().join("doc/stuff")));
    assert!(components.find("mycomponent").unwrap().is_none());
    assert!(components.find("mycomponent2").unwrap().is_none());
}

// If any single file can't be uninstalled, it is not a fatal error
// and the subsequent files will still be removed.
#[test]
fn uninstall_best_effort() {
    //unimplemented!()
}

#[test]
fn component_bad_version() {
    let mock = MockInstallerBuilder {
        components: vec![MockComponentBuilder {
            name: "mycomponent".to_string(),
            files: vec![MockFile::new("bin/foo", b"foo")],
        }],
    };

    let cx = DistContext::new(Some(mock)).unwrap();
    let (tx, components, pkg) = cx.start().unwrap();
    let tx = pkg.install(&components, "mycomponent", None, tx).unwrap();
    tx.commit();

    // Write a bogus version to the component manifest directory
    utils::write_file(
        "",
        &cx.prefix.manifest_file("rust-installer-version"),
        "100\n",
    )
    .unwrap();

    // Can't open components now
    let e = Components::open(cx.prefix).unwrap_err();
    assert_eq!(
        "unsupported metadata version in existing installation: 100",
        format!("{e}")
    );
}

// Installing to a prefix that doesn't exist creates it automatically
#[test]
fn install_to_prefix_that_does_not_exist() {
    let mock = MockInstallerBuilder {
        components: vec![MockComponentBuilder {
            name: "mycomponent".to_string(),
            files: vec![MockFile::new("bin/foo", b"foo")],
        }],
    };

    let mut cx = DistContext::new(Some(mock)).unwrap();
    let does_not_exist = cx.inst_dir.path().join("does_not_exist");
    cx.prefix = InstallPrefix::from(does_not_exist.clone());
    let (tx, components, pkg) = cx.start().unwrap();
    let tx = pkg.install(&components, "mycomponent", None, tx).unwrap();
    tx.commit();

    // The directory that does not exist
    assert!(utils::path_exists(does_not_exist.join("bin/foo")));
}
