# MSVC prerequisites

To compile programs into an exe file, Rust requires a linker, libraries and Windows API import libraries.
For `msvc` targets these can be acquired through Visual Studio.

## Automatically through the installer

If you don't have Visual Studio already installed then the [rustup-init.exe](https://rust-lang.org/tools/install/) installer will offer to automatically install the prerequisites. However, it installs Visual Studio Community edition which may not be appropriate for all users.

## Manual install

You could also install only the Visual Studio Build Tools and the required components.

You can dowload the official Microsoft Visual Studio Installer via winget:
```Batchfile
winget install Microsoft.VisualStudio.BuildTools --interactive --custom "--add Microsoft.VisualStudio.Component.VC.Tools.x86.x64 --add Microsoft.VisualStudio.Component.Windows11SDK.26100"
```
The installer will start by linking to the [Build Tools license][vs licences] and will install the "Visual Studio Installer".

![Accept the license](images/step1.png)
![Installing the installer](images/step2.png)

Then you need to install the individual components:
* MSVC Build Tools for x64/x86 (Latest)
* Windows 11 SDK (10.0.26100.XXXX)

If you used the winget-install method above, the 2 components are already selected and ready for "Install".
If not, you can find them under "Individual Components" if you "Modify" your existing Visual Studio setup.

Once finished, you can continue on to installing Rust and the installer should detect MSVC.

[vs licences]: https://visualstudio.microsoft.com/license-terms/vs2026-ga-diagnostic-buildtools/
