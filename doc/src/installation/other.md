# Other installation methods

The primary installation method, as described at https://rustup.rs, differs by
platform:

* On Windows, download and run the [rustup-init.exe built for
  `i686-pc-windows-gnu` target][setup]. In general, this is the build of
  `rustup` one should install on Windows. Despite being built against the GNU
  toolchain, _the Windows build of `rustup` will install Rust for the MSVC
  toolchain if it detects that MSVC is installed_. If you prefer to install
  GNU toolchains or x86_64 toolchains by default this can be modified at
  install time, either interactively or with the `--default-host` flag, or
  after installation via `rustup set default-host`.
* On Unix, run `curl https://sh.rustup.rs -sSf | sh` in your shell. This
  downloads and runs [`rustup-init.sh`], which in turn downloads and runs the
  correct version of the `rustup-init` executable for your platform.

[setup]: https://static.rust-lang.org/rustup/dist/i686-pc-windows-gnu/rustup-init.exe
[`rustup-init.sh`]: https://static.rust-lang.org/rustup/rustup-init.sh

`rustup-init` accepts arguments, which can be passed through the shell script.
Some examples:

```console
$ curl https://sh.rustup.rs -sSf | sh -s -- --help
$ curl https://sh.rustup.rs -sSf | sh -s -- --no-modify-path
$ curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain nightly
$ curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain none
$ curl https://sh.rustup.rs -sSf | sh -s -- --profile minimal --default-toolchain nightly
```

If you prefer you can directly download `rustup-init` for the platform of your
choice:

- [aarch64-linux-android](https://static.rust-lang.org/rustup/dist/aarch64-linux-android/rustup-init)
- [aarch64-unknown-linux-gnu](https://static.rust-lang.org/rustup/dist/aarch64-unknown-linux-gnu/rustup-init)
- [arm-linux-androideabi](https://static.rust-lang.org/rustup/dist/arm-linux-androideabi/rustup-init)
- [arm-unknown-linux-gnueabi](https://static.rust-lang.org/rustup/dist/arm-unknown-linux-gnueabi/rustup-init)
- [arm-unknown-linux-gnueabihf](https://static.rust-lang.org/rustup/dist/arm-unknown-linux-gnueabihf/rustup-init)
- [armv7-linux-androideabi](https://static.rust-lang.org/rustup/dist/armv7-linux-androideabi/rustup-init)
- [armv7-unknown-linux-gnueabihf](https://static.rust-lang.org/rustup/dist/armv7-unknown-linux-gnueabihf/rustup-init)
- [i686-apple-darwin](https://static.rust-lang.org/rustup/dist/i686-apple-darwin/rustup-init)
- [i686-linux-android](https://static.rust-lang.org/rustup/dist/i686-linux-android/rustup-init)
- [i686-pc-windows-gnu](https://static.rust-lang.org/rustup/dist/i686-pc-windows-gnu/rustup-init.exe)
- [i686-pc-windows-msvc](https://static.rust-lang.org/rustup/dist/i686-pc-windows-msvc/rustup-init.exe)[^msvc]
- [i686-unknown-linux-gnu](https://static.rust-lang.org/rustup/dist/i686-unknown-linux-gnu/rustup-init)
- [mips-unknown-linux-gnu](https://static.rust-lang.org/rustup/dist/mips-unknown-linux-gnu/rustup-init)
- [mips64-unknown-linux-gnuabi64](https://static.rust-lang.org/rustup/dist/mips64-unknown-linux-gnuabi64/rustup-init)
- [mips64el-unknown-linux-gnuabi64](https://static.rust-lang.org/rustup/dist/mips64el-unknown-linux-gnuabi64/rustup-init)
- [mipsel-unknown-linux-gnu](https://static.rust-lang.org/rustup/dist/mipsel-unknown-linux-gnu/rustup-init)
- [powerpc-unknown-linux-gnu](https://static.rust-lang.org/rustup/dist/powerpc-unknown-linux-gnu/rustup-init)
- [powerpc64-unknown-linux-gnu](https://static.rust-lang.org/rustup/dist/powerpc64-unknown-linux-gnu/rustup-init)
- [powerpc64le-unknown-linux-gnu](https://static.rust-lang.org/rustup/dist/powerpc64le-unknown-linux-gnu/rustup-init)
- [s390x-unknown-linux-gnu](https://static.rust-lang.org/rustup/dist/s390x-unknown-linux-gnu/rustup-init)
- [x86_64-apple-darwin](https://static.rust-lang.org/rustup/dist/x86_64-apple-darwin/rustup-init)
- [x86_64-linux-android](https://static.rust-lang.org/rustup/dist/x86_64-linux-android/rustup-init)
- [x86_64-pc-windows-gnu](https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-gnu/rustup-init.exe)
- [x86_64-pc-windows-msvc](https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe)[^msvc]
- [x86_64-unknown-freebsd](https://static.rust-lang.org/rustup/dist/x86_64-unknown-freebsd/rustup-init)
- [x86_64-unknown-linux-gnu](https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init)
- [x86_64-unknown-linux-musl](https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-musl/rustup-init)
- [x86_64-unknown-netbsd](https://static.rust-lang.org/rustup/dist/x86_64-unknown-netbsd/rustup-init)

[^msvc]: MSVC builds of `rustup` additionally require an [installation of
    Visual Studio 2019 or the Visual C++ Build Tools 2019][vs]. For Visual
    Studio, make sure to check the "C++ tools" and "Windows 10 SDK" option. No
    additional software installation is necessary for basic use of the GNU
    build.

[vs]: https://visualstudio.microsoft.com/downloads/

You can fetch an older version from
`https://static.rust-lang.org/rustup/archive/{rustup-version}/{target-triple}/rustup-init[.exe]`

To install from source just run `cargo run --release`. Note that currently
`rustup` only builds on nightly Rust, and that after installation the `rustup`
toolchains will supersede any pre-existing toolchains by prepending
`~/.cargo/bin` to the `PATH` environment variable.
