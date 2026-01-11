# MSVC prerequisites

To compile programs into an exe file, Rust requires a linker, libraries and Windows API import libraries.
For `msvc` targets these can be acquired through Visual Studio.

## Automatic install

If you don't have Visual Studio already installed then the [rustup-init.exe][rustinstall] installer will offer to automatically install the prerequisites. 

## Manual install

If you chose the automatic install this winget command will run in the background:
```Batchfile
winget install --id Microsoft.VisualStudio.BuildTools --force --interactive  --custom "--focusedUi --addProductLang En-us --add Microsoft.VisualStudio.Component.VC.Tools.x86.x64 --add Microsoft.VisualStudio.Component.Windows11SDK.26100"
```

But you could [download the Microsoft Build Tools for Visual Studio][vs download] Installer yourself and do the same steps manual.

The installer will start by linking to the [Build Tools license][vs licences] and will install the "Visual Studio Installer", a tool to manage Visual Studio components.

![Accept the license](images/step1.png)


Then you need to install the "Individual components":
* MSVC Build Tools for x64/x86 (Latest)
* Windows 11 SDK (10.0.26100.XXXX)

And the "Language pack":
* English


Once finished, you can continue on to installing Rust and the installer should detect MSVC.

[vs licences]: https://visualstudio.microsoft.com/license-terms/vs2026-ga-diagnostic-buildtools
[vs download]: https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2026
[rustinstall]: https://rust-lang.org/tools/install/