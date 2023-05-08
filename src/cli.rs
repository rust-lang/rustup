/// The CLI specific code lives in the cli module and sub-modules.
#[macro_use]
pub mod log;
pub mod common;
mod download_tracker;
pub mod errors;
mod help;
mod job;
mod markdown;
pub mod proxy_mode;
pub mod rustup_mode;
pub mod self_update;
pub mod setup_mode;
mod topical_doc;
