# MSVC prerequisites

To compile programs into an exe file, Rust requires a linker, libraries and Windows API import libraries.
For `msvc` targets these can be acquired through Visual Studio.

## Automatically through the installer

If you don't have Visual Studio already installed then the [rustup-init.exe](https://rust-lang.org/tools/install/) installer will offer to automatically install the prerequisites. However, it installs Visual Studio Community edition which may not be appropriate for all users.

## Manual install

If you only want to install the bare essentials you could install only the Build Tools and the required components.

First you need to get the VisualStudio Installer to download the components:
```Batchfile
winget install --id Microsoft.VisualStudio.BuildTools
```
Then you need to install the individual components. Either via a simple cmd command (opens the GUI): 
```Batchfile
"%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vs_installer.exe" modify --productId Microsoft.VisualStudio.Product.BuildTools --channelId VisualStudio.18.Release --add Microsoft.VisualStudio.Component.VC.Tools.x86.x64 --add Microsoft.VisualStudio.Component.Windows11SDK.26100 --passive
```
Or via the Visual Studio Installer GUI by selecting the individual components yourself:
* MSVC Build Tools for x64/x86 (Latest)
* Windows 11 SDK (10.0.26100.XXXX)

Once finished, you can continue on to installing Rust and the installer should detect MSVC.
