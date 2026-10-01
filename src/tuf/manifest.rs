use std::path::Path;

use anyhow::{Context, Result};
use chrono::NaiveDate;
use tracing::{debug, trace};

use crate::{
    config::Cfg,
    dist::{Channel, ChannelToolchainName, PartialVersion},
    download::DownloadOptions,
};

// The TUF-specific channel layout lives here rather than next to the v1 and
// v2 URL builders in `dist`, so the TUF code stays in one place.
impl ChannelToolchainName {
    /// The URL of this toolchain's channel manifest in the TUF target layout
    /// ("manifest v3"), rooted at `cfg.dist_root_url`. Versioned manifests
    /// (`1.93.0`, `1.94.0-beta.2`) are located through the TUF repository,
    /// see [`versioned_manifest_v3_url`].
    ///
    /// The layout is:
    ///
    /// ```text
    /// channels/
    /// ├── beta/
    /// │   ├── 1.75-beta-2023-11-13.toml
    /// │   ├── 2026-09-11.toml
    /// │   └── ...
    /// ├── current/
    /// │   ├── beta.toml
    /// │   ├── nightly.toml
    /// │   └── stable.toml
    /// ├── nightly/
    /// │   └── 2026/
    /// │       ├── 01-01/
    /// │       │   └── nightly.toml
    /// │       └── ...
    /// └── stable/
    ///     ├── 1.98.1.toml
    ///     ├── 2026-09-03.toml
    ///     └── ...
    /// ```
    ///
    /// The dev guide's TUF chapter describes the roles signing each subtree
    /// and how the versioned lookup searches them.
    pub(crate) async fn manifest_v3_url(&self, cfg: &Cfg<'_>) -> Result<String> {
        let dist_root = &cfg.dist_root_url;
        let do_manifest_staging = cfg.process.var("RUSTUP_STAGED_MANIFEST").is_ok();
        trace!(
            do_manifest_staging,
            channel = %self.channel,
            target = %self.target,
            "building v3 manifest url"
        );

        Ok(match (self.date.as_ref(), do_manifest_staging) {
            (None, false) => match &self.channel {
                Channel::Nightly | Channel::Beta | Channel::Stable => {
                    format!("{dist_root}/channels/current/{}.toml", self.channel)
                }
                Channel::Version(version) => versioned_manifest_v3_url(version, cfg).await?,
            },
            (Some(date), false) => {
                let date = NaiveDate::parse_from_str(date, "%Y-%m-%d")
                    .with_context(|| format!("invalid date '{date}', expected yyyy-mm-dd"))?;

                match &self.channel {
                    Channel::Beta | Channel::Stable => {
                        format!(
                            "{dist_root}/channels/{}/{}.toml",
                            self.channel,
                            date.format("%Y-%m-%d"),
                        )
                    }
                    Channel::Nightly => {
                        format!(
                            "{dist_root}/channels/nightly/{}/nightly.toml",
                            date.format("%Y/%m-%d")
                        )
                    }
                    Channel::Version(version) => versioned_manifest_v3_url(version, cfg).await?,
                }
            }
            (None, true) => format!("{dist_root}/channels/staging/{}.toml", self.channel),
            (Some(_), true) => panic!("not a real-world case"),
        })
    }
}

/// The URL of the channel manifest for `version`, found by asking the TUF
/// repository for `<version>.toml` under `channels/stable/` or, for a
/// pre-release version, `channels/beta/`. Versioned manifests are filed in
/// dated subfolders signed by delegated roles, so the actual path is only
/// known to the repository's metadata; the newest match wins.
async fn versioned_manifest_v3_url(version: &PartialVersion, cfg: &Cfg<'_>) -> Result<String> {
    // A pre-release version is a beta; everything else is a stable release.
    let folder = match version.pre.is_empty() {
        true => Path::new("channels/stable"),
        false => Path::new("channels/beta"),
    };
    let filename = format!("{version}.toml");
    debug!(folder = %folder.display(), filename, "locating versioned manifest through TUF");
    let target = cfg
        .tuf
        .repository(DownloadOptions::try_from(cfg.process)?)
        .await?
        .find_targets(folder, &filename)
        .await?
        .with_context(|| {
            format!(
                "no manifest for version '{version}' under '{}' in the TUF repository",
                folder.display()
            )
        })?;
    Ok(format!("{}/{target}", cfg.dist_root_url))
}
