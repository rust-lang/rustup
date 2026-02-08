# MSVC prerequisites

To compile programs into an exe file, Rust requires a linker, libraries and Windows API import libraries.
For `msvc` targets these can be acquired through Visual Studio.

## Automatic install

If you don't have Visual Studio already installed then the [rustup-init.exe][rustinstall] installer will offer to automatically install the prerequisites. 

## WinGet

Alternatively Visual Studio can be installed via the [WinGet] package manager, which should be avaliable by default on recent versions of Windows.
Run the following command in powershell or the command prompt:

```
winget install --id Microsoft.VisualStudio.2022.Community --source winget --force --override "--add Microsoft.VisualStudio.Component.VC.Tools.x86.x64 --add Microsoft.VisualStudio.Component.VC.Tools.ARM64 --add Microsoft.VisualStudio.Component.Windows11SDK.22621 --addProductLang En-us"
```

You can replace "Community" with "BuildTools" in the above command if you already have a Visual Studio license.

### Missing Windows SDK

If after running the above command the Windows 11 SDK is missing then you may need to manually install it, which can also be done via WinGet.
First search for the avaliable versions:

```
winget search --source winget --id Microsoft.WindowsSDK.
```

This should display a table of results. that will look like this:

```
Name                                                       Id                              Version
----------------------------------------------------------------------------------------------------------
Windows Software Development Kit                           Microsoft.WindowsSDK.10.0.22000 10.0.22000.832
Windows Software Development Kit - Windows 10.0.22621.2428 Microsoft.WindowsSDK.10.0.22621 10.0.22621.2428
Windows Software Development Kit - Windows 10.0.26100.4188 Microsoft.WindowsSDK.10.0.26100 10.0.26100.4188
```

Pick the Id with the latest version and install that via `winget install`.
For example, to install `Microsoft.WindowsSDK.10.0.26100` run:

```
winget install --source winget --id Microsoft.WindowsSDK.10.0.26100 
```

[WinGet]: https://learn.microsoft.com/en-us/windows/package-manager/winget/

## Manual install

If you chose the automatic install this winget command will run in the background:
```Batchfile
winget install --id Microsoft.VisualStudio.BuildTools --force --interactive  --custom "--focusedUi --addProductLang En-us --add Microsoft.VisualStudio.Component.VC.Tools.x86.x64 --add Microsoft.VisualStudio.Component.Windows11SDK.26100"
```

But you could [download the Microsoft Build Tools for Visual Studio][vs download] Installer yourself and do the same steps manually.

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