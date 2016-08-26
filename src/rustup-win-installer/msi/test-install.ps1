# This script can be used for manually testing the MSI installer. It is not used for AppVeyor CI.

pushd ..\..\..
# Build rustup.exe
cargo build --release --target i686-pc-windows-msvc --features msi-installed
popd
if($LastExitCode -ne 0) { exit $LastExitCode }
pushd ..
# Build the CA library
cargo build --release --target i686-pc-windows-msvc
popd
if($LastExitCode -ne 0) { exit $LastExitCode }
# Build the MSI
.\build.ps1 -Target i686-pc-windows-msvc
if($LastExitCode -ne 0) { exit $LastExitCode }
# Run the MSI with logging
$OLD_CARGO_HOME = $env:CARGO_HOME
$OLD_RUSTUP_HOME = $env:RUSTUP_HOME
$env:CARGO_HOME = "$env:USERPROFILE\.cargo-test"
$env:RUSTUP_HOME = "$env:USERPROFILE\.rustup-test"
Start-Process msiexec -ArgumentList "/i target\rustup.msi /L*V target\Install.log" -Wait
$env:CARGO_HOME = $OLD_CARGO_HOME
$env:RUSTUP_HOME = $OLD_RUSTUP_HOME