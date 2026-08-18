use std::{fmt, io::Write};

use anyhow::Result;
use clap::{ValueEnum, builder::PossibleValue};

use crate::{config::Cfg, utils::ExitCode};

pub(crate) fn problem_matcher(flavor: Flavor, cfg: &Cfg<'_>) -> Result<ExitCode> {
    print_str(flavor.problem_matcher(), cfg)
}

fn print_str(s: &str, cfg: &Cfg<'_>) -> Result<ExitCode> {
    let stdout = cfg.process.stdout();
    write!(stdout.lock(), "{s}")?;
    Ok(ExitCode::SUCCESS)
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum Flavor {
    Github,
}

impl Flavor {
    fn problem_matcher(&self) -> &'static str {
        match self {
            Self::Github => include_str!("ci/matcher/github.json"),
        }
    }
}

impl ValueEnum for Flavor {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Github]
    }

    fn to_possible_value<'a>(&self) -> Option<PossibleValue> {
        Some(match self {
            Self::Github => PossibleValue::new("github"),
        })
    }
}

impl fmt::Display for Flavor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.to_possible_value() {
            Some(v) => write!(f, "{}", v.get_name()),
            None => unreachable!(),
        }
    }
}
