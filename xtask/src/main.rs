use std::{env, fmt, fs, process::Command, str::FromStr};

use anyhow::{Context, Result};
use jiff::civil::Date;
use semver::Version;
use serde::Deserialize;

fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let command = args.next();
    let args = args.collect::<Vec<_>>();
    let usage = "usage: cargo xtask bump-init-sh <remote>/<main|release/X.Y>";
    match command.as_deref() {
        Some("bump-init-sh") => {
            if args.len() != 1 {
                return Err(anyhow::Error::msg(usage));
            }
            bump_init_sh(&args[0])
        }
        Some(command) => Err(anyhow::Error::msg(format!(
            "unknown xtask command {command:?}\n{usage}"
        ))),
        None => Err(anyhow::Error::msg(usage)),
    }
}

fn bump_init_sh(release_branch: &str) -> Result<()> {
    if !is_release_branch(release_branch) {
        return Err(anyhow::Error::msg(
            "expected a remote-tracking main or release/X.Y branch",
        ));
    }

    let (commit, metadata) = release_metadata(release_branch)?;
    let contents = fs::read_to_string("rustup-init.sh")?;
    let updated = replace_metadata_line(&contents, &metadata)?;
    if updated != contents {
        fs::write("rustup-init.sh", updated)?;
    }
    println!("{release_branch} ({commit}): {metadata}");

    let status = Command::new("cargo")
        .args(["test", "--features=test", "--", "cli_rustup_init_ui"])
        .env("SNAPSHOTS", "overwrite")
        .status()
        .context("cannot run rustup-init.sh help snapshot update")?;
    if !status.success() {
        return Err(anyhow::Error::msg(format!(
            "rustup-init.sh help snapshot update failed: {status}"
        )));
    }
    if cfg!(windows) {
        eprintln!("warning: rustup-init.sh help snapshot is not updated on Windows");
    }
    Ok(())
}

fn release_metadata(release_branch: &str) -> Result<(String, MetadataLine)> {
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
    let manifest = toml::from_str::<Manifest>(&git(&["show", &format!("{commit}:Cargo.toml")])?)
        .context("cannot parse release source Cargo.toml")?;
    let metadata = MetadataLine {
        version: parse_version(&manifest.package.version)
            .context("expected package.version in the release source to be X.Y.Z")?,
        sha: commit.get(..9).context("missing commit SHA")?.to_owned(),
        date: Date::strptime("%Y-%m-%d", commit_date).context("invalid commit date")?,
    };
    Ok((commit.to_owned(), metadata))
}

fn replace_metadata_line(contents: &str, metadata: &MetadataLine) -> Result<String> {
    let error = "expected exactly one rustup-init version/SHA/date line in rustup-init.sh";
    let replacement = metadata.to_string();
    let mut updated = String::with_capacity(contents.len());
    let mut found = false;
    for line in contents.split_inclusive('\n') {
        if line.starts_with("rustup-init ") {
            if found {
                return Err(anyhow::Error::msg(error));
            }
            found = true;
            let (existing, line_ending) = if let Some(existing) = line.strip_suffix("\r\n") {
                (existing, "\r\n")
            } else if let Some(existing) = line.strip_suffix('\n') {
                (existing, "\n")
            } else {
                (line, "")
            };
            let existing = MetadataLine::from_str(existing)?;
            if existing == *metadata {
                updated.push_str(line);
                continue;
            }
            updated.push_str(&replacement);
            updated.push_str(line_ending);
        } else {
            updated.push_str(line);
        }
    }
    if !found {
        return Err(anyhow::Error::msg(error));
    }

    Ok(updated)
}

fn is_release_branch(value: &str) -> bool {
    let Some((remote, branch)) = value.split_once('/') else {
        return false;
    };
    if remote.is_empty() {
        return false;
    }
    if branch == "main" {
        return true;
    }
    let Some(version) = branch.strip_prefix("release/") else {
        return false;
    };
    let Some((major, minor)) = version.split_once('.') else {
        return false;
    };
    [major, minor]
        .into_iter()
        .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

fn git(args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .context("cannot run git")?;
    if !output.status.success() {
        return Err(anyhow::Error::msg(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    String::from_utf8(output.stdout).context("git output is not UTF-8")
}

#[derive(Deserialize)]
struct Manifest {
    package: Package,
}

#[derive(Deserialize)]
struct Package {
    version: String,
}

#[derive(Debug, Eq, PartialEq)]
struct MetadataLine {
    version: Version,
    sha: String,
    date: Date,
}

impl FromStr for MetadataLine {
    type Err = anyhow::Error;

    fn from_str(line: &str) -> Result<Self> {
        let body = line
            .strip_prefix("rustup-init ")
            .context("missing rustup-init prefix")?;
        let (version, rest) = body
            .split_once(" (")
            .context("missing metadata opening delimiter")?;
        let rest = rest
            .strip_suffix(')')
            .context("missing metadata closing delimiter")?;
        let (sha, date) = rest.split_once(' ').context("missing commit date")?;
        if !(7..=40).contains(&sha.len())
            || !sha
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(anyhow::Error::msg(
                "expected 7 to 40 lowercase hexadecimal SHA characters",
            ));
        }
        let parsed_date = Date::strptime("%Y-%m-%d", date)?;
        if parsed_date.to_string() != date {
            return Err(anyhow::Error::msg("expected date YYYY-MM-DD"));
        }
        Ok(Self {
            version: parse_version(version)?,
            sha: sha.to_owned(),
            date: parsed_date,
        })
    }
}

impl fmt::Display for MetadataLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "rustup-init {} ({} {})",
            self.version, self.sha, self.date
        )
    }
}

fn parse_version(value: &str) -> Result<Version> {
    let version = Version::parse(value)?;
    if !version.pre.is_empty() || !version.build.is_empty() {
        return Err(anyhow::Error::msg("expected version X.Y.Z"));
    }
    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::{Manifest, MetadataLine, replace_metadata_line};

    #[test]
    fn metadata_update_preserves_line_endings() {
        let old = "rustup-init 1.29.0 (d243d7d4e 2026-03-04)";
        let new = "rustup-init 1.29.3 (edbc831ce 2026-11-02)";
        let metadata = new.parse::<MetadataLine>().unwrap();

        for line_ending in ["\n", "\r\n"] {
            let contents = format!("before{line_ending}{old}{line_ending}after{line_ending}");
            let expected = format!("before{line_ending}{new}{line_ending}after{line_ending}");
            assert_eq!(
                replace_metadata_line(&contents, &metadata).unwrap(),
                expected
            );
            assert_eq!(
                replace_metadata_line(&expected, &metadata).unwrap(),
                expected
            );
        }

        let contents = format!("before\n{old}");
        let expected = format!("before\n{new}");
        assert_eq!(
            replace_metadata_line(&contents, &metadata).unwrap(),
            expected
        );
    }

    #[test]
    fn metadata_round_trip() {
        for line in [
            "rustup-init 1.29.3 (edbc831ce 2026-11-02)",
            "rustup-init 0.0.0 (abcdef0 2000-02-29)",
            "rustup-init 18446744073709551615.2.3 (0123456789abcdef0123456789abcdef01234567 2024-02-29)",
        ] {
            assert_eq!(line.parse::<MetadataLine>().unwrap().to_string(), line);
        }
    }

    #[test]
    fn malformed_metadata() {
        for line in [
            "rustup-init 01.2.3 (abcdef0 2026-11-02)",
            "rustup-init 18446744073709551616.2.3 (abcdef0 2026-11-02)",
            "rustup-init 1.2.3 (abcdef0 2025-02-29)",
            "rustup-init 1.2.3 (abcdef0 1900-02-29)",
            "rustup-init 1.2.3 (abcdef0 2026-04-31)",
            "rustup-init 1.2.3 (abcdef0 0000-00-00)",
            "rustup-init 1.2.3 (abcdef0 2026-99-99)",
            "rustup-init 1.2.3 (abcdef0 2026-11-02T12:00:00Z)",
            "rustup-init 1.2.3 (abcdef0 2026-11-02[UTC])",
            "rustup-init 1.2.3 (abcdef0 20261102)",
            "rustup-init 1.2 (abcdef0 2026-11-02)",
            "rustup-init 1.2.3.4 (abcdef0 2026-11-02)",
            "rustup-init 1..3 (abcdef0 2026-11-02)",
            "rustup-init ١.2.3 (abcdef0 2026-11-02)",
            "rustup-init 1.2.3-beta (abcdef0 2026-11-02)",
            "rustup-init 1.2.3+build (abcdef0 2026-11-02)",
            "rustup-init 1.2.3 (ABCDEF0 2026-11-02)",
            "rustup-init 1.2.3 (abcdef 2026-11-02)",
            "rustup-init 1.2.3 (0123456789abcdef0123456789abcdef012345678 2026-11-02)",
            "rustup-init 1.2.3 (abcdef0 2026-1-02)",
            "rustup-init 1.2.3 (abcdef0 2026-11-02) trailing",
            "rustup-init 1.2.3 (abcdef0 2026-11-02",
            "1.2.3 (abcdef0 2026-11-02)",
        ] {
            assert!(line.parse::<MetadataLine>().is_err(), "{line:?}");
        }
    }

    #[test]
    fn manifest_version() {
        let manifest = toml::from_str::<Manifest>(
            "[package]\nname = 'rustup'\nversion = '1.29.3'\n[dependencies]\nanyhow = '1'",
        )
        .unwrap();
        assert_eq!(manifest.package.version, "1.29.3");
        for manifest in [
            "",
            "[package]",
            "[package]\nversion = 1",
            "[package]\nversion = {}",
        ] {
            assert!(toml::from_str::<Manifest>(manifest).is_err());
        }
    }
}
