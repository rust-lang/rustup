#![recursion_limit = "1024"] // for error_chain!

#[allow(deprecated)] // WORKAROUND https://github.com/rust-lang-nursery/error-chain/issues/254
pub mod errors;
pub mod notifications;
pub mod raw;
pub mod toml_utils;
pub mod tty;
pub mod utils;

pub use crate::errors::*;
pub use crate::notifications::Notification;
pub mod notify;
