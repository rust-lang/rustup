use std::{collections::BTreeSet, io::Write};

use platforms::Platform;

#[test]
fn gen_known_triples() {
    let out_path = "src/dist/triple/known.rs";
    let existing = std::fs::read_to_string(out_path).unwrap();

    let (mut archs, mut oses, mut envs) = (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
    for (arch, os, env) in Platform::ALL.iter().map(|p| parse_triple(p.target_triple)) {
        archs.insert(arch);
        oses.insert(os);
        if !env.is_empty() {
            envs.insert(env);
        }
    }

    let expected = {
        let mut buf = String::new();

        buf.push_str("pub static LIST_ARCHS: &[&str] = &[\n");
        for arch in archs {
            buf.push_str(&format!("    \"{arch}\",\n"));
        }
        buf.push_str("];\n");

        buf.push_str("pub static LIST_OSES: &[&str] = &[\n");
        for os in oses {
            buf.push_str(&format!("    \"{os}\",\n"));
        }
        buf.push_str("];\n");

        buf.push_str("pub static LIST_ENVS: &[&str] = &[\n");
        for env in envs {
            buf.push_str(&format!("    \"{env}\",\n"));
        }
        buf.push_str("];\n");

        buf
    };

    if expected != existing {
        let mut tmp_file = tempfile::NamedTempFile::new().unwrap();
        tmp_file.write_all(expected.as_bytes()).unwrap();
        std::fs::rename(tmp_file.path(), out_path).unwrap();
        panic!(
            "outdated generated code detected at `{out_path}`, the file has been updated in place"
        );
    }
}

/// Parses the given triple into 3 parts (target architecture, OS and environment).
///
/// # Discussion
///
/// The current model of target triples in Rustup requires some non-code knowledge to correctly generate the list.
/// For example, the parsing results of two 2-dash triples can be different:
///
/// ```jsonc
/// { arch: aarch64, os: linux, env: android }
/// { arch: aarch64, os: unknown-freebsd}
/// ```
///
/// Thus, the following parsing scheme is used:
///
/// ```jsonc
/// // for `x-y`
/// { arch: x, os: y }
///
/// // special case for `x-y-w` where `y` is `none` or `linux`
/// // e.g. `thumbv4t-none-eabi`, `i686-linux-android`
/// // (should've been called `x-unknown-y-w`, but alas)
/// { arch: x, os: y, env: w }
///
/// // for `x-y-z`
/// { arch: x, os: y-z }
///
/// // for `x-y-z-w`
/// { arch: x, os: y-z, env: w }
/// ```
fn parse_triple(triple: &str) -> (&str, &str, &str) {
    match triple.split('-').collect::<Vec<_>>()[..] {
        [arch, os] => (arch, os, ""),
        [arch, os @ ("none" | "linux"), env] => (arch, os, env),
        [arch, _, _] => (arch, &triple[(arch.len() + 1)..], ""),
        [arch, _, _, env] => (
            arch,
            &triple[(arch.len() + 1)..(triple.len() - env.len() - 1)],
            env,
        ),
        _ => panic!(
            "Internal error while parsing target triple `{triple}`, please file an issue at https://github.com/rust-lang/rustup/issues"
        ),
    }
}
