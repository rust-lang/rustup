//! Installation from a Rust distribution server

pub use crate::dist::notifications::Notification;
pub use dist::*;

pub mod temp;

pub mod component;
pub(crate) mod config;
#[allow(clippy::module_inception)]
mod dist;
pub mod download;
pub mod manifest;
pub mod manifestation;
pub(crate) mod notifications;
pub mod prefix;
pub(crate) mod triple;
