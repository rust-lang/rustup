//! TUF (The Update Framework) verification of dist channel manifests.
//!
//! [`TufConfig`] is read from the `RUSTUP_TUF_*` environment variables through
//! [`Process`](crate::process::Process) and hangs off `Cfg`. When it is
//! enabled, `Download` fetches channel manifests through `TufRepository`
//! instead of plain HTTP, and `ChannelToolchainName` maps channels onto the
//! repository's target layout. Everything else rustup downloads, component
//! tarballs included, still comes straight from the dist server.

mod config;
pub use self::config::{TufConfig, TufMode};

mod manifest;

mod repository;
pub(crate) use self::repository::{TufRepository, Verification};

#[cfg(test)]
pub(crate) mod tests;
