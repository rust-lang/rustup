//! Fixed names and paths used across the TUF module.

/// Name of the default [`TufConfig::home`](super::TufConfig::home) directory
/// under `RUSTUP_HOME`.
pub(super) const DEFAULT_HOME_DIR: &str = "tuf";

/// Path prefix of a TUF repository's metadata files.
pub(super) const METADATA_PREFIX: &str = "metadata";

/// Path prefix of a TUF repository's target files.
pub(super) const TARGETS_PREFIX: &str = "targets";
