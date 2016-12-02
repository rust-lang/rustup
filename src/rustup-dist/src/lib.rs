#![recursion_limit = "1024"]

extern crate regex;
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
extern crate sha2;

#[cfg(windows)]
extern crate winapi;
#[cfg(windows)]
extern crate winreg;
#[cfg(windows)]
extern crate user32;
#[cfg(windows)]
extern crate kernel32;
#[cfg(not(windows))]
extern crate libc;

pub use errors::*;
pub use notifications::{Notification};

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
