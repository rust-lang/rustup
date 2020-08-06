# Windows

`rustup` works the same on Windows as it does on Unix, but there are some
special considerations for Rust developers on Windows. As [mentioned on the
Rust download page][msvc-toolchain], there are two [ABIs] in use on Windows:
the native (MSVC) ABI used by [Visual Studio], and the GNU ABI used by the
[GCC toolchain]. Which version of Rust you need depends largely on what C/C++
libraries you want to interoperate with: for interop with software produced by
Visual Studio use the MSVC build of Rust; for interop with GNU software built
using the [MinGW/MSYS2 toolchain] use the GNU build.

When targeting the MSVC ABI, Rust additionally requires an [installation of
Visual Studio 2013 (or later) or the Visual C++ Build Tools 2019][vs] so
`rustc` can use its linker. For Visual Studio, make sure to check the "C++
tools" and "Windows 10 SDK" option. No additional software installation is
necessary for basic use of the GNU build.

By default `rustup` on Windows configures Rust to target the MSVC ABI, that is
a target triple of either `i686-pc-windows-msvc` or `x86_64-pc-windows-msvc`
depending on the CPU architecture of the host Windows OS. The toolchains that
`rustup` chooses to install, unless told otherwise through the [toolchain
specification], will be compiled to run on that target triple host and will
target that triple by default.

You can change this behavior with `rustup set default-host` or during
installation.

For example, to explicitly select the 32-bit MSVC host:

```console
$ rustup set default-host i686-pc-windows-msvc
```

Or to choose the 64 bit GNU toolchain:

```console
$ rustup set default-host x86_64-pc-windows-gnu
```

Since the MSVC ABI provides the best interoperation with other Windows
software it is recommended for most purposes. The GNU toolchain is always
available, even if you don't use it by default. Just install it with `rustup
toolchain install`:

```console
$ rustup toolchain install stable-gnu
```

You don't need to switch toolchains to support all windows targets though; a
single toolchain supports all four x86 windows targets:

```console
$ rustup target add x86_64-pc-windows-msvc
$ rustup target add x86_64-pc-windows-gnu
$ rustup target add i686-pc-windows-msvc
$ rustup target add i686-pc-windows-gnu
```

See the [Cross-compilation] chapter for more details on specifying different
targets with the same compiler.

[ABIs]: https://en.wikipedia.org/wiki/Application_binary_interface
[cross-compilation]: ../cross-compilation.md
[GCC toolchain]: https://gcc.gnu.org/
[MinGW/MSYS2 toolchain]: https://msys2.github.io/
[msvc-toolchain]: https://www.rust-lang.org/tools/install?platform_override=win
[toolchain specification]: ../concepts/toolchains.md#toolchain-specification
[Visual Studio]: https://visualstudio.microsoft.com/visual-cpp-build-tools/
[vs]: https://visualstudio.microsoft.com/downloads/
