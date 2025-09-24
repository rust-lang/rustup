use std::fs;
use std::io::Write;
use std::path::PathBuf;

use crate::errors::RustupError;
use crate::test::DistContext;
use crate::utils::{self, raw as utils_raw};

#[test]
fn add_file() {
    let cx = DistContext::new(None).unwrap();
    let mut tx = cx.transaction();

    let mut file = tx.add_file("c", PathBuf::from("foo/bar")).unwrap();
    write!(file, "test").unwrap();

    tx.commit();
    drop(file);

    assert_eq!(
        fs::read_to_string(cx.prefix.path().join("foo/bar")).unwrap(),
        "test"
    );
}

#[test]
fn add_file_then_rollback() {
    let cx = DistContext::new(None).unwrap();
    let mut tx = cx.transaction();

    tx.add_file("c", PathBuf::from("foo/bar")).unwrap();
    drop(tx);

    assert!(!utils::is_file(cx.prefix.path().join("foo/bar")));
}

#[test]
fn add_file_that_exists() {
    let cx = DistContext::new(None).unwrap();
    let mut tx = cx.transaction();

    fs::create_dir_all(cx.prefix.path().join("foo")).unwrap();
    utils::write_file("", &cx.prefix.path().join("foo/bar"), "").unwrap();

    let err = tx.add_file("c", PathBuf::from("foo/bar")).unwrap_err();

    match err.downcast_ref::<RustupError>() {
        Some(RustupError::ComponentConflict { name, path }) => {
            assert_eq!(name, "c");
            assert_eq!(path.clone(), PathBuf::from("foo/bar"));
        }
        _ => panic!(),
    }
}

#[test]
fn copy_file() {
    let cx = DistContext::new(None).unwrap();
    let mut tx = cx.transaction();

    let srcpath = cx.pkg_dir.path().join("bar");
    utils::write_file("", &srcpath, "").unwrap();

    tx.copy_file("c", PathBuf::from("foo/bar"), &srcpath)
        .unwrap();
    tx.commit();

    assert!(utils::is_file(cx.prefix.path().join("foo/bar")));
}

#[test]
fn copy_file_then_rollback() {
    let cx = DistContext::new(None).unwrap();
    let mut tx = cx.transaction();

    let srcpath = cx.pkg_dir.path().join("bar");
    utils::write_file("", &srcpath, "").unwrap();

    tx.copy_file("c", PathBuf::from("foo/bar"), &srcpath)
        .unwrap();
    drop(tx);

    assert!(!utils::is_file(cx.prefix.path().join("foo/bar")));
}

#[test]
fn copy_file_that_exists() {
    let cx = DistContext::new(None).unwrap();
    let mut tx = cx.transaction();

    let srcpath = cx.pkg_dir.path().join("bar");
    utils::write_file("", &srcpath, "").unwrap();

    fs::create_dir_all(cx.prefix.path().join("foo")).unwrap();
    utils::write_file("", &cx.prefix.path().join("foo/bar"), "").unwrap();

    let err = tx
        .copy_file("c", PathBuf::from("foo/bar"), &srcpath)
        .unwrap_err();

    match err.downcast_ref::<RustupError>() {
        Some(RustupError::ComponentConflict { name, path }) => {
            assert_eq!(name, "c");
            assert_eq!(path.clone(), PathBuf::from("foo/bar"));
        }
        _ => panic!(),
    }
}

#[test]
fn copy_dir() {
    let cx = DistContext::new(None).unwrap();
    let mut tx = cx.transaction();

    let srcpath1 = cx.pkg_dir.path().join("foo");
    let srcpath2 = cx.pkg_dir.path().join("bar/baz");
    let srcpath3 = cx.pkg_dir.path().join("bar/qux/tickle");
    utils::write_file("", &srcpath1, "").unwrap();
    fs::create_dir_all(srcpath2.parent().unwrap()).unwrap();
    utils::write_file("", &srcpath2, "").unwrap();
    fs::create_dir_all(srcpath3.parent().unwrap()).unwrap();
    utils::write_file("", &srcpath3, "").unwrap();

    tx.copy_dir("c", PathBuf::from("a"), cx.pkg_dir.path())
        .unwrap();
    tx.commit();

    assert!(utils::is_file(cx.prefix.path().join("a/foo")));
    assert!(utils::is_file(cx.prefix.path().join("a/bar/baz")));
    assert!(utils::is_file(cx.prefix.path().join("a/bar/qux/tickle")));
}

#[test]
fn copy_dir_then_rollback() {
    let cx = DistContext::new(None).unwrap();
    let mut tx = cx.transaction();

    let srcpath1 = cx.pkg_dir.path().join("foo");
    let srcpath2 = cx.pkg_dir.path().join("bar/baz");
    let srcpath3 = cx.pkg_dir.path().join("bar/qux/tickle");
    utils::write_file("", &srcpath1, "").unwrap();
    fs::create_dir_all(srcpath2.parent().unwrap()).unwrap();
    utils::write_file("", &srcpath2, "").unwrap();
    fs::create_dir_all(srcpath3.parent().unwrap()).unwrap();
    utils::write_file("", &srcpath3, "").unwrap();

    tx.copy_dir("c", PathBuf::from("a"), cx.pkg_dir.path())
        .unwrap();
    drop(tx);

    assert!(!utils::is_file(cx.prefix.path().join("a/foo")));
    assert!(!utils::is_file(cx.prefix.path().join("a/bar/baz")));
    assert!(!utils::is_file(cx.prefix.path().join("a/bar/qux/tickle")));
}

#[test]
fn copy_dir_that_exists() {
    let cx = DistContext::new(None).unwrap();
    let mut tx = cx.transaction();

    fs::create_dir_all(cx.prefix.path().join("a")).unwrap();

    let err = tx
        .copy_dir("c", PathBuf::from("a"), cx.pkg_dir.path())
        .unwrap_err();

    match err.downcast_ref::<RustupError>() {
        Some(RustupError::ComponentConflict { name, path }) => {
            assert_eq!(name, "c");
            assert_eq!(path.clone(), PathBuf::from("a"));
        }
        _ => panic!(),
    }
}

#[test]
fn remove_file() {
    let cx = DistContext::new(None).unwrap();
    let mut tx = cx.transaction();

    let filepath = cx.prefix.path().join("foo");
    utils::write_file("", &filepath, "").unwrap();

    tx.remove_file("c", PathBuf::from("foo")).unwrap();
    tx.commit();

    assert!(!utils::is_file(filepath));
}

#[test]
fn remove_file_then_rollback() {
    let cx = DistContext::new(None).unwrap();
    let mut tx = cx.transaction();

    let filepath = cx.prefix.path().join("foo");
    utils::write_file("", &filepath, "").unwrap();

    tx.remove_file("c", PathBuf::from("foo")).unwrap();
    drop(tx);

    assert!(utils::is_file(filepath));
}

#[test]
fn remove_file_that_not_exists() {
    let cx = DistContext::new(None).unwrap();
    let mut tx = cx.transaction();

    let err = tx.remove_file("c", PathBuf::from("foo")).unwrap_err();

    match err.downcast_ref::<RustupError>() {
        Some(RustupError::ComponentMissingFile { name, path }) => {
            assert_eq!(name, "c");
            assert_eq!(path.clone(), PathBuf::from("foo"));
        }
        _ => panic!(),
    }
}

#[test]
fn remove_dir() {
    let cx = DistContext::new(None).unwrap();
    let mut tx = cx.transaction();

    let filepath = cx.prefix.path().join("foo/bar");
    fs::create_dir_all(filepath.parent().unwrap()).unwrap();
    utils::write_file("", &filepath, "").unwrap();

    tx.remove_dir("c", PathBuf::from("foo")).unwrap();
    tx.commit();

    assert!(!utils::path_exists(filepath.parent().unwrap()));
}

#[test]
fn remove_dir_then_rollback() {
    let cx = DistContext::new(None).unwrap();
    let mut tx = cx.transaction();

    let filepath = cx.prefix.path().join("foo/bar");
    fs::create_dir_all(filepath.parent().unwrap()).unwrap();
    utils::write_file("", &filepath, "").unwrap();

    tx.remove_dir("c", PathBuf::from("foo")).unwrap();
    drop(tx);

    assert!(utils::path_exists(filepath.parent().unwrap()));
}

#[test]
fn remove_dir_that_not_exists() {
    let cx = DistContext::new(None).unwrap();
    let mut tx = cx.transaction();

    let err = tx.remove_dir("c", PathBuf::from("foo")).unwrap_err();

    match err.downcast_ref::<RustupError>() {
        Some(RustupError::ComponentMissingDir { name, path }) => {
            assert_eq!(name, "c");
            assert_eq!(path.clone(), PathBuf::from("foo"));
        }
        _ => panic!(),
    }
}

#[test]
fn write_file() {
    let cx = DistContext::new(None).unwrap();
    let mut tx = cx.transaction();

    let content = "hi".to_string();
    tx.write_file("c", PathBuf::from("foo/bar"), content.clone())
        .unwrap();
    tx.commit();

    let path = cx.prefix.path().join("foo/bar");
    assert!(utils::is_file(&path));
    let file_content = fs::read_to_string(&path).unwrap();
    assert_eq!(content, file_content);
}

#[test]
fn write_file_then_rollback() {
    let cx = DistContext::new(None).unwrap();
    let mut tx = cx.transaction();

    let content = "hi".to_string();
    tx.write_file("c", PathBuf::from("foo/bar"), content)
        .unwrap();
    drop(tx);

    assert!(!utils::is_file(cx.prefix.path().join("foo/bar")));
}

#[test]
fn write_file_that_exists() {
    let cx = DistContext::new(None).unwrap();
    let mut tx = cx.transaction();

    let content = "hi".to_string();
    utils_raw::write_file(&cx.prefix.path().join("a"), &content).unwrap();
    let err = tx.write_file("c", PathBuf::from("a"), content).unwrap_err();

    match err.downcast_ref::<RustupError>() {
        Some(RustupError::ComponentConflict { name, path }) => {
            assert_eq!(name, "c");
            assert_eq!(path.clone(), PathBuf::from("a"));
        }
        _ => panic!(),
    }
}

// If the file does not exist, then the path to it is created,
// but the file is not.
#[test]
fn modify_file_that_not_exists() {
    let cx = DistContext::new(None).unwrap();
    let mut tx = cx.transaction();

    tx.modify_file(PathBuf::from("foo/bar")).unwrap();
    tx.commit();

    assert!(utils::path_exists(cx.prefix.path().join("foo")));
    assert!(!utils::path_exists(cx.prefix.path().join("foo/bar")));
}

// If the file does exist, then it's just backed up
#[test]
fn modify_file_that_exists() {
    let cx = DistContext::new(None).unwrap();
    let mut tx = cx.transaction();

    let path = cx.prefix.path().join("foo");
    utils_raw::write_file(&path, "wow").unwrap();
    tx.modify_file(PathBuf::from("foo")).unwrap();
    tx.commit();

    assert_eq!(fs::read_to_string(&path).unwrap(), "wow");
}

#[test]
fn modify_file_that_not_exists_then_rollback() {
    let cx = DistContext::new(None).unwrap();
    let mut tx = cx.transaction();

    tx.modify_file(PathBuf::from("foo/bar")).unwrap();
    drop(tx);

    assert!(!utils::path_exists(cx.prefix.path().join("foo/bar")));
}

#[test]
fn modify_file_that_exists_then_rollback() {
    let cx = DistContext::new(None).unwrap();
    let mut tx = cx.transaction();

    let path = cx.prefix.path().join("foo");
    utils_raw::write_file(&path, "wow").unwrap();
    tx.modify_file(PathBuf::from("foo")).unwrap();
    utils_raw::write_file(&path, "eww").unwrap();
    drop(tx);

    assert_eq!(fs::read_to_string(&path).unwrap(), "wow");
}

// This is testing that the backup scheme is smart enough not
// to overwrite the earliest backup.
#[test]
fn modify_twice_then_rollback() {
    let cx = DistContext::new(None).unwrap();
    let mut tx = cx.transaction();

    let path = cx.prefix.path().join("foo");
    utils_raw::write_file(&path, "wow").unwrap();
    tx.modify_file(PathBuf::from("foo")).unwrap();
    utils_raw::write_file(&path, "eww").unwrap();
    tx.modify_file(PathBuf::from("foo")).unwrap();
    utils_raw::write_file(&path, "ewww").unwrap();
    drop(tx);

    assert_eq!(fs::read_to_string(&path).unwrap(), "wow");
}

fn do_multiple_op_transaction(rollback: bool) {
    let cx = DistContext::new(None).unwrap();
    let mut tx = cx.transaction();

    // copy_file
    let relpath1 = PathBuf::from("bin/rustc");
    let relpath2 = PathBuf::from("bin/cargo");
    // copy_dir
    let relpath4 = PathBuf::from("doc/html/index.html");
    // modify_file
    let relpath5 = PathBuf::from("lib/rustlib/components");
    // write_file
    let relpath6 = PathBuf::from("lib/rustlib/rustc-manifest.in");
    // remove_file
    let relpath7 = PathBuf::from("bin/oldrustc");
    // remove_dir
    let relpath8 = PathBuf::from("olddoc/htm/index.html");

    let path1 = cx.prefix.path().join(&relpath1);
    let path2 = cx.prefix.path().join(&relpath2);
    let path4 = cx.prefix.path().join(&relpath4);
    let path5 = cx.prefix.path().join(&relpath5);
    let path6 = cx.prefix.path().join(&relpath6);
    let path7 = cx.prefix.path().join(&relpath7);
    let path8 = cx.prefix.path().join(relpath8);

    let srcpath1 = cx.pkg_dir.path().join(&relpath1);
    fs::create_dir_all(srcpath1.parent().unwrap()).unwrap();
    utils_raw::write_file(&srcpath1, "").unwrap();
    tx.copy_file("", relpath1, &srcpath1).unwrap();

    let srcpath2 = cx.pkg_dir.path().join(&relpath2);
    utils_raw::write_file(&srcpath2, "").unwrap();
    tx.copy_file("", relpath2, &srcpath2).unwrap();

    let srcpath4 = cx.pkg_dir.path().join(&relpath4);
    fs::create_dir_all(srcpath4.parent().unwrap()).unwrap();
    utils_raw::write_file(&srcpath4, "").unwrap();
    tx.copy_dir("", PathBuf::from("doc"), &cx.pkg_dir.path().join("doc"))
        .unwrap();

    tx.modify_file(relpath5).unwrap();
    utils_raw::write_file(&path5, "").unwrap();

    tx.write_file("", relpath6, "".to_string()).unwrap();

    fs::create_dir_all(path7.parent().unwrap()).unwrap();
    utils_raw::write_file(&path7, "").unwrap();
    tx.remove_file("", relpath7).unwrap();

    fs::create_dir_all(path8.parent().unwrap()).unwrap();
    utils_raw::write_file(&path8, "").unwrap();
    tx.remove_dir("", PathBuf::from("olddoc")).unwrap();

    if !rollback {
        tx.commit();

        assert!(utils::path_exists(path1));
        assert!(utils::path_exists(path2));
        assert!(utils::path_exists(path4));
        assert!(utils::path_exists(path5));
        assert!(utils::path_exists(path6));
        assert!(!utils::path_exists(path7));
        assert!(!utils::path_exists(path8));
    } else {
        drop(tx);

        assert!(!utils::path_exists(path1));
        assert!(!utils::path_exists(path2));
        assert!(!utils::path_exists(path4));
        assert!(!utils::path_exists(path5));
        assert!(!utils::path_exists(path6));
        assert!(utils::path_exists(path7));
        assert!(utils::path_exists(path8));
    }
}

#[test]
fn multiple_op_transaction() {
    do_multiple_op_transaction(false);
}

#[test]
fn multiple_op_transaction_then_rollback() {
    do_multiple_op_transaction(true);
}

// Even if one step fails to rollback, rollback should
// continue to rollback other steps.
#[test]
fn rollback_failure_keeps_going() {
    let cx = DistContext::new(None).unwrap();
    let mut tx = cx.transaction();

    write!(tx.add_file("", PathBuf::from("foo")).unwrap(), "").unwrap();
    write!(tx.add_file("", PathBuf::from("bar")).unwrap(), "").unwrap();
    write!(tx.add_file("", PathBuf::from("baz")).unwrap(), "").unwrap();

    fs::remove_file(cx.prefix.path().join("bar")).unwrap();

    drop(tx);

    assert!(!utils::path_exists(cx.prefix.path().join("foo")));
    assert!(!utils::path_exists(cx.prefix.path().join("baz")));
}

// Test that when a transaction creates intermediate directories that
// they are deleted during rollback.
#[test]
#[ignore]
fn intermediate_dir_rollback() {}
