use crate::dist::dist::TargetTriple;
use crate::Verbosity;
use log::{debug, warn};

#[derive(Debug)]
pub enum Notification<'a> {
    Utils(crate::utils::Notification<'a>),

    FileAlreadyDownloaded,
    CachedFileChecksumFailed,
    ComponentUnavailable(&'a str, Option<&'a TargetTriple>),
}

impl<'a> From<crate::utils::Notification<'a>> for Notification<'a> {
    fn from(n: crate::utils::Notification<'a>) -> Notification<'a> {
        Notification::Utils(n)
    }
}

impl<'a> Notification<'a> {
    pub fn log_with_verbosity(&self, verbosity: Verbosity) {
        use self::Notification::*;
        match self {
            Notification::Utils(n) => n.log_with_verbosity(verbosity),
            FileAlreadyDownloaded => match verbosity {
                Verbosity::Verbose => debug!("reusing previously downloaded file"),
                Verbosity::NotVerbose => (),
            },
            CachedFileChecksumFailed => warn!("bad checksum for cached download"),
            ComponentUnavailable(pkg, toolchain) => {
                if let Some(tc) = toolchain {
                    warn!(
                        "component '{}' is not available anymore on target '{}'",
                        pkg, tc
                    )
                } else {
                    warn!("component '{}' is not available anymore", pkg)
                }
            }
        }
    }
}
