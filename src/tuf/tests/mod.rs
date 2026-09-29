use std::{fs, path::Path};

use chrono::{DateTime, Duration, Utc};

mod config;
mod download;
mod manifest;
mod online;

/// Fixture repository: manifests in `src/`, TUF metadata and targets in `tuf/`.
pub(crate) const REPO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/tuf/tests/repo");

/// Unreachable dist server: with TUF on, nothing is fetched from it directly.
pub(crate) const DIST_ROOT: &str = "https://dist.invalid";

/// One second before the fixture's timestamp expires.
pub(crate) fn ignore_date(tuf: &Path) -> String {
    let json = fs::read_to_string(tuf.join("metadata/timestamp.json")).unwrap();
    let key = "\"expires\":\"";
    let start = json.find(key).unwrap() + key.len();
    let end = start + json[start..].find('"').unwrap();
    let expires = json[start..end].parse::<DateTime<Utc>>().unwrap();
    (expires - Duration::seconds(1)).to_rfc3339()
}
