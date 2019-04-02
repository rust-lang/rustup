use std::env;

fn main() {
    let target = env::var("TARGET").unwrap();
    println!("cargo:rustc-env=TARGET={}", target);
}
