use std::env;

fn main() {
	let target = env::var("TARGET").unwrap();
	
	if target.ends_with("-pc-windows-gnu") {
		println!("cargo:rustc-link-lib=dylib=gdi32");
		println!("cargo:rustc-link-lib=dylib=user32");
	}
}