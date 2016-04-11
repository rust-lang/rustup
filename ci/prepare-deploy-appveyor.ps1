if ($env:APPVEYOR_PULL_REQUEST_NUMBER) {
   exit 0
}

if ($env:APPVEYOR_REPO_BRANCH -eq "auto") {
   exit 0
}

# Generate hashes
Get-FileHash .\target\release\* | ForEach-Object {[io.file]::WriteAllText($_.Path + ".sha256", $_.Hash.ToLower() + "`n")}

# Prepare bins for upload
$dest = "dist\$env:TARGET"
md -Force "$dest"
cp target\release\rustup-init.exe "$dest/"
cp target\release\rustup-init.exe.sha256 "$dest/"

ls "$dest"
