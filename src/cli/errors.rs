#![allow(clippy::large_enum_variant)]
#![allow(dead_code)]

use std::io;
use std::path::PathBuf;

use lazy_static::lazy_static;
use regex::Regex;
use strsim::damerau_levenshtein;
use thiserror::Error as ThisError;

#[derive(ThisError, Debug)]
pub enum CLIError {
    #[error("couldn't determine self executable name")]
    NoExeName,
    #[error("rustup is not installed at '{}'", .p.display())]
    NotSelfInstalled { p: PathBuf },
    #[error("failure reading directory {}", .p.display())]
    ReadDirError { p: PathBuf, source: io::Error },
    #[error("failure during windows uninstall")]
    WindowsUninstallMadness,
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
