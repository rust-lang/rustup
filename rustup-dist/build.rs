use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let target = env::var("TARGET").unwrap();

    File::create(out_dir.join("target.txt")).unwrap().write_all(target.as_bytes()).unwrap();
    println!("cargo:rerun-if-changed=build.rs");
}
