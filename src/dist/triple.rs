use lazy_static::lazy_static;
use regex::Regex;

// These lists contain the targets known to rustup, and used to build
// the PartialTargetTriple.

static LIST_ARCHS: &[&str] = &[
    "i386",
    "i586",
    "i686",
    "x86_64",
    "arm",
    "armv7",
    "armv7s",
    "aarch64",
    "mips",
    "mipsel",
    "mips64",
    "mips64el",
    "powerpc",
    "powerpc64",
    "powerpc64le",
    "riscv64gc",
    "s390x",
    "loongarch64",
];
static LIST_OSES: &[&str] = &[
    "pc-windows",
    "unknown-linux",
    "apple-darwin",
    "unknown-netbsd",
    "apple-ios",
    "linux",
    "rumprun-netbsd",
    "unknown-freebsd",
    "unknown-illumos",
];
static LIST_ENVS: &[&str] = &[
    "gnu",
    "gnux32",
    "msvc",
    "gnueabi",
    "gnueabihf",
    "gnuabi64",
    "androideabi",
    "android",
    "musl",
];

#[derive(Debug, Clone, PartialEq, Eq)]
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
        lazy_static! {
            static ref PATTERN: String = format!(
                r"^(?:-({}))?(?:-({}))?(?:-({}))?$",
                LIST_ARCHS.join("|"),
                LIST_OSES.join("|"),
                LIST_ENVS.join("|")
            );
            static ref RE: Regex = Regex::new(&PATTERN).unwrap();
        }
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
    use rustup_macros::unit_test as test;

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
