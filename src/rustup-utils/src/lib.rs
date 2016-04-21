#![feature(core_intrinsics)] // For type_name().
#![feature(fundamental)]

extern crate hyper;
extern crate openssl;
extern crate rand;
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
#[cfg(windows)]
extern crate userenv;

pub mod notify;
pub mod errors;
pub mod notifications;
pub mod raw;
pub mod utils;

pub use errors::{Error};
pub use notifications::{Notification, NotifyHandler};
