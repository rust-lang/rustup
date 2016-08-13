use std::env;

fn main() {
    println!("cargo:rustc-link-lib=static=wcautil");
    println!("cargo:rustc-link-lib=static=dutil");
    println!("cargo:rustc-link-lib=dylib=msi");
    println!("cargo:rustc-link-lib=dylib=user32");
    println!("cargo:rustc-link-lib=dylib=mincore");

    let wix_path = env::var("WIX").unwrap();
    // x86 target is hard-coded because we only build an x86 installer (works just fine on x64)
    println!("cargo:rustc-link-search=native={}SDK\\VS2015\\lib\\x86", wix_path);
}