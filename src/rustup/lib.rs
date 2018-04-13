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
