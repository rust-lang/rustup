#![recursion_limit = "1024"]

pub use crate::config::*;
pub use crate::errors::*;
pub use crate::notifications::*;
pub use crate::toolchain::*;
pub use rustup_utils::{notify, toml_utils, utils};

// A list of all binaries which Rustup will proxy.
pub static TOOLS: &'static [&'static str] = &[
    "rustc",
    "rustdoc",
    "cargo",
    "rust-lldb",
    "rust-gdb",
    "rls",
    "cargo-clippy",
    "clippy-driver",
    "cargo-miri",
];

// Tools which are commonly installed by Cargo as well as rustup. We take a bit
// more care with these to ensure we don't overwrite the user's previous
// installation.
pub static DUP_TOOLS: &'static [&'static str] = &["rustfmt", "cargo-fmt"];

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
        "rust-lldb" => Some("lldb-preview"),
        "rust-gdb" => Some("gdb-preview"),
        "rls" => Some("rls"),
        "cargo-clippy" => Some("clippy"),
        "clippy-driver" => Some("clippy"),
        "cargo-miri" => Some("miri"),
        "rustfmt" | "cargo-fmt" => Some("rustfmt"),
        _ => None,
    }
}

pub mod command;
mod config;
pub mod env_var;
#[allow(deprecated)] // WORKAROUND https://github.com/rust-lang-nursery/error-chain/issues/254
mod errors;
mod install;
mod notifications;
pub mod settings;
mod toolchain;
