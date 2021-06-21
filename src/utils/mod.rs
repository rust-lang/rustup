///!  Utility functions for Rustup
pub(crate) mod notifications;
pub mod raw;
pub(crate) mod toml_utils;
pub(crate) mod tty;
pub(crate) mod units;
#[allow(clippy::module_inception)]
pub mod utils;

pub(crate) use crate::utils::notifications::Notification;
pub(crate) mod notify;
