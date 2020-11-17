use std::env;

include!("src/dist/triple.rs");

pub fn from_build() -> Result<PartialTargetTriple, String> {
    let triple = if let Ok(triple) = env::var("RUSTUP_OVERRIDE_BUILD_TRIPLE") {
        triple
    } else {
        env::var("TARGET").unwrap()
    };
    PartialTargetTriple::new(&triple).ok_or(triple)
}

fn main() {
    println!("cargo:rerun-if-env-changed=RUSTUP_OVERRIDE_BUILD_TRIPLE");
    println!("cargo:rerun-if-env-changed=TARGET");
    match from_build() {
        Ok(triple) => eprintln!("Computed build based partial target triple: {:#?}", triple),
        Err(s) => {
            eprintln!("Unable to parse target '{}' as a PartialTargetTriple", s);
            eprintln!(
                "If you are attempting to bootstrap a new target you may need to adjust the\n\
               permitted values found in src/dist/triple.rs"
            );
            std::process::abort();
        }
    }
    let target = env::var("TARGET").unwrap();
    println!("cargo:rustc-env=TARGET={}", target);
}
