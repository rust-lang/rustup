#![feature(core_intrinsics)] // For type_name().
#![feature(fundamental)]

extern crate hyper;
extern crate openssl;
extern crate rand;
#[macro_use]
extern crate scopeguard;

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

pub mod notify;
pub mod errors;
pub mod raw;
pub mod utils;

pub use errors::{Error, Notification, NotifyHandler};
