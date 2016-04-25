#![recursion_limit = "1024"]

extern crate rustup_dist;
#[macro_use]
extern crate rustup_utils;
#[macro_use]
extern crate error_chain;
extern crate hyper;
extern crate regex;
extern crate itertools;
extern crate rustc_serialize;
extern crate time;

pub use errors::*;
pub use notifications::*;
pub use config::*;
pub use toolchain::*;
pub use override_db::*;
pub use rustup_utils::{utils, notify};

mod errors;
mod notifications;
mod override_db;
mod toolchain;
mod config;
mod env_var;
mod install;
pub mod telemetry;
pub mod command;
