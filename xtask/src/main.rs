use std::{env, fs, process::Command};

use anyhow::{Context, Result, ensure};
use regex::{Regex, bytes::Regex as BytesRegex};

fn main() -> Result<()> {
    let args: Vec<_> = env::args().skip(1).collect();
    ensure!(
        args.len() == 1,
        "usage: cargo run --manifest-path xtask/Cargo.toml -- <remote>/<main|release/X.Y>"
    );
    let release_branch = &args[0];
    ensure!(
        Regex::new(r"\A[^/]+/(main|release/[0-9]+\.[0-9]+)\z")?.is_match(release_branch),
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
        .context("expected package.version in the release source to be X.Y.Z")?;
    ensure!(
        Regex::new(r"\A[0-9]+\.[0-9]+\.[0-9]+\z")?.is_match(version),
        "expected package.version in the release source to be X.Y.Z"
    );

    let mut contents = fs::read("rustup-init.sh")?;
    let lines = BytesRegex::new(r"(?m-u)^rustup-init .*$")?;
    let mut matches = lines.find_iter(&contents);
    let error = "expected exactly one rustup-init version/SHA/date line in rustup-init.sh";
    let metadata_line = matches.next().context(error)?;
    ensure!(
        matches.next().is_none()
            && BytesRegex::new(
                r"\Arustup-init [0-9]+\.[0-9]+\.[0-9]+ \([0-9a-f]{7,40} [0-9]{4}-[0-9]{2}-[0-9]{2}\)\z"
            )?
            .is_match(metadata_line.as_bytes()),
        error
    );

    let metadata = format!("rustup-init {version} ({} {commit_date})", &commit[..9]);
    if metadata_line.as_bytes() != metadata.as_bytes() {
        let range = metadata_line.range();
        contents.splice(range, metadata.bytes());
        fs::write("rustup-init.sh", contents)?;
    }
    println!("{release_branch} ({commit}): {metadata}");
    Ok(())
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
