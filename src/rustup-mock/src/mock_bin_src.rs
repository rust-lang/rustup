use std::process::Command;
use std::io::{self, BufWriter, Write};
use std::env::consts::EXE_SUFFIX;

fn main() {
    let args: Vec<_> = ::std::env::args().collect();
    if args.get(1) == Some(&"--version".to_string()) {
        println!("%EXAMPLE_VERSION% (%EXAMPLE_VERSION_HASH%)");
    } else if args.get(1) == Some(&"--empty-arg-test".to_string()) {
        assert!(args.get(2) == Some(&"".to_string()));
    } else if args.get(1) == Some(&"--huge-output".to_string()) {
        let out = io::stderr();
        let lock = out.lock();
        let mut buf = BufWriter::new(lock);
        for _ in 0 .. 10000 {
            buf.write_all(b"error: a value named `fail` has already been defined in this module [E0428]\n").unwrap();
        }
    } else if args.get(1) == Some(&"--call-rustc".to_string()) {
        // Used by the fallback_cargo_calls_correct_rustc test. Tests that
        // the environment has been set up right such that invoking rustc
        // will actually invoke the wrapper
        let rustc = &format!("rustc{}", EXE_SUFFIX);
        Command::new(rustc).arg("--version").status().unwrap();
    } else {
        panic!("bad mock proxy commandline");
    }
}
