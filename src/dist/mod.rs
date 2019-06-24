//! Installation from a Rust distribution server

pub use crate::dist::notifications::Notification;

pub mod temp;

pub mod component;
pub mod config;
#[allow(clippy::module_inception)]
pub mod dist;
pub mod download;
pub mod manifest;
pub mod manifestation;
pub mod notifications;
pub mod prefix;
