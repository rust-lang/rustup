#![deny(rust_2018_idioms)]
#![warn(clippy::use_self)]
#![recursion_limit = "1024"]

pub use crate::config::*;
pub use crate::errors::*;
pub use crate::notifications::*;
pub use crate::toolchain::*;
pub use crate::utils::{notify, toml_utils};

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
pub static DUP_TOOLS: &[&str] = &["rustfmt", "cargo-fmt"];

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
pub mod diskio;
pub mod dist;
pub mod env_var;
pub mod errors;
mod install;
mod notifications;
pub mod settings;
mod toolchain;
pub mod utils;
