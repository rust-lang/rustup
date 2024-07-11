#![deny(rust_2018_idioms)]
#![allow(
    clippy::type_complexity,
    clippy::result_large_err, // 288 bytes is our 'large' variant today, which is unlikely to be a performance problem
    clippy::arc_with_non_send_sync, // will get resolved as we move further into async
)]
#![recursion_limit = "1024"]

use anyhow::{anyhow, Result};
use errors::RustupError;
use itertools::{chain, Itertools};

#[macro_use]
extern crate rs_tracing;

// A list of all binaries which Rustup will proxy.
pub static TOOLS: &[&str] = &[
    "rustc",
    "rustdoc",
    "cargo",
    "rust-lldb",
    "rust-gdb",
    "rust-gdbgui",
    "rls",
    "cargo-clippy",
    "clippy-driver",
    "cargo-miri",
];

// Tools which are commonly installed by Cargo as well as rustup. We take a bit
// more care with these to ensure we don't overwrite the user's previous
// installation.
pub static DUP_TOOLS: &[&str] = &["rust-analyzer", "rustfmt", "cargo-fmt"];

// If the given name is one of the tools we proxy.
pub fn is_proxyable_tools(tool: &str) -> Result<()> {
    if chain!(TOOLS, DUP_TOOLS).contains(&tool) {
        Ok(())
    } else {
        Err(anyhow!(
            "unknown proxy name: '{tool}'; valid proxy names are {}",
            chain!(TOOLS, DUP_TOOLS)
                .map(|s| format!("'{s}'"))
                .join(", "),
        ))
    }
}

fn component_for_bin(binary: &str) -> Option<&'static str> {
    use std::env::consts::EXE_SUFFIX;

    let binary_prefix = match binary.find(EXE_SUFFIX) {
        _ if EXE_SUFFIX.is_empty() => binary,
        Some(i) => &binary[..i],
        None => binary,
    };

    match binary_prefix {
        "rustc" | "rustdoc" => Some("rustc"),
        "cargo" => Some("cargo"),
        "rust-lldb" | "rust-gdb" | "rust-gdbgui" => Some("rustc"), // These are not always available
        "rls" => Some("rls"),
        "cargo-clippy" => Some("clippy"),
        "clippy-driver" => Some("clippy"),
        "cargo-miri" => Some("miri"),
        "rustfmt" | "cargo-fmt" => Some("rustfmt"),
        _ => None,
    }
}

#[macro_use]
pub mod cli;
mod command;
mod config;
mod diskio;
pub mod dist;
pub mod env_var;
pub mod errors;
mod fallback_settings;
mod install;
pub mod notifications;
pub mod process;
mod settings;
#[cfg(feature = "test")]
pub mod test;
mod toolchain;
pub mod utils;

#[cfg(test)]
mod tests {
    use crate::{is_proxyable_tools, DUP_TOOLS, TOOLS};

    #[test]
    fn test_is_proxyable_tools() {
        for tool in TOOLS {
            assert!(is_proxyable_tools(tool).is_ok());
        }
        for tool in DUP_TOOLS {
            assert!(is_proxyable_tools(tool).is_ok());
        }
        let message = "unknown proxy name: 'unknown-tool'; valid proxy names are 'rustc', \
        'rustdoc', 'cargo', 'rust-lldb', 'rust-gdb', 'rust-gdbgui', 'rls', \
        'cargo-clippy', 'clippy-driver', 'cargo-miri', 'rust-analyzer', 'rustfmt', 'cargo-fmt'";
        assert_eq!(
            is_proxyable_tools("unknown-tool").unwrap_err().to_string(),
            message
        );
    }
}
