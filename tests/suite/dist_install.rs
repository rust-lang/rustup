use std::fs::File;
use std::io::Write;

use rustup::dist::component::Components;
use rustup::dist::component::Transaction;
use rustup::dist::component::{DirectoryPackage, Package};
use rustup::dist::prefix::InstallPrefix;
use rustup::dist::temp;
use rustup::dist::Notification;
use rustup::dist::DEFAULT_DIST_SERVER;
use rustup::process::TestProcess;
use rustup::utils::utils;

use rustup::test::mock::{MockComponentBuilder, MockFile, MockInstallerBuilder};

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
    let pkgdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();

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

    mock.build(pkgdir.path());

    let instdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let prefix = InstallPrefix::from(instdir.path().to_owned());

    let tmpdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let tmp_cx = temp::Context::new(
        tmpdir.path().to_owned(),
        DEFAULT_DIST_SERVER,
        Box::new(|_| ()),
    );
    let notify = |_: Notification<'_>| ();
    let tp = TestProcess::default();
    let tx = Transaction::new(prefix.clone(), &tmp_cx, &notify, &tp.process);

    let components = Components::open(prefix).unwrap();

    let pkg = DirectoryPackage::new(pkgdir.path().to_owned(), true).unwrap();

    let tx = pkg.install(&components, "mycomponent", None, tx).unwrap();
    tx.commit();

    assert!(utils::path_exists(instdir.path().join("bin/foo")));
    assert!(utils::path_exists(instdir.path().join("lib/bar")));
    assert!(utils::path_exists(instdir.path().join("doc/stuff/doc1")));
    assert!(utils::path_exists(instdir.path().join("doc/stuff/doc2")));

    assert!(components.find("mycomponent").unwrap().is_some());
}

#[test]
fn multiple_component_install() {
    let pkgdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();

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

    mock.build(pkgdir.path());

    let instdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let prefix = InstallPrefix::from(instdir.path().to_owned());

    let tmpdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let tmp_cx = temp::Context::new(
        tmpdir.path().to_owned(),
        DEFAULT_DIST_SERVER,
        Box::new(|_| ()),
    );
    let notify = |_: Notification<'_>| ();
    let tp = TestProcess::default();
    let tx = Transaction::new(prefix.clone(), &tmp_cx, &notify, &tp.process);

    let components = Components::open(prefix).unwrap();

    let pkg = DirectoryPackage::new(pkgdir.path().to_owned(), true).unwrap();

    let tx = pkg.install(&components, "mycomponent", None, tx).unwrap();
    let tx = pkg.install(&components, "mycomponent2", None, tx).unwrap();
    tx.commit();

    assert!(utils::path_exists(instdir.path().join("bin/foo")));
    assert!(utils::path_exists(instdir.path().join("lib/bar")));

    assert!(components.find("mycomponent").unwrap().is_some());
    assert!(components.find("mycomponent2").unwrap().is_some());
}

#[test]
fn uninstall() {
    let pkgdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();

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

    mock.build(pkgdir.path());

    let instdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let prefix = InstallPrefix::from(instdir.path().to_owned());

    let tmpdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let tmp_cx = temp::Context::new(
        tmpdir.path().to_owned(),
        DEFAULT_DIST_SERVER,
        Box::new(|_| ()),
    );
    let notify = |_: Notification<'_>| ();
    let tp = TestProcess::default();
    let tx = Transaction::new(prefix.clone(), &tmp_cx, &notify, &tp.process);

    let components = Components::open(prefix.clone()).unwrap();

    let pkg = DirectoryPackage::new(pkgdir.path().to_owned(), true).unwrap();

    let tx = pkg.install(&components, "mycomponent", None, tx).unwrap();
    let tx = pkg.install(&components, "mycomponent2", None, tx).unwrap();
    tx.commit();

    // Now uninstall
    let notify = |_: Notification<'_>| ();
    let tp = TestProcess::default();
    let mut tx = Transaction::new(prefix.clone(), &tmp_cx, &notify, &tp.process);
    for component in components.list().unwrap() {
        tx = component.uninstall(tx, &tp.process).unwrap();
    }
    tx.commit();

    assert!(!utils::path_exists(instdir.path().join("bin/foo")));
    assert!(!utils::path_exists(instdir.path().join("lib/bar")));
    assert!(!utils::path_exists(instdir.path().join("doc/stuff/doc1")));
    assert!(!utils::path_exists(instdir.path().join("doc/stuff/doc2")));
    assert!(!utils::path_exists(instdir.path().join("doc/stuff")));
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
    let pkgdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();

    let mock = MockInstallerBuilder {
        components: vec![MockComponentBuilder {
            name: "mycomponent".to_string(),
            files: vec![MockFile::new("bin/foo", b"foo")],
        }],
    };

    mock.build(pkgdir.path());

    let instdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let prefix = InstallPrefix::from(instdir.path().to_owned());

    let tmpdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let tmp_cx = temp::Context::new(
        tmpdir.path().to_owned(),
        DEFAULT_DIST_SERVER,
        Box::new(|_| ()),
    );
    let notify = |_: Notification<'_>| ();
    let tp = TestProcess::default();
    let tx = Transaction::new(prefix.clone(), &tmp_cx, &notify, &tp.process);

    let components = Components::open(prefix.clone()).unwrap();

    let pkg = DirectoryPackage::new(pkgdir.path().to_owned(), true).unwrap();

    let tx = pkg.install(&components, "mycomponent", None, tx).unwrap();
    tx.commit();

    // Write a bogus version to the component manifest directory
    utils::write_file("", &prefix.manifest_file("rust-installer-version"), "100\n").unwrap();

    // Can't open components now
    let e = Components::open(prefix).unwrap_err();
    assert_eq!(
        "unsupported metadata version in existing installation: 100",
        format!("{e}")
    );
}

// Installing to a prefix that doesn't exist creates it automatically
#[test]
fn install_to_prefix_that_does_not_exist() {
    let pkgdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();

    let mock = MockInstallerBuilder {
        components: vec![MockComponentBuilder {
            name: "mycomponent".to_string(),
            files: vec![MockFile::new("bin/foo", b"foo")],
        }],
    };

    mock.build(pkgdir.path());

    let instdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    // The directory that does not exist
    let does_not_exist = instdir.path().join("super_not_real");
    let prefix = InstallPrefix::from(does_not_exist.clone());

    let tmpdir = tempfile::Builder::new().prefix("rustup").tempdir().unwrap();
    let tmp_cx = temp::Context::new(
        tmpdir.path().to_owned(),
        DEFAULT_DIST_SERVER,
        Box::new(|_| ()),
    );
    let notify = |_: Notification<'_>| ();
    let tp = TestProcess::default();
    let tx = Transaction::new(prefix.clone(), &tmp_cx, &notify, &tp.process);

    let components = Components::open(prefix).unwrap();

    let pkg = DirectoryPackage::new(pkgdir.path().to_owned(), true).unwrap();

    let tx = pkg.install(&components, "mycomponent", None, tx).unwrap();
    tx.commit();

    assert!(utils::path_exists(does_not_exist.join("bin/foo")));
}
