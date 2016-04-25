#![recursion_limit = "1024"]

extern crate hyper;
extern crate regex;
extern crate openssl;
extern crate itertools;
extern crate tempdir;
extern crate walkdir;
extern crate toml;
extern crate flate2;
extern crate tar;
#[macro_use]
extern crate rustup_utils;
#[macro_use]
extern crate error_chain;

pub use errors::*;
pub use notifications::{Notification, NotifyHandler};

pub mod temp;

pub mod dist;
pub mod errors;
pub mod notifications;
pub mod prefix;
pub mod component;
pub mod manifestation;
pub mod download;
pub mod manifest;
pub mod config;
mod toml_utils;
