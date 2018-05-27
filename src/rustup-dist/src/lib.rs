#![recursion_limit = "1024"]

#[macro_use]
extern crate error_chain;
extern crate flate2;
extern crate itertools;
extern crate regex;
extern crate rustup_utils;
extern crate sha2;
extern crate tar;
extern crate toml;
extern crate url;
extern crate walkdir;

#[cfg(not(windows))]
extern crate libc;
#[cfg(windows)]
extern crate winapi;
#[cfg(windows)]
extern crate winreg;

pub use errors::*;
pub use notifications::Notification;

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
