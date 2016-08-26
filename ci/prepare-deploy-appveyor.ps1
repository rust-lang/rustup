if ($env:APPVEYOR_PULL_REQUEST_NUMBER) {
   exit 0
}

if ($env:APPVEYOR_REPO_BRANCH -eq "auto") {
   exit 0
}

# Copy rustup-init to rustup-setup for backwards compatibility
cp target\${env:TARGET}\release\rustup-init.exe target\${env:TARGET}release\rustup-setup.exe

# Generate hashes
Get-FileHash .\target\${env:TARGET}\release\* | ForEach-Object {[io.file]::WriteAllText($_.Path + ".sha256", $_.Hash.ToLower() + "`n")}

# Prepare bins for upload
$dest = "dist\$env:TARGET"
md -Force "$dest"
if ($env:BUILD_MSI) {
    # MSI only needs the actual MSI installer and a hash of the embedded rustup.exe for the self update check
    # This hash is different from rustup-init.exe.sha256 because it is built with the `msi-installed` feature flag
    cp target\${env:TARGET}\release\rustup-init.exe.sha256 "$dest\rustup-msi.exe.sha256"
    cp src\rustup-win-installer\msi\target\rustup.msi "$dest\"
} else {
    cp target\${env:TARGET}\release\rustup-init.exe "$dest\"
    cp target\${env:TARGET}\release\rustup-init.exe.sha256 "$dest\"
    cp target\${env:TARGET}\release\rustup-setup.exe "$dest\"
    cp target\${env:TARGET}\release\rustup-setup.exe.sha256 "$dest\"
}

ls "$dest"
