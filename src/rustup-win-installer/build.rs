extern crate gcc;

use std::env;

fn main() {
    println!("cargo:rustc-link-lib=dylib=msi");
    println!("cargo:rustc-link-lib=dylib=user32");
    println!("cargo:rustc-link-lib=dylib=mincore");

    // Part of WIX SDK
    println!("cargo:rustc-link-lib=static=wcautil");
    println!("cargo:rustc-link-lib=static=dutil");

    let wix_path = env::var("WIX").expect("WIX must be installed, and 'WIX' environment variable must be set");

    // For the correct WIX library path, we need to know which VS version we are using.
    // We use the `gcc` crate to get the path to the correct `cl.exe`, and then try to
    // guess the version from the path components (we don't depend on the actual location
    // of the VS installation, but only on the internal directory structure).
    let config = gcc::Config::new();
    let compiler = config.get_compiler();
    let path = compiler.path();
    let vs_version = if path.to_string_lossy().contains("VC\\bin") {
        println!("cargo:warning=Guessing VS version: VS2015");
        "VS2015"
    } else if path.to_string_lossy().contains("VC\\Tools\\MSVC") {
        println!("cargo:warning=Guessing VS version: VS2017");
        "VS2017"
    } else {
        panic!("Unknown VS version with compiler path {:?}", path);
    };

    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").expect("cannot read CARGO_CFG_TARGET_ARCH in build script");
    let target_arch = match target_arch.as_str() {
        "x86" => "x86",
        "x86_64" => "x64",
        other => panic!("Target architecture {} not supported by WIX.", other)
    };
    
    // Tell cargo about the WIX SDK path for `wcautil.lib` and `dutil.lib`
    println!("cargo:rustc-link-search=native={}SDK\\{}\\lib\\{}", wix_path, vs_version, target_arch);
}