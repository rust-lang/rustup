//! Installation from a Rust distribution server

pub use crate::dist::errors::*;

pub mod temp;

pub mod component;
pub mod config;
pub mod dist;
pub mod download;
#[allow(deprecated)] // WORKAROUND https://github.com/rust-lang-nursery/error-chain/issues/254
pub mod errors;
pub mod manifest;
pub mod manifestation;
pub mod prefix;
