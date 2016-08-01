fn main() {
    println!("cargo:rustc-link-lib=dylib=msi");
    println!("cargo:rustc-link-lib=dylib=wcautil");
    println!("cargo:rustc-link-lib=dylib=dutil");
    println!("cargo:rustc-link-lib=dylib=user32");
    println!("cargo:rustc-link-lib=dylib=mincore");

    // TODO: maybe don't hardcode path to WiX 3.10
    println!("cargo:rustc-link-search=native=C:\\Program Files (x86)\\WiX Toolset v3.10\\SDK\\VS2015\\lib\\x86");
}