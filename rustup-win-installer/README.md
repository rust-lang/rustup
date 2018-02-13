# How to build

Important: For all `cargo build` invocations, set `--target` (even if the target is the same as the host architecture), because that affects the output directory. Pass the same target also via `-Target` to `build.ps1` in step 3.

## Steps

1) Build the main project with the `--features "msi-installed"` flag, resulting in `rustup-init.exe`
2) Build the CustomAction DLL in `src/rustup-win-installer` using `cargo build`
3) Build the actual installer in `src/rustup-win-installer/msi` using `build.ps1`

The resulting installer will be in `src/rustup-win-installer/msi/target`.