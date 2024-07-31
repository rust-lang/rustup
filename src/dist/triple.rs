use std::sync::LazyLock;

use regex::Regex;

pub mod known;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PartialTargetTriple {
    pub arch: Option<String>,
    pub os: Option<String>,
    pub env: Option<String>,
}

impl PartialTargetTriple {
    pub(crate) fn new(name: &str) -> Option<Self> {
        if name.is_empty() {
            return Some(Self {
                arch: None,
                os: None,
                env: None,
            });
        }

        // Prepending `-` makes this next regex easier since
        // we can count  on all triple components being
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
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_partial_target_triple_new() {
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
            let partial_target_triple = PartialTargetTriple::new(input);
            assert!(
                partial_target_triple.is_some(),
                "expected `{input}` to create some partial target triple; got None"
            );

            let expected = PartialTargetTriple {
                arch: arch.map(String::from),
                os: os.map(String::from),
                env: env.map(String::from),
            };

            assert_eq!(partial_target_triple.unwrap(), expected, "input: `{input}`");
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
            let partial_target_triple = PartialTargetTriple::new(input);
            assert!(
                partial_target_triple.is_none(),
                "expected `{input}` to be `None`, was: `{partial_target_triple:?}`"
            );
        }
    }
}
