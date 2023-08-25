use lazy_static::lazy_static;
use regex::Regex;

// These lists contain the targets known to rustup, and used to build
// the PartialTargetTriple.

static LIST_ARCHS: &[&str] = &[
    "aarch64",
    "arm",
    "armebv7r",
    "armv5te",
    "armv7",
    "armv7a",
    "armv7r",
    "armv7s",
    "asmjs",
    "avr",
    "bpfeb",
    "bpfel",
    "hexagon",
    "i386",
    "i586",
    "i686",
    "loongarch64",
    "mips",
    "mips64",
    "mips64el",
    "mipsel",
    "mipsisa32r6",
    "mipsisa32r6el",
    "msp430",
    "nvptx64",
    "powerpc",
    "powerpc64",
    "powerpc64le",
    "riscv32gc",
    "riscv32i",
    "riscv32im",
    "riscv32imac",
    "riscv32imc",
    "riscv64gc",
    "riscv64imac",
    "s390x",
    "sparc",
    "sparc64",
    "sparcv9",
    "thumbv4t",
    "thumbv5te",
    "thumbv6m",
    "thumbv7a",
    "thumbv7em",
    "thumbv7m",
    "thumbv7neon",
    "thumbv8m.base",
    "thumbv8m.main",
    "wasm32",
    "wasm64",
    "x86_64",
    "x86_64h",
];
static LIST_OSES: &[&str] = &[
    "apple-darwin",
    "apple-ios",
    "apple-tvos",
    "apple-watchos",
    "esp-espidf",
    "fortanix-unknown",
    "fuchsia",
    "ibm-aix",
    "kmc-solid_asp3",
    "linux",
    "nintendo-3ds",
    "nintendo-switch",
    "none",
    "nvidia-cuda",
    "openwrt-linux",
    "pc-nto",
    "pc-solaris",
    "pc-windows",
    "rumprun-netbsd",
    "sony-psp",
    "sony-psx",
    "sony-vita",
    "sun-solaris",
    "unknown-dragonfly",
    "unknown-emscripten",
    "unknown-freebsd",
    "unknown-fuchsia",
    "unknown-gnu",
    "unknown-haiku",
    "unknown-hermit",
    "unknown-illumos",
    "unknown-l4re",
    "unknown-linux",
    "unknown-netbsd",
    "unknown-none",
    "unknown-nto",
    "unknown-openbsd",
    "unknown-redox",
    "unknown-uefi",
    "unknown-unknown",
    "unknown-xous",
    "uwp-windows",
    "wasi",
    "wrs-vxworks",
];
static LIST_ENVS: &[&str] = &[
    "android",
    "androideabi",
    "atmega328",
    "eabi",
    "eabihf",
    "elf",
    "freestanding",
    "gnu",
    "gnu_ilp32",
    "gnuabi64",
    "gnueabi",
    "gnueabihf",
    "gnullvm",
    "gnuspe",
    "gnux32",
    "macabi",
    "msvc",
    "musl",
    "muslabi64",
    "musleabi",
    "musleabihf",
    "newlibeabihf",
    "ohos",
    "qnx700",
    "qnx710",
    "sgx",
    "sim",
    "softfloat",
    "spe",
    "uclibc",
    "uclibceabi",
    "uclibceabihf",
];

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
