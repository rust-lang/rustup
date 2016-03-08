#![feature(core_intrinsics)] // For type_name().
#![feature(fundamental)]

extern crate hyper;
extern crate openssl;
extern crate rand;

pub mod notify;
pub mod errors;
pub mod raw;
pub mod utils;

pub use errors::{Error, Notification, NotifyHandler};
