use std::{env, fs, process::Command};

use anyhow::{Context, Result, bail, ensure};

fn main() -> Result<()> {
    let args: Vec<_> = env::args().skip(1).collect();
    let usage = "usage: cargo xtask bump-init-sh <remote>/<main|release/X.Y>";
    match args.first().map(String::as_str) {
        Some("bump-init-sh") => {
            ensure!(args.len() == 2, "{usage}");
            bump_init_sh(&args[1])
        }
        Some(command) => bail!("unknown xtask command {command:?}\n{usage}"),
        None => bail!("{usage}"),
    }
}

fn bump_init_sh(release_branch: &str) -> Result<()> {
    ensure!(
        is_release_branch(release_branch),
        "expected a remote-tracking main or release/X.Y branch"
    );

    // Resolve the source once so all metadata comes from the same commit, not HEAD.
    let source = git(&[
        "show",
        "--no-patch",
        "--format=%H %cs",
        &format!("refs/remotes/{release_branch}"),
        "--",
    ])?;
    let (commit, commit_date) = source
        .trim()
        .split_once(' ')
        .context("missing commit date")?;
    let manifest: toml::Value = toml::from_str(&git(&["show", &format!("{commit}:Cargo.toml")])?)?;
    let version = manifest
        .get("package")
        .and_then(|package| package.get("version"))
        .and_then(toml::Value::as_str)
        .context("missing or non-string package.version in the release source")?;
    ensure!(
        is_dotted_numbers(version, 3),
        "expected package.version in the release source to be X.Y.Z"
    );

    let contents = fs::read_to_string("rustup-init.sh")?;
    let error = "expected exactly one rustup-init version/SHA/date line in rustup-init.sh";
    let metadata = format!("rustup-init {version} ({} {commit_date})", &commit[..9]);
    let mut updated = String::with_capacity(contents.len());
    let mut found = false;
    for line in contents.split_inclusive('\n') {
        if line.starts_with("rustup-init ") {
            ensure!(!found, error);
            found = true;
            let existing = line.strip_suffix('\n').unwrap_or(line);
            ensure!(is_metadata_line(existing), error);
            updated.push_str(&metadata);
            if line.ends_with('\n') {
                updated.push('\n');
            }
        } else {
            updated.push_str(line);
        }
    }
    ensure!(found, error);

    if updated != contents {
        fs::write("rustup-init.sh", updated)?;
    }
    println!("{release_branch} ({commit}): {metadata}");

    let status = Command::new("cargo")
        .args(["test", "--features=test", "--", "cli_rustup_init_ui"])
        .env("SNAPSHOTS", "overwrite")
        .status()
        .context("cannot run rustup-init.sh help snapshot update")?;
    ensure!(
        status.success(),
        "rustup-init.sh help snapshot update failed: {status}"
    );
    if cfg!(windows) {
        eprintln!("warning: rustup-init.sh help snapshot is not updated on Windows");
    }
    Ok(())
}

fn is_release_branch(value: &str) -> bool {
    value.split_once('/').is_some_and(|(remote, branch)| {
        !remote.is_empty()
            && (branch == "main"
                || branch
                    .strip_prefix("release/")
                    .is_some_and(|version| is_dotted_numbers(version, 2)))
    })
}

fn is_ascii_digits(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn is_dotted_numbers(value: &str, count: usize) -> bool {
    let mut parts = value.split('.');
    (0..count).all(|_| parts.next().is_some_and(is_ascii_digits)) && parts.next().is_none()
}

fn is_metadata_line(line: &str) -> bool {
    let Some((version, metadata)) = line
        .strip_prefix("rustup-init ")
        .and_then(|line| line.split_once(" ("))
    else {
        return false;
    };
    let Some((sha, date)) = metadata
        .strip_suffix(')')
        .and_then(|metadata| metadata.split_once(' '))
    else {
        return false;
    };
    let date = date.as_bytes();
    is_dotted_numbers(version, 3)
        && (7..=40).contains(&sha.len())
        && sha
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && date.len() == 10
        && date[4] == b'-'
        && date[7] == b'-'
        && date
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
}

fn git(args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .context("cannot run git")?;
    ensure!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr).trim()
    );
    String::from_utf8(output.stdout).context("git output is not UTF-8")
}
