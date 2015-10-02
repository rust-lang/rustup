#![feature(coerce_unsized, fs_canonicalize)]

extern crate rand;
extern crate hyper;
extern crate regex;
#[cfg(windows)]
extern crate winapi;
#[cfg(windows)]
extern crate winreg;

pub use errors::*;
pub use install::*;

pub mod env_var;
pub mod notify;
pub mod utils;
pub mod temp;

#[cfg(windows)]
pub mod msi;

pub mod dist;
mod errors;
mod install;
