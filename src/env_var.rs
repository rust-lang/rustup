use std::collections::VecDeque;
use std::env;
use std::path::PathBuf;
use std::process::Command;

use crate::process::Process;

pub const RUST_RECURSION_COUNT_MAX: u32 = 20;

pub(crate) fn insert_path(
    name: &str,
    prepend: Vec<PathBuf>,
    append: Option<PathBuf>,
    cmd: &mut Command,
    process: &Process,
) {
    let old_value = process.var_os(name);
    let mut parts = if let Some(v) = &old_value {
        let mut tail = env::split_paths(v).collect::<VecDeque<_>>();
        for path in prepend.into_iter().rev() {
            if !tail.contains(&path) {
                tail.push_front(path);
            }
        }
        tail
    } else {
        prepend.into()
    };

    if let Some(path) = append {
        if !parts.contains(&path) {
            parts.push_back(path);
        }
    }

    if let Ok(new_value) = env::join_paths(parts) {
        cmd.env(name, new_value);
    }
}

pub(crate) fn inc(name: &str, cmd: &mut Command, process: &Process) {
    let old_value = process
        .var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    cmd.env(name, (old_value + 1).to_string());
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::ffi::{OsStr, OsString};

    use super::*;
    #[cfg(windows)]
    use crate::cli::self_update::{RegistryGuard, USER_PATH};
    use crate::process::TestProcess;
    use crate::test::Env;

    #[test]
    fn prepend_unique_path() {
        let mut vars = HashMap::new();
        vars.env(
            "PATH",
            env::join_paths(["/home/a/.cargo/bin", "/home/b/.cargo/bin"].iter()).unwrap(),
        );
        let tp = TestProcess::with_vars(vars);
        #[cfg(windows)]
        let _path_guard = RegistryGuard::new(&USER_PATH).unwrap();
        let mut path_entries = vec![];
        let mut cmd = Command::new("test");

        let a = OsString::from("/home/a/.cargo/bin");
        let path_a = PathBuf::from(a);
        path_entries.push(path_a);

        let _a = OsString::from("/home/a/.cargo/bin");
        let _path_a = PathBuf::from(_a);
        path_entries.push(_path_a);

        let z = OsString::from("/home/z/.cargo/bin");
        let path_z = PathBuf::from(z);
        path_entries.push(path_z);

        insert_path("PATH", path_entries, None, &mut cmd, &tp.process);
        let envs: Vec<_> = cmd.get_envs().collect();

        assert_eq!(
            envs,
            &[(
                OsStr::new("PATH"),
                Some(
                    env::join_paths(
                        [
                            "/home/z/.cargo/bin",
                            "/home/a/.cargo/bin",
                            "/home/b/.cargo/bin"
                        ]
                        .iter()
                    )
                    .unwrap()
                    .as_os_str()
                )
            ),]
        );
    }

    #[test]
    fn append_unique_path() {
        let mut vars = HashMap::new();
        vars.env(
            "PATH",
            env::join_paths(["/home/a/.cargo/bin", "/home/b/.cargo/bin"].iter()).unwrap(),
        );
        let tp = TestProcess::with_vars(vars);
        #[cfg(windows)]
        let _path_guard = RegistryGuard::new(&USER_PATH).unwrap();

        #[track_caller]
        fn check(tp: &TestProcess, path_entries: Vec<PathBuf>, append: &str, expected: &[&str]) {
            let mut cmd = Command::new("test");

            insert_path(
                "PATH",
                path_entries,
                Some(append.into()),
                &mut cmd,
                &tp.process,
            );
            let envs: Vec<_> = cmd.get_envs().collect();

            assert_eq!(
                envs,
                &[(
                    OsStr::new("PATH"),
                    Some(env::join_paths(expected.iter()).unwrap().as_os_str())
                ),]
            );
        }

        check(
            &tp,
            Vec::new(),
            "/home/z/.cargo/bin",
            &[
                "/home/a/.cargo/bin",
                "/home/b/.cargo/bin",
                "/home/z/.cargo/bin",
            ],
        );
        check(
            &tp,
            Vec::new(),
            "/home/a/.cargo/bin",
            &["/home/a/.cargo/bin", "/home/b/.cargo/bin"],
        );
        check(
            &tp,
            Vec::new(),
            "/home/b/.cargo/bin",
            &["/home/a/.cargo/bin", "/home/b/.cargo/bin"],
        );
        check(
            &tp,
            Vec::from(["/home/c/.cargo/bin".into()]),
            "/home/c/.cargo/bin",
            &[
                "/home/c/.cargo/bin",
                "/home/a/.cargo/bin",
                "/home/b/.cargo/bin",
            ],
        );
        check(
            &tp,
            Vec::from(["/home/c/.cargo/bin".into()]),
            "/home/z/.cargo/bin",
            &[
                "/home/c/.cargo/bin",
                "/home/a/.cargo/bin",
                "/home/b/.cargo/bin",
                "/home/z/.cargo/bin",
            ],
        );
    }
}
