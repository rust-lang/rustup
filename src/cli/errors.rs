#![allow(clippy::large_enum_variant)]
#![allow(dead_code)]
#![allow(deprecated)] // because of `Error::description` deprecation in `error_chain`

use std::io;
use std::path::PathBuf;

use clap::Shell;
use error_chain::error_chain;
use lazy_static::lazy_static;
use regex::Regex;
use strsim::damerau_levenshtein;

use super::rustup_mode::CompletionCommand;
use crate::dist::temp;
use crate::{TOOLS, DUP_TOOLS};

error_chain! {
    links {
        Rustup(crate::Error, crate::ErrorKind);
    }

    foreign_links {
        Clap(clap::Error);
        Temp(temp::Error);
        Io(io::Error);
        Term(term::Error);
    }

    errors {
        InvalidCustomToolchainName(t: String) {
            display("invalid custom toolchain name: '{}'", t)
        }
        PermissionDenied {
            description("permission denied")
        }
        ToolchainNotInstalled(t: String) {
            description("toolchain is not installed")
            display("toolchain '{}' is not installed", t)
        }
        InvalidToolchainName(t: String) {
            description("invalid toolchain name")
            display("invalid toolchain name: '{}'{}", t, maybe_suggest_toolchain(t))
        }
        InfiniteRecursion {
            description("infinite recursion detected")
        }
        NoExeName {
            description("couldn't determine self executable name")
        }
        UnknownProxyName(n: String) {
            description("unknown proxy name")
            display(
                "unknown proxy name: '{}'; valid proxy names are {}",
                n,
                valid_proxy_names(),
            )
        }
        NotSelfInstalled(p: PathBuf) {
            description("rustup is not installed")
            display("rustup is not installed at '{}'", p.display())
        }
        WindowsUninstallMadness {
            description("failure during windows uninstall")
        }
        UnsupportedCompletionShell(shell: Shell, cmd: CompletionCommand) {
            description("completion script for shell not yet supported for tool")
            display("{} does not currently support completions for {}", cmd, shell)
        }
        TargetAllSpecifiedWithTargets(t: Vec<String>) {
            description(
                "the `all` target, which installs all available targets, \
                 cannot be combined with other targets"
            )
            display("`rustup target add {}` includes `all`", t.join(" "))
        }
        WritingShellProfile {
            path: PathBuf,
        } {
            description("could not amend shell profile")
            display("could not amend shell profile: '{}'", path.display())
        }
    }
}

fn valid_proxy_names() -> String {
    TOOLS
        .iter().chain(DUP_TOOLS.iter())
        .map(|s| format!("'{}'", s))
        .collect::<Vec<_>>()
        .join(", ")
}

fn maybe_suggest_toolchain(bad_name: &str) -> String {
    let bad_name = &bad_name.to_ascii_lowercase();
    static VALID_CHANNELS: &[&str] = &["stable", "beta", "nightly"];
    lazy_static! {
        static ref NUMBERED: Regex = Regex::new(r"^\d+\.\d+$").unwrap();
    }

    if NUMBERED.is_match(bad_name) {
        return format!(
            ". Toolchain numbers tend to have three parts, e.g. {}.0",
            bad_name
        );
    }

    // Suggest only for very small differences
    // High number can result in inaccurate suggestions for short queries e.g. `rls`
    const MAX_DISTANCE: usize = 3;

    let mut scored: Vec<_> = VALID_CHANNELS
        .iter()
        .filter_map(|s| {
            let distance = damerau_levenshtein(bad_name, s);
            if distance <= MAX_DISTANCE {
                Some((distance, s))
            } else {
                None
            }
        })
        .collect();
    scored.sort();
    if scored.is_empty() {
        String::new()
    } else {
        format!(". Did you mean '{}'?", scored[0].1)
    }
}
