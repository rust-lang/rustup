#![recursion_limit = "1024"] // for error_chain!

extern crate download;
#[macro_use]
extern crate error_chain;
extern crate rand;
extern crate scopeguard;
extern crate semver;
extern crate sha2;
extern crate toml;
extern crate url;

#[cfg(windows)]
extern crate advapi32;
#[cfg(windows)]
extern crate kernel32;
#[cfg(windows)]
extern crate ole32;
#[cfg(windows)]
extern crate shell32;
#[cfg(windows)]
extern crate userenv;
#[cfg(windows)]
extern crate winapi;
#[cfg(windows)]
extern crate winreg;

#[cfg(unix)]
extern crate libc;

pub mod errors;
pub mod notifications;
pub mod raw;
pub mod tty;
pub mod utils;
pub mod toml_utils;

pub use errors::*;
pub use notifications::Notification;
pub mod notify;
