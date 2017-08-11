use std::env::consts::EXE_SUFFIX;
use std::env;
use std::io::{self, Write};
use std::process::Command;

fn main() {
    let mut args = env::args().skip(1);
    match args.next().as_ref().map(|s| &**s) {
        Some("--version") => {
            let version = env!("EXAMPLE_VERSION");
            let hash = env!("EXAMPLE_VERSION_HASH");
            println!("{} ({})", version, hash);
        }
        Some("--empty-arg-test") => {
            assert_eq!(args.next().unwrap(), "");
        }
        Some("--huge-output") => {
            let mut out = io::stderr();
            for _ in 0 .. 10000 {
                out.write_all(b"error: a value named `fail` has already been defined in this module [E0428]\n").unwrap();
            }
        }
        Some("--call-rustc") => {
            // Used by the fallback_cargo_calls_correct_rustc test. Tests that
            // the environment has been set up right such that invoking rustc
            // will actually invoke the wrapper
            let rustc = &format!("rustc{}", EXE_SUFFIX);
            Command::new(rustc).arg("--version").status().unwrap();
        }
        _ => panic!("bad mock proxy commandline"),
    }
}
