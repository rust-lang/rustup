use std::{fmt, sync::LazyLock};

use anyhow::anyhow;
use regex::Regex;

use super::TargetTuple;

pub mod known;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PartialTargetTuple {
    pub arch: Option<String>,
    pub os: Option<String>,
    pub env: Option<String>,
}

impl PartialTargetTuple {
    pub(crate) fn new(name: &str) -> Option<Self> {
        if name.is_empty() {
            return Some(Self {
                arch: None,
                os: None,
                env: None,
            });
        }

        // Prepending `-` makes this next regex easier since
        // we can count on all tuple components being
        // delineated by it.
        let name = format!("-{name}");
        static RE: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(&format!(
                r"^(?:-({}))?(?:-({}))?(?:-({}))?$",
                known::LIST_ARCHS.join("|"),
                known::LIST_OSES.join("|"),
                known::LIST_ENVS.join("|")
            ))
            .unwrap()
        });

        RE.captures(&name).map(|c| {
            fn fn_map(s: &str) -> Option<String> {
                if s.is_empty() {
                    None
                } else {
                    Some(s.to_owned())
                }
            }

            Self {
                arch: c.get(1).map(|s| s.as_str()).and_then(fn_map),
                os: c.get(2).map(|s| s.as_str()).and_then(fn_map),
                env: c.get(3).map(|s| s.as_str()).and_then(fn_map),
            }
        })
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.arch.is_none() && self.env.is_none() && self.os.is_none()
    }

    /// Returns a full [`TargetTuple`] using `input_host` to fill in missing fields.
    pub(crate) fn complete(self, input_host: &TargetTuple) -> anyhow::Result<TargetTuple> {
        let host = Self::new(&input_host.0).ok_or_else(|| {
            anyhow!("provided host '{input_host}' couldn't be converted to partial tuple")
        })?;
        let host_arch = host.arch.ok_or_else(|| {
            anyhow!("provided host '{input_host}' did not specify a CPU architecture")
        })?;
        let host_os = host.os.ok_or_else(|| {
            anyhow!("provided host '{input_host}' did not specify an operating system")
        })?;
        let host_env = host.env;

        // If OS was specified, don't default to host environment, even if the OS matches
        // the host OS, otherwise cannot specify no environment.
        let env = match self.os {
            Some(_) => self.env,
            None => self.env.or(host_env),
        };
        let arch = self.arch.unwrap_or(host_arch);
        let os = self.os.unwrap_or(host_os);

        Ok(TargetTuple(match env {
            Some(env) => format!("{arch}-{os}-{env}"),
            None => format!("{arch}-{os}"),
        }))
    }
}

impl fmt::Display for PartialTargetTuple {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(arch) = &self.arch {
            write!(f, "{arch}")?;
        }
        if let Some(os) = &self.os {
            write!(f, "-{os}")?;
        }
        if let Some(env) = &self.env {
            write!(f, "-{env}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_partial_target_tuple_new() {
        let success_cases = vec![
            ("", (None, None, None)),
            ("i386", (Some("i386"), None, None)),
            ("pc-windows", (None, Some("pc-windows"), None)),
            ("gnu", (None, None, Some("gnu"))),
            ("i386-gnu", (Some("i386"), None, Some("gnu"))),
            ("pc-windows-gnu", (None, Some("pc-windows"), Some("gnu"))),
            ("i386-pc-windows", (Some("i386"), Some("pc-windows"), None)),
            (
                "i386-pc-windows-gnu",
                (Some("i386"), Some("pc-windows"), Some("gnu")),
            ),
        ];

        for (input, (arch, os, env)) in success_cases {
            let partial_target_tuple = PartialTargetTuple::new(input);
            assert!(
                partial_target_tuple.is_some(),
                "expected `{input}` to create some partial target tuple; got None"
            );

            let expected = PartialTargetTuple {
                arch: arch.map(String::from),
                os: os.map(String::from),
                env: env.map(String::from),
            };

            assert_eq!(partial_target_tuple.unwrap(), expected, "input: `{input}`");
        }

        let failure_cases = vec![
            "anything",
            "any-other-thing",
            "-",
            "--",
            "i386-",
            "i386-pc-",
            "i386-pc-windows-",
            "-pc-windows",
            "i386-pc-windows-anything",
            "0000-00-00-",
            "00000-000-000",
        ];

        for input in failure_cases {
            let partial_target_tuple = PartialTargetTuple::new(input);
            assert!(
                partial_target_tuple.is_none(),
                "expected `{input}` to be `None`, was: `{partial_target_tuple:?}`"
            );
        }
    }
}
