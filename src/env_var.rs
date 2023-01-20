use std::collections::{HashSet, VecDeque};
use std::env;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::Command;

use crate::process;

pub const RUST_RECURSION_COUNT_MAX: u32 = 20;

/// This can be removed when https://github.com/rust-lang/rust/issues/44434 is
/// stablised.
pub(crate) trait ProcessEnvs {
    fn env<K, V>(&mut self, key: K, val: V) -> &mut Self
    where
        Self: Sized,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>;
}

impl ProcessEnvs for Command {
    fn env<K, V>(&mut self, key: K, val: V) -> &mut Self
    where
        Self: Sized,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.env(key, val)
    }
}

#[allow(unused)]
fn append_path<E: ProcessEnvs>(name: &str, value: Vec<PathBuf>, cmd: &mut E) {
    let old_value = process().var_os(name);
    let mut parts: Vec<PathBuf>;
    if let Some(ref v) = old_value {
        let old_paths: Vec<PathBuf> = env::split_paths(v).collect::<Vec<_>>();
        parts = concat_uniq_paths(old_paths, value);
    } else {
        parts = value;
    }
    if let Ok(new_value) = env::join_paths(parts) {
        cmd.env(name, new_value);
    }
}

pub(crate) fn prepend_path<E: ProcessEnvs>(name: &str, prepend: Vec<PathBuf>, cmd: &mut E) {
    let old_value = process().var_os(name);
    let parts = if let Some(ref v) = old_value {
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

    if let Ok(new_value) = env::join_paths(parts) {
        cmd.env(name, new_value);
    }
}

pub(crate) fn inc(name: &str, cmd: &mut Command) {
    let old_value = process()
        .var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    cmd.env(name, (old_value + 1).to_string());
}

fn concat_uniq_paths(fst_paths: Vec<PathBuf>, snd_paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let deduped_fst_paths = dedupe_with_preserved_order(fst_paths);
    let deduped_snd_paths = dedupe_with_preserved_order(snd_paths);

    let vec_fst_paths: Vec<_> = deduped_fst_paths.into_iter().collect();

    let mut unified_paths;
    unified_paths = vec_fst_paths.clone();
    unified_paths.extend(
        deduped_snd_paths
            .into_iter()
            .filter(|v| !vec_fst_paths.contains(v))
            .collect::<Vec<_>>(),
    );

    unified_paths
}

fn dedupe_with_preserved_order(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut uniq_paths: Vec<PathBuf> = Vec::new();
    let mut seen_paths: HashSet<PathBuf> = HashSet::new();

    for path in &paths {
        if !seen_paths.contains(path) {
            seen_paths.insert(path.to_path_buf());
            uniq_paths.push(path.to_path_buf());
        }
    }

    uniq_paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::currentprocess;
    use crate::test::{with_saved_path, Env};

    use super::ProcessEnvs;
    use std::collections::HashMap;
    use std::ffi::{OsStr, OsString};

    #[derive(Default)]
    struct TestCommand {
        envs: HashMap<OsString, Option<OsString>>,
    }

    impl ProcessEnvs for TestCommand {
        fn env<K, V>(&mut self, key: K, val: V) -> &mut Self
        where
            Self: Sized,
            K: AsRef<OsStr>,
            V: AsRef<OsStr>,
        {
            self.envs
                .insert(key.as_ref().to_owned(), Some(val.as_ref().to_owned()));
            self
        }
    }

    #[test]
    fn deduplicate_and_concat_paths() {
        let mut old_paths = vec![];

        let z = OsString::from("/home/z/.cargo/bin");
        let path_z = PathBuf::from(z);
        old_paths.push(path_z);

        let a = OsString::from("/home/a/.cargo/bin");
        let path_a = PathBuf::from(a);
        old_paths.push(path_a);

        let _a = OsString::from("/home/a/.cargo/bin");
        let _path_a = PathBuf::from(_a);
        old_paths.push(_path_a);

        let mut new_paths = vec![];

        let n = OsString::from("/home/n/.cargo/bin");
        let path_n = PathBuf::from(n);
        new_paths.push(path_n);

        let g = OsString::from("/home/g/.cargo/bin");
        let path_g = PathBuf::from(g);
        new_paths.push(path_g);

        let _g = OsString::from("/home/g/.cargo/bin");
        let _path_g = PathBuf::from(_g);
        new_paths.push(_path_g);

        let _z = OsString::from("/home/z/.cargo/bin");
        let path_z = PathBuf::from(_z);
        old_paths.push(path_z);

        let mut unified_paths: Vec<PathBuf> = vec![];
        let zpath = OsString::from("/home/z/.cargo/bin");
        let _zpath = PathBuf::from(zpath);
        unified_paths.push(_zpath);
        let apath = OsString::from("/home/a/.cargo/bin");
        let _apath = PathBuf::from(apath);
        unified_paths.push(_apath);
        let npath = OsString::from("/home/n/.cargo/bin");
        let _npath = PathBuf::from(npath);
        unified_paths.push(_npath);
        let gpath = OsString::from("/home/g/.cargo/bin");
        let _gpath = PathBuf::from(gpath);
        unified_paths.push(_gpath);

        assert_eq!(concat_uniq_paths(old_paths, new_paths), unified_paths);
    }

    #[test]
    fn prepend_unique_path() {
        let mut vars = HashMap::new();
        vars.env(
            "PATH",
            env::join_paths(vec!["/home/a/.cargo/bin", "/home/b/.cargo/bin"].iter()).unwrap(),
        );
        let tp = Box::new(currentprocess::TestProcess {
            vars,
            ..Default::default()
        });
        with_saved_path(&|| {
            currentprocess::with(tp.clone(), || {
                let mut path_entries = vec![];
                let mut cmd = TestCommand::default();

                let a = OsString::from("/home/a/.cargo/bin");
                let path_a = PathBuf::from(a);
                path_entries.push(path_a);

                let _a = OsString::from("/home/a/.cargo/bin");
                let _path_a = PathBuf::from(_a);
                path_entries.push(_path_a);

                let z = OsString::from("/home/z/.cargo/bin");
                let path_z = PathBuf::from(z);
                path_entries.push(path_z);

                prepend_path("PATH", path_entries, &mut cmd);
                let envs: Vec<_> = cmd.envs.iter().collect();

                assert_eq!(
                    envs,
                    &[(
                        &OsString::from("PATH"),
                        &Some(
                            env::join_paths(
                                vec![
                                    "/home/z/.cargo/bin",
                                    "/home/a/.cargo/bin",
                                    "/home/b/.cargo/bin"
                                ]
                                .iter()
                            )
                            .unwrap()
                        )
                    ),]
                );
            });
        });
    }
}
