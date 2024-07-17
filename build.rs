use std::env;

use platforms::Platform;

fn from_build() -> Result<String, String> {
    let triple =
        env::var("RUSTUP_OVERRIDE_BUILD_TRIPLE").unwrap_or_else(|_| env::var("TARGET").unwrap());
    if Platform::ALL.iter().any(|p| p.target_triple == triple) {
        Ok(triple)
    } else {
        Err(triple)
    }
}

fn main() {
    println!("cargo:rerun-if-env-changed=RUSTUP_OVERRIDE_BUILD_TRIPLE");
    println!("cargo:rerun-if-env-changed=TARGET");
    match from_build() {
        Ok(triple) => eprintln!("Computed build based on target triple: {triple:#?}"),
        Err(s) => {
            eprintln!("Unable to parse target '{s}' as a known target triple");
            eprintln!(
                "If you are attempting to bootstrap a new target, you might need to update `platforms` to a newer version"
            );
            std::process::abort();
        }
    }
    let target = env::var("TARGET").unwrap();
    println!("cargo:rustc-env=TARGET={target}");

    // Set linker options specific to Windows MSVC.
    let target_os = env::var("CARGO_CFG_TARGET_OS");
    let target_env = env::var("CARGO_CFG_TARGET_ENV");
    if !(target_os.as_deref() == Ok("windows") && target_env.as_deref() == Ok("msvc")) {
        return;
    }

    // # Only search system32 for DLLs
    //
    // This applies to DLLs loaded at load time. However, this setting is ignored
    // before Windows 10 RS1 (aka 1601).
    // https://learn.microsoft.com/en-us/cpp/build/reference/dependentloadflag?view=msvc-170
    println!("cargo:cargo:rustc-link-arg-bin=rustup-init=/DEPENDENTLOADFLAG:0x800");

    // # Delay load
    //
    // Delay load dlls that are not "known DLLs"[1].
    // Known DLLs are always loaded from the system directory whereas other DLLs
    // are loaded from the application directory. By delay loading the latter
    // we can ensure they are instead loaded from the system directory.
    // [1]: https://learn.microsoft.com/en-us/windows/win32/dlls/dynamic-link-library-search-order#factors-that-affect-searching
    //
    // This will work on all supported Windows versions but it relies on
    // us using `SetDefaultDllDirectories` before any libraries are loaded.
    // See also: src/bin/rustup-init.rs
    let delay_load_dlls = ["bcrypt", "secur32"];
    for dll in delay_load_dlls {
        println!("cargo:rustc-link-arg-bin=rustup-init=/delayload:{dll}.dll");
    }
    // When using delayload, it's necessary to also link delayimp.lib
    // https://learn.microsoft.com/en-us/cpp/build/reference/dependentloadflag?view=msvc-170
    println!("cargo:rustc-link-arg-bin=rustup-init=delayimp.lib");

    // # Turn linker warnings into errors
    //
    // Rust hides linker warnings meaning mistakes may go unnoticed.
    // Turning them into errors forces them to be displayed (and the build to fail).
    // If we do want to ignore specific warnings then `/IGNORE:` should be used.
    println!("cargo:rustc-link-arg-bin=rustup-init=/WX");
}
