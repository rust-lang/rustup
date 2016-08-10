#![recursion_limit = "1024"]

extern crate rustup_dist;
#[macro_use]
extern crate rustup_utils;
#[macro_use]
extern crate error_chain;
extern crate url;
extern crate regex;
extern crate itertools;
extern crate rustc_serialize;
extern crate tempfile;
extern crate time;
extern crate toml;
#[cfg(unix)]
extern crate libc;

pub use errors::*;
pub use notifications::*;
pub use config::*;
pub use toolchain::*;
pub use rustup_utils::{utils, notify, toml_utils};

mod errors;
mod notifications;
mod toolchain;
mod config;
mod env_var;
mod install;
pub mod settings;
pub mod telemetry;
pub mod command;
pub mod telemetry_analysis;
