extern crate rustup_dist;
extern crate rustup_utils;
extern crate rustup_mock;
extern crate tempdir;

use rustup_dist::component::Components;
use rustup_dist::component::{DirectoryPackage, Package};
use rustup_dist::component::Transaction;
use rustup_dist::dist::DEFAULT_DIST_SERVER;
use rustup_dist::temp;
use rustup_dist::ErrorKind;
use rustup_dist::Notification;
use rustup_utils::utils;
use rustup_dist::prefix::InstallPrefix;
use std::fs::File;
use std::io::Write;
use tempdir::TempDir;
use rustup_mock::{MockInstallerBuilder, MockCommand};

// Just testing that the mocks work
#[test]
fn mock_smoke_test() {
    let tempdir = TempDir::new("multirust").unwrap();

    let mock = MockInstallerBuilder {
        components: vec![("mycomponent".to_string(),
                          vec![MockCommand::File("bin/foo".to_string()),
                               MockCommand::File("lib/bar".to_string()),
                               MockCommand::Dir("doc/stuff".to_string())],
                          vec![("bin/foo".to_string(), "foo".into()),
                               ("lib/bar".to_string(), "bar".into()),
                               ("doc/stuff/doc1".to_string(), "".into()),
                               ("doc/stuff/doc2".to_string(), "".into())]),
                         ("mycomponent2".to_string(),
                          vec![MockCommand::File("bin/quux".to_string())],
                          vec![("bin/quux".to_string(), "quux".into())]
                          )]
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
    let tempdir = TempDir::new("multirust").unwrap();

    let mock = MockInstallerBuilder {
        components: vec![("mycomponent".to_string(),
                          vec![MockCommand::File("bin/foo".to_string())],
                          vec![("bin/foo".to_string(), "foo".into())],
                          ),
                         ("mycomponent2".to_string(),
                          vec![MockCommand::File("bin/bar".to_string())],
                          vec![("bin/bar".to_string(), "bar".into())]
                          )]
    };

    mock.build(tempdir.path());

    let package = DirectoryPackage::new(tempdir.path().to_owned()).unwrap();
    assert!(package.contains("mycomponent", None));
    assert!(package.contains("mycomponent2", None));
}

#[test]
fn package_bad_version() {
    let tempdir = TempDir::new("multirust").unwrap();

    let mock = MockInstallerBuilder {
        components: vec![("mycomponent".to_string(),
                          vec![MockCommand::File("bin/foo".to_string())],
                          vec![("bin/foo".to_string(), "foo".into())])]
    };

    mock.build(tempdir.path());

    let mut ver = File::create(tempdir.path().join("rust-installer-version")).unwrap();
    writeln!(ver, "100").unwrap();

    assert!(DirectoryPackage::new(tempdir.path().to_owned()).is_err());
}

#[test]
fn basic_install() {
    let pkgdir = TempDir::new("multirust").unwrap();

    let mock = MockInstallerBuilder {
        components: vec![("mycomponent".to_string(),
                          vec![MockCommand::File("bin/foo".to_string()),
                               MockCommand::File("lib/bar".to_string()),
                               MockCommand::Dir("doc/stuff".to_string())],
                          vec![("bin/foo".to_string(), "foo".into()),
                               ("lib/bar".to_string(), "bar".into()),
                               ("doc/stuff/doc1".to_string(), "".into()),
                               ("doc/stuff/doc2".to_string(), "".into())])]
    };

    mock.build(pkgdir.path());

    let instdir = TempDir::new("multirust").unwrap();
    let prefix = InstallPrefix::from(instdir.path().to_owned());

    let tmpdir = TempDir::new("multirust").unwrap();
    let tmpcfg = temp::Cfg::new(tmpdir.path().to_owned(), DEFAULT_DIST_SERVER, Box::new(|_| ()));
    let notify = |_: Notification| ();
    let tx = Transaction::new(prefix.clone(), &tmpcfg, &notify);

    let components = Components::open(prefix.clone()).unwrap();

    let pkg = DirectoryPackage::new(pkgdir.path().to_owned()).unwrap();

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
    let pkgdir = TempDir::new("multirust").unwrap();

    let mock = MockInstallerBuilder {
        components: vec![("mycomponent".to_string(),
                          vec![MockCommand::File("bin/foo".to_string())],
                          vec![("bin/foo".to_string(), "foo".into())]),
                         ("mycomponent2".to_string(),
                          vec![MockCommand::File("lib/bar".to_string())],
                          vec![("lib/bar".to_string(), "bar".into())])]
    };

    mock.build(pkgdir.path());

    let instdir = TempDir::new("multirust").unwrap();
    let prefix = InstallPrefix::from(instdir.path().to_owned());

    let tmpdir = TempDir::new("multirust").unwrap();
    let tmpcfg = temp::Cfg::new(tmpdir.path().to_owned(), DEFAULT_DIST_SERVER, Box::new(|_| ()));
    let notify = |_: Notification| ();
    let tx = Transaction::new(prefix.clone(), &tmpcfg, &notify);

    let components = Components::open(prefix.clone()).unwrap();

    let pkg = DirectoryPackage::new(pkgdir.path().to_owned()).unwrap();

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
    let pkgdir = TempDir::new("multirust").unwrap();

    let mock = MockInstallerBuilder {
        components: vec![("mycomponent".to_string(),
                          vec![MockCommand::File("bin/foo".to_string()),
                               MockCommand::File("lib/bar".to_string()),
                               MockCommand::Dir("doc/stuff".to_string())],
                          vec![("bin/foo".to_string(), "foo".into()),
                               ("lib/bar".to_string(), "bar".into()),
                               ("doc/stuff/doc1".to_string(), "".into()),
                               ("doc/stuff/doc2".to_string(), "".into())]),
                         ("mycomponent2".to_string(),
                          vec![MockCommand::File("lib/quux".to_string())],
                          vec![("lib/quux".to_string(), "quux".into())])]
    };

    mock.build(pkgdir.path());

    let instdir = TempDir::new("multirust").unwrap();
    let prefix = InstallPrefix::from(instdir.path().to_owned());

    let tmpdir = TempDir::new("multirust").unwrap();
    let tmpcfg = temp::Cfg::new(tmpdir.path().to_owned(), DEFAULT_DIST_SERVER, Box::new(|_| ()));
    let notify = |_: Notification| ();
    let tx = Transaction::new(prefix.clone(), &tmpcfg, &notify);

    let components = Components::open(prefix.clone()).unwrap();

    let pkg = DirectoryPackage::new(pkgdir.path().to_owned()).unwrap();

    let tx = pkg.install(&components, "mycomponent", None, tx).unwrap();
    let tx = pkg.install(&components, "mycomponent2", None, tx).unwrap();
    tx.commit();

    // Now uninstall
    let notify = |_: Notification| ();
    let mut tx = Transaction::new(prefix.clone(), &tmpcfg, &notify);
    for component in components.list().unwrap() {
        tx = component.uninstall(tx).unwrap();
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
    let pkgdir = TempDir::new("multirust").unwrap();

    let mock = MockInstallerBuilder {
        components: vec![("mycomponent".to_string(),
                          vec![MockCommand::File("bin/foo".to_string())],
                          vec![("bin/foo".to_string(), "foo".into())])]
    };

    mock.build(pkgdir.path());

    let instdir = TempDir::new("multirust").unwrap();
    let prefix = InstallPrefix::from(instdir.path().to_owned());

    let tmpdir = TempDir::new("multirust").unwrap();
    let tmpcfg = temp::Cfg::new(tmpdir.path().to_owned(), DEFAULT_DIST_SERVER, Box::new(|_| ()));
    let notify = |_: Notification| ();
    let tx = Transaction::new(prefix.clone(), &tmpcfg, &notify);

    let components = Components::open(prefix.clone()).unwrap();

    let pkg = DirectoryPackage::new(pkgdir.path().to_owned()).unwrap();

    let tx = pkg.install(&components, "mycomponent", None, tx).unwrap();
    tx.commit();

    // Write a bogus version to the component manifest directory
    utils::write_file("", &prefix.manifest_file("rust-installer-version"), "100\n").unwrap();

    // Can't open components now
    let e = Components::open(prefix.clone()).unwrap_err();
    if let ErrorKind::BadInstalledMetadataVersion(_) = *e.kind() { } else { panic!() }
}

// Directories should be 0755, normal files 0644, files that come
// from the bin/ directory 0755.
#[test]
#[cfg(unix)]
fn unix_permissions() {
    use std::os::unix::fs::PermissionsExt;
    use std::fs;

    let pkgdir = TempDir::new("multirust").unwrap();

    let mock = MockInstallerBuilder {
        components: vec![("mycomponent".to_string(),
                          vec![MockCommand::File("bin/foo".to_string()),
                               MockCommand::File("lib/bar".to_string()),
                               MockCommand::Dir("doc/stuff".to_string())],
                          vec![("bin/foo".to_string(), "foo".into()),
                               ("lib/bar".to_string(), "bar".into()),
                               ("doc/stuff/doc1".to_string(), "".into()),
                               ("doc/stuff/morestuff/doc2".to_string(), "".into())])]
    };

    mock.build(pkgdir.path());

    let instdir = TempDir::new("multirust").unwrap();
    let prefix = InstallPrefix::from(instdir.path().to_owned());

    let tmpdir = TempDir::new("multirust").unwrap();
    let tmpcfg = temp::Cfg::new(tmpdir.path().to_owned(), DEFAULT_DIST_SERVER, Box::new(|_| ()));
    let notify = |_: Notification| ();
    let tx = Transaction::new(prefix.clone(), &tmpcfg, &notify);

    let components = Components::open(prefix.clone()).unwrap();

    let pkg = DirectoryPackage::new(pkgdir.path().to_owned()).unwrap();

    let tx = pkg.install(&components, "mycomponent", None, tx).unwrap();
    tx.commit();

    let m = fs::metadata(instdir.path().join("bin/foo")).unwrap().permissions().mode();
    assert_eq!(m, 0o755);
    let m = fs::metadata(instdir.path().join("lib/bar")).unwrap().permissions().mode();
    assert_eq!(m, 0o644);
    let m = fs::metadata(instdir.path().join("doc/stuff/")).unwrap().permissions().mode();
    assert_eq!(m, 0o755);
    let m = fs::metadata(instdir.path().join("doc/stuff/doc1")).unwrap().permissions().mode();
    assert_eq!(m, 0o644);
    let m = fs::metadata(instdir.path().join("doc/stuff/morestuff")).unwrap().permissions().mode();
    assert_eq!(m, 0o755);
    let m = fs::metadata(instdir.path().join("doc/stuff/morestuff/doc2")).unwrap().permissions().mode();
    assert_eq!(m, 0o644);
}

// Installing to a prefix that doesn't exist creates it automatically
#[test]
fn install_to_prefix_that_does_not_exist() {
    let pkgdir = TempDir::new("multirust").unwrap();

    let mock = MockInstallerBuilder {
        components: vec![("mycomponent".to_string(),
                          vec![MockCommand::File("bin/foo".to_string())],
                          vec![("bin/foo".to_string(), "foo".into())])]
    };

    mock.build(pkgdir.path());

    let instdir = TempDir::new("multirust").unwrap();
    // The directory that does not exist
    let does_not_exist = instdir.path().join("super_not_real");
    let prefix = InstallPrefix::from(does_not_exist.clone());

    let tmpdir = TempDir::new("multirust").unwrap();
    let tmpcfg = temp::Cfg::new(tmpdir.path().to_owned(), DEFAULT_DIST_SERVER, Box::new(|_| ()));
    let notify = |_: Notification| ();
    let tx = Transaction::new(prefix.clone(), &tmpcfg, &notify);

    let components = Components::open(prefix.clone()).unwrap();

    let pkg = DirectoryPackage::new(pkgdir.path().to_owned()).unwrap();

    let tx = pkg.install(&components, "mycomponent", None, tx).unwrap();
    tx.commit();

    assert!(utils::path_exists(does_not_exist.join("bin/foo")));
}
