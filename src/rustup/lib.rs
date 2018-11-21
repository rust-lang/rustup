#![recursion_limit = "1024"]

#[macro_use]
extern crate error_chain;
extern crate itertools;
#[cfg(unix)]
extern crate libc;
extern crate regex;
extern crate rustup_dist;
extern crate rustup_utils;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate tempfile;
extern crate time;
extern crate toml;
extern crate url;

pub use errors::*;
pub use notifications::*;
pub use config::*;
pub use toolchain::*;
pub use rustup_utils::{notify, toml_utils, utils};


// A list of all binaries which Rustup will proxy.
pub static TOOLS: &'static [&'static str] =
    &["rustc", "rustdoc", "cargo", "rust-lldb", "rust-gdb", "rls", "cargo-clippy"];

// Tools which are commonly installed by Cargo as well as rustup. We take a bit
// more care with these to ensure we don't overwrite the user's previous
// installation.
pub static DUP_TOOLS: &'static [&'static str] = &["rustfmt", "cargo-fmt"];

fn component_for_bin(binary: &str) -> Option<&'static str> {
    match binary {
        "rustc" | "rustdoc" => Some("rustc"),
        "cargo" => Some("cargo"),
        "rust-lldb" => Some("lldb-preview"),
        "rust-gdb" => Some("gdb-preview"),
        "rls" => Some("rls"),
        "cargo-clippy" => Some("clippy"),
        "rustfmt" | "cargo-fmt" => Some("rustfmt"),
        _ => None,
    }
}

mod errors;
mod notifications;
mod toolchain;
mod config;
mod install;
pub mod settings;
pub mod telemetry;
pub mod command;
pub mod telemetry_analysis;
pub mod env_var;
