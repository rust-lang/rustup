fn main() {
    let args: Vec<_> = ::std::env::args().collect();
    if args.get(1) == Some(&"--version".to_string()) {
        println!("%EXAMPLE_VERSION% (%EXAMPLE_VERSION_HASH%)");
    } else if args.get(1) == Some(&"--empty-arg-test".to_string()) {
        assert!(args.get(2) == Some(&"".to_string()));
    } else {
        panic!("bad mock proxy commandline");
    }
}
