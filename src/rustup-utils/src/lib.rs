#![recursion_limit = "1024"] // for error_chain!

extern crate rand;
extern crate scopeguard;
#[macro_use]
extern crate error_chain;
extern crate rustc_serialize;
extern crate sha2;
extern crate url;
extern crate toml;
extern crate download;
extern crate semver;

#[cfg(windows)]
extern crate winapi;
#[cfg(windows)]
extern crate winreg;
#[cfg(windows)]
extern crate shell32;
#[cfg(windows)]
extern crate ole32;
#[cfg(windows)]
extern crate kernel32;
#[cfg(windows)]
extern crate advapi32;
#[cfg(windows)]
extern crate userenv;

#[cfg(unix)]
extern crate libc;

pub mod errors;
pub mod notifications;
pub mod raw;
pub mod tty;
pub mod utils;
pub mod toml_utils;

pub use errors::*;
pub use notifications::{Notification};
pub mod notify;
