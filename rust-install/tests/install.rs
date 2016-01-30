extern crate rust_install;
extern crate tempdir;

use rust_install::component::Components;
use rust_install::component::{DirectoryPackage, Package};
use rust_install::component::Transaction;
use rust_install::temp;
use rust_install::utils;
use rust_install::{InstallType, InstallPrefix, NotifyHandler};
use std::fs::{self, OpenOptions, File};
use std::io::Write;
use std::path::Path;
use tempdir::TempDir;

struct MockInstallerBuilder {
    components: Vec<(&'static str, Vec<Command>, Vec<(&'static str, &'static str)>)>,
}

enum Command {
    File(&'static str),
    Dir(&'static str)
}

impl MockInstallerBuilder {
    fn build(&self, path: &Path) {
        for &(name, ref commands, ref files) in &self.components {
            // Update the components file
            let comp_file = path.join("components");
            let ref mut comp_file = OpenOptions::new().write(true).append(true).create(true)
                .open(comp_file).unwrap();
            writeln!(comp_file, "{}", name).unwrap();

            // Create the component directory
            let component_dir = path.join(name);
            fs::create_dir(&component_dir).unwrap();

            // Create the component manifest
            let ref mut manifest = File::create(component_dir.join("manifest.in")).unwrap();
            for command in commands {
                match command {
                    &Command::File(f) => writeln!(manifest, "file:{}", f).unwrap(),
                    &Command::Dir(d) => writeln!(manifest, "dir:{}", d).unwrap(),
                }
            }

            // Create the component files
            for &(ref f_path, ref content) in files {
                let dir_path = component_dir.join(f_path);
                let dir_path = dir_path.parent().unwrap();
                fs::create_dir_all(dir_path).unwrap();

                let ref mut f = File::create(component_dir.join(f_path)).unwrap();
                f.write(content.as_bytes()).unwrap();
            }
        }

        let mut ver = File::create(path.join("rust-installer-version")).unwrap();
        writeln!(ver, "3").unwrap();
    }
}

// Just testing that the mocks work
#[test]
fn mock_smoke_test() {
    let tempdir = TempDir::new("multirust").unwrap();

    let mock = MockInstallerBuilder {
        components: vec![("mycomponent",
                          vec![Command::File("bin/foo"),
                               Command::File("lib/bar"),
                               Command::Dir("doc/stuff")],
                          vec![("bin/foo", "foo"),
                               ("lib/bar", "bar"),
                               ("doc/stuff/doc1", ""),
                               ("doc/stuff/doc2", "")]),
                         ("mycomponent2",
                          vec![Command::File("bin/quux")],
                          vec![("bin/quux", "quux")]
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
        components: vec![("mycomponent",
                          vec![Command::File("bin/foo")],
                          vec![("bin/foo", "foo")],
                          ),
                         ("mycomponent2",
                          vec![Command::File("bin/bar")],
                          vec![("bin/bar", "bar")]
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
        components: vec![("mycomponent",
                          vec![Command::File("bin/foo")],
                          vec![("bin/foo", "foo")])]
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
        components: vec![("mycomponent",
                          vec![Command::File("bin/foo"),
                               Command::File("lib/bar"),
                               Command::Dir("doc/stuff")],
                          vec![("bin/foo", "foo"),
                               ("lib/bar", "bar"),
                               ("doc/stuff/doc1", ""),
                               ("doc/stuff/doc2", "")])]
    };

    mock.build(pkgdir.path());

    let instdir = TempDir::new("multirust").unwrap();
    let prefix = InstallPrefix::from(instdir.path().to_owned(),
                                     InstallType::Owned);

    let notify = temp::SharedNotifyHandler::none();
    let tmpdir = TempDir::new("multirust").unwrap();
    let tmpcfg = temp::Cfg::new(tmpdir.path().to_owned(), notify);
    let notify = NotifyHandler::none();
    let tx = Transaction::new(prefix.clone(), &tmpcfg, notify);

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
        components: vec![("mycomponent",
                          vec![Command::File("bin/foo")],
                          vec![("bin/foo", "foo")]),
                         ("mycomponent2",
                          vec![Command::File("lib/bar")],
                          vec![("lib/bar", "bar")])]
    };

    mock.build(pkgdir.path());

    let instdir = TempDir::new("multirust").unwrap();
    let prefix = InstallPrefix::from(instdir.path().to_owned(),
                                     InstallType::Owned);

    let notify = temp::SharedNotifyHandler::none();
    let tmpdir = TempDir::new("multirust").unwrap();
    let tmpcfg = temp::Cfg::new(tmpdir.path().to_owned(), notify);
    let notify = NotifyHandler::none();
    let tx = Transaction::new(prefix.clone(), &tmpcfg, notify);

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

#[ignore] // FIXME windows
#[test]
fn uninstall() {
    let pkgdir = TempDir::new("multirust").unwrap();

    let mock = MockInstallerBuilder {
        components: vec![("mycomponent",
                          vec![Command::File("bin/foo"),
                               Command::File("lib/bar"),
                               Command::Dir("doc/stuff")],
                          vec![("bin/foo", "foo"),
                               ("lib/bar", "bar"),
                               ("doc/stuff/doc1", ""),
                               ("doc/stuff/doc2", "")]),
                         ("mycomponent2",
                          vec![Command::File("lib/quux")],
                          vec![("lib/quux", "quux")])]
    };

    mock.build(pkgdir.path());

    let instdir = TempDir::new("multirust").unwrap();
    let prefix = InstallPrefix::from(instdir.path().to_owned(),
                                     InstallType::Owned);

    let notify = temp::SharedNotifyHandler::none();
    let tmpdir = TempDir::new("multirust").unwrap();
    let tmpcfg = temp::Cfg::new(tmpdir.path().to_owned(), notify);
    let notify = NotifyHandler::none();
    let tx = Transaction::new(prefix.clone(), &tmpcfg, notify);

    let components = Components::open(prefix.clone()).unwrap();

    let pkg = DirectoryPackage::new(pkgdir.path().to_owned()).unwrap();

    let tx = pkg.install(&components, "mycomponent", None, tx).unwrap();
    let tx = pkg.install(&components, "mycomponent2", None, tx).unwrap();
    tx.commit();

    // Now uninstall
    let notify = NotifyHandler::none();
    let mut tx = Transaction::new(prefix.clone(), &tmpcfg, notify);
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

#[test]
fn component_bad_version() {
    let pkgdir = TempDir::new("multirust").unwrap();

    let mock = MockInstallerBuilder {
        components: vec![("mycomponent",
                          vec![Command::File("bin/foo")],
                          vec![("bin/foo", "foo")])]
    };

    mock.build(pkgdir.path());

    let instdir = TempDir::new("multirust").unwrap();
    let prefix = InstallPrefix::from(instdir.path().to_owned(),
                                     InstallType::Owned);

    let notify = temp::SharedNotifyHandler::none();
    let tmpdir = TempDir::new("multirust").unwrap();
    let tmpcfg = temp::Cfg::new(tmpdir.path().to_owned(), notify);
    let notify = NotifyHandler::none();
    let tx = Transaction::new(prefix.clone(), &tmpcfg, notify);

    let components = Components::open(prefix.clone()).unwrap();

    let pkg = DirectoryPackage::new(pkgdir.path().to_owned()).unwrap();

    let tx = pkg.install(&components, "mycomponent", None, tx).unwrap();
    tx.commit();

    // Write a bogus version to the component manifest directory
    utils::write_file("", &prefix.manifest_file("rust-installer-version"), "100\n").unwrap();

    // Can't open components now
    let e = Components::open(prefix.clone()).unwrap_err();
    if let rust_install::Error::BadInstalledMetadataVersion(_) = e { } else { panic!() }
}

// Directories should be 0755, normal files 0644, files that come
// from the bin/ directory 0755.
#[test]
#[cfg(unix)]
fn unix_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let pkgdir = TempDir::new("multirust").unwrap();

    let mock = MockInstallerBuilder {
        components: vec![("mycomponent",
                          vec![Command::File("bin/foo"),
                               Command::File("lib/bar"),
                               Command::Dir("doc/stuff")],
                          vec![("bin/foo", "foo"),
                               ("lib/bar", "bar"),
                               ("doc/stuff/doc1", ""),
                               ("doc/stuff/morestuff/doc2", "")])]
    };

    mock.build(pkgdir.path());

    let instdir = TempDir::new("multirust").unwrap();
    let prefix = InstallPrefix::from(instdir.path().to_owned(),
                                     InstallType::Owned);

    let notify = temp::SharedNotifyHandler::none();
    let tmpdir = TempDir::new("multirust").unwrap();
    let tmpcfg = temp::Cfg::new(tmpdir.path().to_owned(), notify);
    let notify = NotifyHandler::none();
    let tx = Transaction::new(prefix.clone(), &tmpcfg, notify);

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
