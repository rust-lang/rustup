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
extern crate rust_manifest;
extern crate tempdir;
extern crate walkdir;

pub use errors::*;
pub use install::*;

pub mod env_var;
#[macro_use]
pub mod notify;
pub mod utils;
pub mod temp;

#[cfg(windows)]
pub mod msi;

pub mod dist;
mod errors;
mod install;
pub mod component;
pub mod manifest;
pub mod configured;
pub mod download;
