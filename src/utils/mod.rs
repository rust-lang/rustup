///!  Utility functions for Rustup
pub mod notifications;
pub mod raw;
pub mod toml_utils;
pub mod tty;
pub mod units;
#[allow(clippy::module_inception)]
pub mod utils;

pub use crate::utils::notifications::Notification;
pub mod notify;
