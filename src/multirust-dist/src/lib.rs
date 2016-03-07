#![feature(core_intrinsics)] // For type_name().
#![feature(fundamental)]

extern crate rand;
extern crate hyper;
extern crate regex;
extern crate openssl;
extern crate itertools;
#[cfg(windows)]
extern crate winapi;
#[cfg(windows)]
extern crate winreg;
#[cfg(windows)]
extern crate shell32;
#[cfg(windows)]
extern crate ole32;
extern crate tempdir;
extern crate walkdir;
extern crate toml;
extern crate flate2;
extern crate tar;

pub use errors::{Error, Notification, NotifyHandler};

#[macro_use]
pub mod notify;
pub mod utils;
pub mod temp;

pub mod dist;
pub mod errors;
pub mod prefix;
pub mod component;
pub mod manifestation;
pub mod download;
pub mod manifest;
pub mod config;
mod toml_utils;
