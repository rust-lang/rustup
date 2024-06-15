use std::fmt;

use tracing::Level;

#[derive(Debug)]
pub(crate) enum NotificationLevel {
    Debug,
    Verbose,
    Info,
    Warn,
    Error,
}

impl fmt::Display for NotificationLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            NotificationLevel::Debug => "debug",
            NotificationLevel::Verbose => "verbose",
            NotificationLevel::Info => "info",
            NotificationLevel::Warn => "warning",
            NotificationLevel::Error => "error",
        })
    }
}

impl From<Level> for NotificationLevel {
    fn from(level: Level) -> Self {
        match level {
            Level::TRACE => Self::Debug,
            Level::DEBUG => Self::Verbose,
            Level::INFO => Self::Info,
            Level::WARN => Self::Warn,
            Level::ERROR => Self::Error,
        }
    }
}
