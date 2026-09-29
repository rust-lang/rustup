use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use chrono::{DateTime, Utc};

use crate::{
    process::TestProcess,
    tuf::{
        TufConfig, TufMode,
        config::{LazyTufRepository, parse_date_time},
    },
};

fn config(vars: &[(&str, &str)]) -> TufConfig {
    let vars = vars
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect::<HashMap<_, _>>();
    let tp = TestProcess::with_vars(vars);
    TufConfig::from_env(Path::new("/rustup"), &tp.process)
}

#[test]
fn defaults() {
    assert_eq!(
        config(&[]),
        TufConfig {
            server: "https://storage.googleapis.com/tufops".to_owned(),
            root: None,
            home: PathBuf::from("/rustup/tuf"),
            mode: TufMode::Off,
            ignore_failures: false,
            ignore_expiry_after: None,
            repository: LazyTufRepository::default(),
        }
    );
}

#[test]
fn all_set() {
    let cfg = config(&[
        ("RUSTUP_TUF_SERVER", "https://tuf.example.com"),
        ("RUSTUP_TUF_ROOT", "/etc/rustup/root.json"),
        ("RUSTUP_TUF_HOME", "/var/lib/rustup-tuf"),
        ("RUSTUP_TUF_ENABLE", "warn"),
        ("RUSTUP_TUF_IGNORE", "1"),
        ("RUSTUP_TUF_IGNOREDATE", "2026-09-16T12:34:56Z"),
    ]);
    assert_eq!(
        cfg,
        TufConfig {
            server: "https://tuf.example.com".to_owned(),
            root: Some(PathBuf::from("/etc/rustup/root.json")),
            home: PathBuf::from("/var/lib/rustup-tuf"),
            mode: TufMode::Warn,
            ignore_failures: true,
            ignore_expiry_after: "2026-09-16T12:34:56Z".parse::<DateTime<Utc>>().ok(),
            repository: LazyTufRepository::default(),
        }
    );
}

#[test]
fn mode_is_case_insensitive_and_lenient() {
    assert_eq!(config(&[("RUSTUP_TUF_ENABLE", "on")]).mode, TufMode::On);
    assert_eq!(config(&[("RUSTUP_TUF_ENABLE", "ON")]).mode, TufMode::On);
    assert_eq!(config(&[("RUSTUP_TUF_ENABLE", "true")]).mode, TufMode::On);
    assert_eq!(config(&[("RUSTUP_TUF_ENABLE", "1")]).mode, TufMode::On);
    assert_eq!(config(&[("RUSTUP_TUF_ENABLE", "Warn")]).mode, TufMode::Warn);
    assert_eq!(config(&[("RUSTUP_TUF_ENABLE", "false")]).mode, TufMode::Off);
    assert_eq!(config(&[("RUSTUP_TUF_ENABLE", "off")]).mode, TufMode::Off);
    assert_eq!(config(&[("RUSTUP_TUF_ENABLE", "maybe")]).mode, TufMode::Off);
    assert_eq!(config(&[("RUSTUP_TUF_ENABLE", "")]).mode, TufMode::Off);
}

#[test]
fn ignore_flag_is_only_one() {
    assert!(config(&[("RUSTUP_TUF_IGNORE", "1")]).ignore_failures);
    for other in ["0", "", "true", "yes", "2"] {
        assert!(
            !config(&[("RUSTUP_TUF_IGNORE", other)]).ignore_failures,
            "{other:?}"
        );
    }
}

#[test]
fn date_parsing() {
    let midnight = "2026-09-16T00:00:00Z".parse::<DateTime<Utc>>().unwrap();
    assert_eq!(parse_date_time("2026-09-16"), Some(midnight));
    assert_eq!(parse_date_time("2026-09-16T02:00:00+02:00"), Some(midnight));
    assert_eq!(parse_date_time("yesterday"), None);
    assert_eq!(parse_date_time("2026-13-01"), None);
    assert_eq!(
        config(&[("RUSTUP_TUF_IGNOREDATE", "yesterday")]).ignore_expiry_after,
        None
    );
}
