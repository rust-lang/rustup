use std::io::{self, BufWriter, Write};

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
    } else {
        panic!("bad mock proxy commandline");
    }
}
