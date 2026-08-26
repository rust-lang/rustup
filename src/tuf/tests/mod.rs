use std::{fs, path::Path};

use chrono::{DateTime, Duration, Utc};

mod config;
mod download;
mod manifest;

/// The fixture repository: `src/` holds the published manifests and `tuf/`
/// the TUF metadata and targets generated from them.
pub(crate) const REPO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/tuf/tests/repo");

/// A server that is deliberately unreachable: with TUF enabled nothing should
/// ever be fetched from it directly. Target names are URL paths, so this has
/// no path of its own and the fixture's targets sit directly under it.
pub(crate) const DIST_ROOT: &str = "https://dist.invalid";

/// One second before the fixture's timestamp role expires.
pub(crate) fn ignore_date(tuf: &Path) -> String {
    let json = fs::read_to_string(tuf.join("metadata/timestamp.json")).unwrap();
    let key = "\"expires\":\"";
    let start = json.find(key).unwrap() + key.len();
    let end = start + json[start..].find('"').unwrap();
    let expires = json[start..end].parse::<DateTime<Utc>>().unwrap();
    (expires - Duration::seconds(1)).to_rfc3339()
}
