$manifest = cargo read-manifest --manifest-path ..\..\..\Cargo.toml | ConvertFrom-Json
$version = $manifest.version.Split(".")
$env:CFG_VER_MAJOR = $version[0]
$env:CFG_VER_MINOR = $version[1]
$env:CFG_VER_PATCH = $version[2]

foreach($file in Get-ChildItem *.wxs) {
    &'C:\Program Files (x86)\WiX Toolset v3.10\bin\candle.exe' -nologo -arch x86 -ext WixUIExtension -ext WixUtilExtension -out "target\$($file.Name.Replace(".wxs",".wixobj"))" $($file.Name)
    if ($LASTEXITCODE -ne 0) { exit }
}

# ICE57 wrongly complains about per-machine data in per-user install, because it doesn't know that INSTALLLOCATION is in per-user directory
&'C:\Program Files (x86)\WiX Toolset v3.10\bin\light.exe' -nologo -ext WixUIExtension -ext WixUtilExtension -out "target\rustup.msi" -sice:ICE57 $(Get-ChildItem target\*.wixobj)
