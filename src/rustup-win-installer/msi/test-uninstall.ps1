# Uninstall currently installed version of rustup. Does the same thing as `rustup self uninstall`.

$key = 'HKCU:\SOFTWARE\rustup'
$productCode = (Get-ItemProperty -Path $key -Name InstalledProductCode).InstalledProductCode

# No need to set CARGO_HOME, because the installation directory is stored in the registry
$OLD_RUSTUP_HOME = $env:RUSTUP_HOME
$env:RUSTUP_HOME = "$env:USERPROFILE\.rustup-test"
msiexec /x "$productCode" /L*V "target\Uninstall.log"
$env:RUSTUP_HOME = $OLD_RUSTUP_HOME