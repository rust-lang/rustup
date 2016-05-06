#![feature(core_intrinsics)] // For type_name().
#![feature(fundamental)]
#![recursion_limit = "1024"] // for error_chain!

extern crate hyper;
extern crate crypto;
extern crate rand;
extern crate scopeguard;
#[macro_use]
extern crate error_chain;
extern crate native_tls;

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

pub mod notify;
pub mod errors;
pub mod notifications;
pub mod raw;
pub mod tty;
pub mod utils;

pub use errors::*;
pub use notifications::{Notification, NotifyHandler};
