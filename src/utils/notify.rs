use std::fmt;

use tracing::Level;

#[derive(Debug)]
pub(crate) enum NotificationLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl fmt::Display for NotificationLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            NotificationLevel::Trace => "trace",
            NotificationLevel::Debug => "debug",
            NotificationLevel::Info => "info",
            NotificationLevel::Warn => "warn",
            NotificationLevel::Error => "error",
        })
    }
}

impl From<Level> for NotificationLevel {
    fn from(level: Level) -> Self {
        match level {
            Level::TRACE => Self::Trace,
            Level::DEBUG => Self::Debug,
            Level::INFO => Self::Info,
            Level::WARN => Self::Warn,
            Level::ERROR => Self::Error,
        }
    }
}
