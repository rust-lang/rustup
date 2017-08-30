if ($env:APPVEYOR_PULL_REQUEST_NUMBER) {
   exit 0
}

if ($env:APPVEYOR_REPO_BRANCH -eq "auto") {
   exit 0
}

# Prepare bins for upload
$dest = "dist\$env:TARGET"
md -Force "$dest"

if ($env:BUILD_MSI) {
    # Generate hash
    Get-FileHash .\src\rustup-win-installer\msi\target\rustup.msi | ForEach-Object {[io.file]::WriteAllText($_.Path + ".sha256", $_.Hash.ToLower() + "`n")}

    # NOTE: target\${env:TARGET}\release\rustup-init.exe also exists (built with the `msi-installed` feature flag),
    #       but doesn't need to be deployed, because it is embedded into the MSI.

    cp src\rustup-win-installer\msi\target\rustup.msi "$dest\"
    cp src\rustup-win-installer\msi\target\rustup.msi.sha256 "$dest\"
} else {
    # Copy rustup-init to rustup-setup for backwards compatibility
    cp target\${env:TARGET}\release\rustup-init.exe target\${env:TARGET}release\rustup-setup.exe

    # Generate hashes
    Get-FileHash .\target\${env:TARGET}\release\* | ForEach-Object {[io.file]::WriteAllText($_.Path + ".sha256", $_.Hash.ToLower() + "`n")}

    cp target\${env:TARGET}\release\rustup-init.exe "$dest\"
    cp target\${env:TARGET}\release\rustup-init.exe.sha256 "$dest\"
    cp target\${env:TARGET}\release\rustup-setup.exe "$dest\"
    cp target\${env:TARGET}\release\rustup-setup.exe.sha256 "$dest\"
}

ls "$dest"
