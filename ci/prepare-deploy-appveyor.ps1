if ($env:APPVEYOR_PULL_REQUEST_NUMBER) {
   exit 0
}

if ($env:APPVEYOR_REPO_BRANCH -eq "auto") {
   exit 0
}

# Don't do anything for MSI (yet)
if ($env:BUILD_MSI) {
   exit 0
}

# Copy rustup-init to rustup-setup for backwards compatibility
cp target\${env:TARGET}\release\rustup-init.exe target\${env:TARGET}release\rustup-setup.exe

# Generate hashes
Get-FileHash .\target\${env:TARGET}\release\* | ForEach-Object {[io.file]::WriteAllText($_.Path + ".sha256", $_.Hash.ToLower() + "`n")}

# Prepare bins for upload
$dest = "dist\$env:TARGET"
md -Force "$dest"
cp target\${env:TARGET}\release\rustup-init.exe "$dest/"
cp target\${env:TARGET}\release\rustup-init.exe.sha256 "$dest/"
cp target\${env:TARGET}\release\rustup-setup.exe "$dest/"
cp target\${env:TARGET}\release\rustup-setup.exe.sha256 "$dest/"

ls "$dest"
