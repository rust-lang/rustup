#!/usr/bin/pwsh

# This is just a little script that can be downloaded from the internet to
# install rustup. It just does platform detection, downloads the installer and
# runs it.

# XXX: If you change anything here, please make the same changes in
# `src/cli/setup_mode.rs` and `rustup-init.sh`. This keeps the usage output of
# the bootstrapping scripts consistent with the `rustup-init` executable.
function Write-Usage {
    Write-Host @'
rustup-init 1.18.3 (435397f48 2019-05-22)
The installer for rustup

USAGE:
    rustup-init.ps1 [FLAGS] [OPTIONS]

FLAGS:
    -v, --verbose           Enable verbose output
    -y                      Disable confirmation prompt.
        --no-modify-path    Don't configure the PATH environment variable
    -h, --help              Prints help information
    -V, --version           Prints version information

OPTIONS:
        --default-host <default-host>              Choose a default host triple
        --default-toolchain <default-toolchain>    Choose a default toolchain to install
'@
}

function Invoke-Main([AllowEmptyCollection()] [string[]]$Argv = @()) {
    foreach ($arg in $Argv) {
        if ("$arg" -eq "-h" -or "$arg" -eq "--help") {
            Write-Usage
            return
        }
    }

    Install-Rustup $Argv
}

function Get-Architecture {
    if (($IsWindows -eq $null) -or $IsWindows) {
        $ostype = "pc-windows-msvc"
    } elseif ($IsMacOS) {
        $ostype = "apple-darwin"
    } elseif ($IsLinux) {
        $ostype = "unknown-linux-gnu"
    } else {
        Write-Failure "unrecognized OS type"
    }

    if ([System.IntPtr]::Size -eq 8) {
        $cputype = "x86_64"
    } elseif ([System.IntPtr]::Size -eq 4) {
        $cputype = "i686"
    } else {
        Write-Failure "unknown CPU type"
    }

    $arch = "${cputype}-${ostype}"
    Write-Verbose "Detected arch '$arch'"
    $arch
}

# If RUSTUP_UPDATE_ROOT is unset, default it.
function Get-RustupUpdateRoot {
    $EnvVar = [System.Environment]::GetEnvironmentVariable("RUSTUP_UPDATE_ROOT")
    if (($EnvVar -ne $null) -and (Test-Path -Path $EnvVar)) {
        $EnvVar
    } else {
        "https://static.rust-lang.org/rustup"
    }
}

function Install-Rustup([AllowEmptyCollection()] [string[]]$Argv) {
    $arch = Get-Architecture
    switch -Wildcard ($arch) {
        "*windows*" { $ext = ".exe"; break }
        default { $ext = "" }
    }

    $url = "$(Get-RustupUpdateRoot)/dist/$arch/rustup-init$ext"

    $dir = New-TemporaryDirectory
    $file = Join-Path $dir "rustup-init$ext"

    try {
        Write-Verbose "Downloading $url to $file"
        Set-HighestEncryption
        (New-Object System.Net.WebClient).DownloadFile($url, $file)

        if ($IsMacOS -or $IsLinux) {
            Write-Verbose "Setting execute permission on $file"
            chmod u+x "$file"
        }

        Write-Verbose "Calling $file $Argv"
        & "$file" @Argv
    } finally {
        Write-Verbose "Cleaning $dir"
        Remove-Item "$dir" -Force -Recurse
    }
}

function New-TemporaryDirectory {
    $parent = [System.IO.Path]::GetTempPath()
    [string]$name = [System.Guid]::NewGuid()
    New-Item -ItemType Directory -Path (Join-Path $parent $name)
}

function Set-HighestEncryption {
    # Implementation adapted from the Chocolatey installer script,
    # see: https://chocolatey.org/install.ps1

    # Attempt to set highest encryption available for SecurityProtocol.
    # PowerShell will not set this by default (until maybe .NET 4.6.x). This
    # will typically produce a message for PowerShell v2 (just an info message
    # though)
    $OldSPM = [System.Net.ServicePointManager]::SecurityProtocol
    try {
        # Set TLS 1.2 (3072) which is currently the highest protocol enabled on
        # static.rust-lang.org. Favor TLS 1.3 (12288) with a TLS 1.2 fallback
        # when 1.3 is supported. Use integers because the enumeration values
        # for TLS 1.2 won't exist in .NET 4.0, even though they are addressable
        # if .NET 4.5+ is installed (.NET 4.5 is an in-place upgrade).
        [System.Net.ServicePointManager]::SecurityProtocol = 3072
    } catch {
        Write-Failure @'
rustup: Unable to set PowerShell to use TLS 1.2 due to old .NET Framework
installed. If you see underlying connection closed or trust errors, you may
need to do one or more of the following: (1) upgrade to .NET Framework 4.5+ and
PowerShell v3+, (2) download an alternative install method from
https://rustup.rs/.
'@
    } finally {
        [System.Net.ServicePointManager]::SecurityProtocol = $OldSPM
    }
}

function Write-Failure([string]$Message) {
    Write-Warning "rustup: $Message"
    throw
}

Invoke-Main $args
