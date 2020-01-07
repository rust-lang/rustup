
# Copy rustup-init to rustup-setup for backwards compatibility
cp target\${env:TARGET}\release\rustup-init.exe target\${env:TARGET}\release\rustup-setup.exe

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
