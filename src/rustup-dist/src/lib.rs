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

pub use crate::errors::*;
pub use crate::notifications::Notification;

pub mod temp;

pub mod component;
pub mod config;
pub mod dist;
pub mod download;
pub mod errors;
pub mod manifest;
pub mod manifestation;
pub mod notifications;
pub mod prefix;
