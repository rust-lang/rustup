use std::iter;

use clap::CommandFactory;
use clap_complete::{ArgValueCandidates, engine::CompletionCandidate};

use crate::{
    cli::rustup_mode::Rustup,
    config::Cfg,
    toolchain::{ChannelToolchain, ToolchainName, ToolchainNameOrPath},
};

pub(super) fn rustup_command(cfg: &Cfg<'_>) -> clap::Command {
    let mut toolchains = cfg.list_toolchains(true).unwrap_or_default();
    toolchains.sort();

    // Collect available targets from the first installed
    // distributable toolchain. Uses only on-disk manifest data
    let available_targets = toolchains
        .iter()
        .find_map(|name| {
            let ToolchainName::Channel(desc) = name else {
                return None;
            };
            let dist = ChannelToolchain::new(cfg, desc.clone()).ok()?;
            let components = dist.components().ok()?;
            Some(
                components
                    .into_iter()
                    .filter_map(|c| {
                        (c.component.short_name() == "rust-std" && c.available)
                            .then(|| c.component.target.map(|t| t.to_string()))?
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .unwrap_or_default();

    let active_channel_toolchain = match cfg.active_toolchain() {
        Ok(Some((ToolchainNameOrPath::Named(ToolchainName::Channel(desc)), _))) => {
            ChannelToolchain::new(cfg, desc).ok()
        }
        _ => None,
    };
    let installed_targets = active_channel_toolchain
        .and_then(|dist| dist.toolchain.installed_targets().ok())
        .unwrap_or_default();

    Rustup::command()
        .mut_arg("+toolchain", move |arg| {
            arg.add(ArgValueCandidates::new(move || {
                toolchains
                    .iter()
                    .map(|t| CompletionCandidate::new(format!("+{t}")))
                    .collect()
            }))
        })
        .mut_subcommand("target", move |target_cmd| {
            target_cmd
                .mut_subcommand("add", move |add_cmd| {
                    add_cmd.mut_arg("target", move |arg| {
                        arg.add(ArgValueCandidates::new(move || {
                            // "all" is a special keyword that installs every available target
                            iter::once("all")
                                .chain(available_targets.iter().map(AsRef::as_ref))
                                .map(CompletionCandidate::new)
                                .collect()
                        }))
                    })
                })
                .mut_subcommand("remove", move |remove_cmd| {
                    remove_cmd.mut_arg("target", move |arg| {
                        arg.add(ArgValueCandidates::new(move || {
                            installed_targets
                                .iter()
                                .map(|t| CompletionCandidate::new(t.to_string()))
                                .collect()
                        }))
                    })
                })
        })
}

#[cfg(all(test, feature = "test"))]
mod tests {
    use std::{
        collections::{HashMap, HashSet},
        ffi::OsString,
        fs,
    };

    use super::rustup_command;
    use crate::{config::Cfg, process::TestProcess};

    fn complete(cfg: &Cfg<'_>, args: &[&str], index: usize) -> Vec<String> {
        let mut command = rustup_command(cfg);
        clap_complete::engine::complete(
            &mut command,
            args.iter().map(OsString::from).collect(),
            index,
            Some(&cfg.current_dir),
        )
        .unwrap()
        .into_iter()
        .map(|candidate| candidate.get_value().to_string_lossy().into_owned())
        .collect()
    }

    #[test]
    fn dynamic_completion_distinguishes_toolchains_and_subcommands() {
        let (rustup_home, process) = completion_cfg();
        fs::create_dir_all(rustup_home.path().join("toolchains/custom")).unwrap();
        let cfg = Cfg::from_env(rustup_home.path().to_owned(), true, false, &process).unwrap();

        assert_eq!(complete(&cfg, &["rustup", "+cus"], 1), ["+custom"]);

        for (args, index) in [
            (&["rustup", "component", ""][..], 2),
            (&["rustup", "+custom", "component", ""][..], 3),
        ] {
            let candidates = complete(&cfg, args, index);
            let candidates = candidates
                .iter()
                .map(String::as_str)
                .collect::<HashSet<_>>();
            let expected = HashSet::from(["list", "add", "remove"]);
            assert!(
                candidates.is_superset(&expected),
                "missing component subcommands for {args:?}: {:?}",
                expected.difference(&candidates).collect::<Vec<_>>()
            );
            let root_only = HashSet::from(["install", "toolchain", "show"]);
            assert!(
                candidates.is_disjoint(&root_only),
                "unexpected root-only subcommands for {args:?}: {:?}",
                candidates.intersection(&root_only).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn dynamic_completion_target_add_offers_all_keyword() {
        let (rustup_home, process) = completion_cfg();
        let cfg = Cfg::from_env(rustup_home.path().to_owned(), true, false, &process).unwrap();

        let candidates = complete(&cfg, &["rustup", "target", "add", ""], 3);
        assert!(candidates.contains(&"all".to_owned()));
    }

    #[test]
    fn dynamic_completion_target_add_prefix_filters_all() {
        let (rustup_home, process) = completion_cfg();
        let cfg = Cfg::from_env(rustup_home.path().to_owned(), true, false, &process).unwrap();

        // "al" prefix matches only "all"
        let candidates = complete(&cfg, &["rustup", "target", "add", "al"], 3);
        assert_eq!(candidates, ["all"]);

        let candidates = complete(&cfg, &["rustup", "target", "add", "x86"], 3);
        assert!(!candidates.contains(&"all".to_owned()));
    }

    #[test]
    fn dynamic_completion_target_add_does_not_bleed_into_other_subcmds() {
        let (rustup_home, process) = completion_cfg();
        let cfg = Cfg::from_env(rustup_home.path().to_owned(), true, false, &process).unwrap();

        let candidates = complete(&cfg, &["rustup", "target", "remove", ""], 3);
        assert!(!candidates.contains(&"all".to_owned()));
    }

    #[test]
    fn dynamic_completion_target_remove() {
        let (rustup_home, process) = completion_cfg();
        let manifest_dir = rustup_home.path().join(MANIFEST_DIR);
        fs::create_dir_all(&manifest_dir).unwrap();
        fs::write(
            manifest_dir.join("components"),
            "rustc-x86_64-unknown-linux-gnu\n\
             rust-std-x86_64-unknown-linux-gnu\n\
             rust-std-x86_64-unknown-linux-musl\n",
        )
        .unwrap();

        let cfg = Cfg::from_env(rustup_home.path().to_owned(), true, false, &process).unwrap();
        cfg.set_default(Some(&TOOLCHAIN.parse().unwrap())).unwrap();

        assert_eq!(
            complete(&cfg, &["rustup", "target", "remove", "x86_64"], 3),
            ["x86_64-unknown-linux-gnu", "x86_64-unknown-linux-musl"]
        );
    }

    #[test]
    fn dynamic_completion_target_add_offers_real_targets() {
        let (rustup_home, process) = completion_cfg();
        let manifest_dir = rustup_home.path().join(MANIFEST_DIR);
        fs::create_dir_all(&manifest_dir).unwrap();
        fs::write(
            manifest_dir.join("multirust-channel-manifest.toml"),
            MANIFEST,
        )
        .unwrap();

        let cfg = Cfg::from_env(rustup_home.path().to_owned(), true, false, &process).unwrap();

        let candidates = complete(&cfg, &["rustup", "target", "add", ""], 3);

        assert!(candidates.contains(&"aarch64-unknown-linux-gnu".to_owned()));
        assert!(candidates.contains(&"x86_64-unknown-linux-musl".to_owned()));
        assert!(candidates.contains(&"x86_64-unknown-linux-gnu".to_owned()));
        assert!(candidates.contains(&"all".to_owned()));

        let candidates = complete(&cfg, &["rustup", "target", "add", "aarch64"], 3);
        assert_eq!(candidates, ["aarch64-unknown-linux-gnu"]);
    }

    /// Creates a bare rustup home with no toolchains installed.
    /// Used by completion tests to verify the completer is robust
    /// and to set up a base environment for further customisation.
    fn completion_cfg() -> (tempfile::TempDir, crate::process::Process) {
        let rustup_home = tempfile::tempdir().unwrap();
        let vars = HashMap::from([
            (
                "RUSTUP_HOME".to_owned(),
                rustup_home.path().display().to_string(),
            ),
            (
                "CARGO_HOME".to_owned(),
                rustup_home.path().display().to_string(),
            ),
            (
                "RUSTUP_OVERRIDE_UNIX_FALLBACK_SETTINGS".to_owned(),
                rustup_home
                    .path()
                    .join("missing-settings.toml")
                    .display()
                    .to_string(),
            ),
        ]);
        let process = TestProcess::new(rustup_home.path(), &["rustup"], vars, "").process;
        (rustup_home, process)
    }

    const TOOLCHAIN: &str = "stable-x86_64-unknown-linux-gnu";
    const MANIFEST_DIR: &str = "toolchains/stable-x86_64-unknown-linux-gnu/lib/rustlib";
    const MANIFEST: &str = r#"
manifest-version = "2"
date = "2025-01-01"
[pkg.rust]
  version = "rustc 1.80.0 (fake)"
  [pkg.rust.target.x86_64-unknown-linux-gnu]
    available = true
    url = "https://example.com"
    hash = "deadbeef"
    [[pkg.rust.target.x86_64-unknown-linux-gnu.components]]
      pkg = "rustc"
      target = "x86_64-unknown-linux-gnu"
    [[pkg.rust.target.x86_64-unknown-linux-gnu.components]]
      pkg = "rust-std"
      target = "x86_64-unknown-linux-gnu"
    [[pkg.rust.target.x86_64-unknown-linux-gnu.extensions]]
      pkg = "rust-std"
      target = "aarch64-unknown-linux-gnu"
    [[pkg.rust.target.x86_64-unknown-linux-gnu.extensions]]
      pkg = "rust-std"
      target = "x86_64-unknown-linux-musl"
[pkg.rustc]
  version = "rustc 1.80.0 (fake)"
  [pkg.rustc.target.x86_64-unknown-linux-gnu]
    available = true
    url = "https://example.com"
    hash = "deadbeef"
[pkg.rust-std]
  version = "rustc 1.80.0 (fake)"
  [pkg.rust-std.target.x86_64-unknown-linux-gnu]
    available = true
    url = "https://example.com"
    hash = "deadbeef"
  [pkg.rust-std.target.aarch64-unknown-linux-gnu]
    available = true
    url = "https://example.com"
    hash = "deadbeef"
  [pkg.rust-std.target.x86_64-unknown-linux-musl]
    available = true
    url = "https://example.com"
    hash = "deadbeef"
"#;
}
