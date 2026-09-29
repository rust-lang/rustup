use anyhow::{Context, Result};
use chrono::NaiveDate;
use tracing::trace;

use crate::{
    dist::{Channel, ChannelToolchainName},
    process::Process,
};

// The TUF-specific channel layout lives here rather than next to the v1 and
// v2 URL builders in `dist`, so the TUF code stays in one place.
impl ChannelToolchainName {
    /// The URL of this toolchain's channel manifest in the TUF target layout
    /// ("manifest v3"), rooted at `dist_root`.
    ///
    /// The layout is:
    ///
    /// ```text
    /// channels/
    /// ├── beta/
    /// │   ├── 1.75-beta-2023-11-13.toml
    /// │   └── ...
    /// ├── current/
    /// │   ├── beta.toml
    /// │   ├── nightly.toml
    /// │   └── stable.toml
    /// ├── nightly/
    /// │   └── 2018/
    /// │       ├── 01-01/
    /// │       │   └── nightly.toml
    /// │       ├── 01-02/
    /// │       │   ├── beta.toml
    /// │       │   └── nightly.toml
    /// │       └── ...
    /// └── stable/
    ///     ├── 1.10.0.toml
    ///     └── ...
    /// ```
    ///
    /// Open question: dated `stable`/`beta` requests currently resolve under
    /// `nightly/<date>/`. Serving them there means the nightly role signs
    /// stable and beta manifests, which erodes the per-channel role boundary;
    /// the alternative is publishing stable and beta both by version and by
    /// date.
    pub(crate) fn manifest_v3_url(&self, dist_root: &str, process: &Process) -> Result<String> {
        let do_manifest_staging = process.var("RUSTUP_STAGED_MANIFEST").is_ok();
        trace!(
            do_manifest_staging,
            channel = %self.channel,
            target = %self.target,
            "building v3 manifest url"
        );

        Ok(match (self.date.as_ref(), do_manifest_staging) {
            (None, false) => match &self.channel {
                Channel::Nightly | Channel::Beta | Channel::Stable => {
                    format!("{}/channels/current/{}.toml", dist_root, self.channel)
                }
                // A pre-release version is a beta; everything else is a
                // stable release.
                Channel::Version(version) if !version.pre.is_empty() => {
                    format!("{}/channels/beta/{}.toml", dist_root, self.channel)
                }
                Channel::Version(_) => {
                    format!("{}/channels/stable/{}.toml", dist_root, self.channel)
                }
            },
            (Some(date), false) => {
                let date = NaiveDate::parse_from_str(date, "%Y-%m-%d")
                    .with_context(|| format!("invalid date '{date}', expected yyyy-mm-dd"))?;

                match &self.channel {
                    Channel::Beta | Channel::Stable => {
                        format!(
                            "{}/channels/{}/{}.toml",
                            dist_root,
                            self.channel,
                            date.format("%Y-%m-%d"),
                        )
                    }
                    Channel::Nightly => {
                        format!(
                            "{}/channels/nightly/{}/nightly.toml",
                            dist_root,
                            date.format("%Y/%m-%d")
                        )
                    }
                    // A pre-release version is a beta; everything else is a
                    // stable release.
                    Channel::Version(version) if !version.pre.is_empty() => {
                        format!("{}/channels/beta/{}.toml", dist_root, self.channel)
                    }
                    Channel::Version(_) => {
                        format!("{}/channels/stable/{}.toml", dist_root, self.channel)
                    }
                }
            }
            (None, true) => format!("{}/channels/staging/{}.toml", dist_root, self.channel),
            (Some(_), true) => panic!("not a real-world case"),
        })
    }
}
