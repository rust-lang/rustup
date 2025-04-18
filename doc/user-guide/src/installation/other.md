# Other installation methods

The primary installation method, as described at <https://rustup.rs>, differs
by platform:

* On Windows, download and run the `rustup-init.exe` built for the
  [`x86_64-pc-windows-msvc`] or [`aarch64-pc-windows-msvc`] target,
  depending on your OS architecture. In general, this is the build of
  `rustup` one should install on Windows. This will require the Visual C++
  Build Tools 2019 or equivalent (Visual Studio 2019, etc.) to already be
  installed. If you would prefer to install GNU toolchains or the i686
  toolchains by default this can be modified at install time, either
  interactively, with the `--default-host` flag, or after installation
  via `rustup set default-host`.
* On Unix, run `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh` in your shell. This
  downloads and runs [`rustup-init.sh`], which in turn downloads and runs the
  correct version of the `rustup-init` executable for your platform.

[`x86_64-pc-windows-msvc`]: https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe
[`aarch64-pc-windows-msvc`]: https://static.rust-lang.org/rustup/dist/aarch64-pc-windows-msvc/rustup-init.exe
[`rustup-init.sh`]: https://static.rust-lang.org/rustup/rustup-init.sh

`rustup-init` accepts arguments, which can be passed through the shell script.
Some examples:

```console
$ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --help
$ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --no-modify-path
$ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --default-toolchain nightly
$ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --default-toolchain none
$ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --profile minimal --default-toolchain nightly
```


## Using a package manager

> Please note that the rustup project is not maintaining any package mentioned in this section.
> If you have encountered any problems installing `rustup` with a package manager,
> please contact the package maintainer(s) for further information.

### General tips

Different package managers take slightly different approaches towards managing rustup.
After installing rustup with your favorite package manager, there are usually two possibilities:

- If your package manager has made the `rustup` command available
  together with proxies for Rust tools such as `rustc` and `cargo`,
  picking a default toolchain (e.g. `stable`) would usually be enough:

  ```sh
  $ rustup default stable
  ```

  > As of 2024/12/23, this is the case for
  > [APT](https://packages.debian.org/search?searchon=names&keywords=rustup),
  > [homebrew](https://formulae.brew.sh/formula/rustup)
  > and [pacman](https://wiki.archlinux.org/title/Rust#Arch_Linux_package).

- If your package manager has only made the `rustup-init` command available, simply run:

  ```sh
  $ rustup-init
  ```

  This will allow you to perform the initial setup of `rustup`, populate all the proxies
  managed by rustup, and install a default toolchain.

  > As of 2024/12/23, this is the case for
  > [DNF](https://developer.fedoraproject.org/tech/languages/rust/further-reading.html).

When the installation is completed, please make sure that the rustup proxies
(usually under `$HOME/.cargo/bin`) are [correctly exposed] via your `$PATH`,
and you should be able to run `rustup`, `rustc`, `cargo`, etc. normally.

[correctly exposed]: ./already-installed-rust.html#ensure-the-correct-path-configuration

### APT

Starting from Debian 13 (trixie) and Ubuntu 24.04 (noble),
you may use `apt` to install `rustup`:

```sh
$ sudo apt install rustup
```

### Homebrew

You can use `brew` to install `rustup`[^not-rust]:

```sh
$ brew install rustup
```

Please note that Rust tools like `rustc` and `cargo` are not available via `$PATH` by default
in this `rustup` distribution
(see [homebrew-core#177582](https://github.com/Homebrew/homebrew-core/pull/177582) for more details).
You might want to add `$(brew --prefix rustup)/bin` to `$PATH` to make them easier to access.

[^not-rust]: This is not to be confused with the `rust` package,
which is a `brew`-managed `rust` toolchain installation.

## Manual installation

You can manually download `rustup-init` for a given target from
`https://static.rust-lang.org/rustup/dist/{target-triple}/rustup-init[.exe]`[^msys2] [^msvc].

<details>
<summary>Direct links</summary>

- [aarch64-apple-darwin](https://static.rust-lang.org/rustup/dist/aarch64-apple-darwin/rustup-init)
  - [sha256 file](https://static.rust-lang.org/rustup/dist/aarch64-apple-darwin/rustup-init.sha256)
- [aarch64-linux-android](https://static.rust-lang.org/rustup/dist/aarch64-linux-android/rustup-init)
  - [sha256 file](https://static.rust-lang.org/rustup/dist/aarch64-linux-android/rustup-init.sha256)
- [aarch64-pc-windows-msvc](https://static.rust-lang.org/rustup/dist/aarch64-pc-windows-msvc/rustup-init.exe)
  - [sha256 file](https://static.rust-lang.org/rustup/dist/aarch64-pc-windows-msvc/rustup-init.exe.sha256)
- [aarch64-unknown-linux-gnu](https://static.rust-lang.org/rustup/dist/aarch64-unknown-linux-gnu/rustup-init)
  - [sha256 file](https://static.rust-lang.org/rustup/dist/aarch64-unknown-linux-gnu/rustup-init.sha256)
- [aarch64-unknown-linux-musl](https://static.rust-lang.org/rustup/dist/aarch64-unknown-linux-musl/rustup-init)
  - [sha256 file](https://static.rust-lang.org/rustup/dist/aarch64-unknown-linux-musl/rustup-init.sha256)
- [arm-linux-androideabi](https://static.rust-lang.org/rustup/dist/arm-linux-androideabi/rustup-init)
  - [sha256 file](https://static.rust-lang.org/rustup/dist/arm-linux-androideabi/rustup-init.sha256)
- [arm-unknown-linux-gnueabi](https://static.rust-lang.org/rustup/dist/arm-unknown-linux-gnueabi/rustup-init)
  - [sha256 file](https://static.rust-lang.org/rustup/dist/arm-unknown-linux-gnueabi/rustup-init.sha256)
- [arm-unknown-linux-gnueabihf](https://static.rust-lang.org/rustup/dist/arm-unknown-linux-gnueabihf/rustup-init)
  - [sha256 file](https://static.rust-lang.org/rustup/dist/arm-unknown-linux-gnueabihf/rustup-init.sha256)
- [armv7-linux-androideabi](https://static.rust-lang.org/rustup/dist/armv7-linux-androideabi/rustup-init)
  - [sha256 file](https://static.rust-lang.org/rustup/dist/armv7-linux-androideabi/rustup-init.sha256)
- [armv7-unknown-linux-gnueabihf](https://static.rust-lang.org/rustup/dist/armv7-unknown-linux-gnueabihf/rustup-init)
  - [sha256 file](https://static.rust-lang.org/rustup/dist/armv7-unknown-linux-gnueabihf/rustup-init.sha256)
- [i686-apple-darwin](https://static.rust-lang.org/rustup/dist/i686-apple-darwin/rustup-init)
  - [sha256 file](https://static.rust-lang.org/rustup/dist/i686-apple-darwin/rustup-init.sha256)
- [i686-linux-android](https://static.rust-lang.org/rustup/dist/i686-linux-android/rustup-init)
  - [sha256 file](https://static.rust-lang.org/rustup/dist/i686-linux-android/rustup-init.sha256)
- [i686-pc-windows-gnu](https://static.rust-lang.org/rustup/dist/i686-pc-windows-gnu/rustup-init.exe)[^msys2]
  - [sha256 file](https://static.rust-lang.org/rustup/dist/i686-pc-windows-gnu/rustup-init.exe.sha256)
- [i686-pc-windows-msvc](https://static.rust-lang.org/rustup/dist/i686-pc-windows-msvc/rustup-init.exe)[^msvc]
  - [sha256 file](https://static.rust-lang.org/rustup/dist/i686-pc-windows-msvc/rustup-init.exe.sha256)
- [i686-unknown-linux-gnu](https://static.rust-lang.org/rustup/dist/i686-unknown-linux-gnu/rustup-init)
  - [sha256 file](https://static.rust-lang.org/rustup/dist/i686-unknown-linux-gnu/rustup-init.sha256)
- [loongarch64-unknown-linux-gnu](https://static.rust-lang.org/rustup/dist/loongarch64-unknown-linux-gnu/rustup-init)
  - [sha256 file](https://static.rust-lang.org/rustup/dist/loongarch64-unknown-linux-gnu/rustup-init.sha256)
- [loongarch64-unknown-linux-musl](https://static.rust-lang.org/rustup/dist/loongarch64-unknown-linux-musl/rustup-init)
  - [sha256 file](https://static.rust-lang.org/rustup/dist/loongarch64-unknown-linux-musl/rustup-init.sha256)
- [mips-unknown-linux-gnu](https://static.rust-lang.org/rustup/dist/mips-unknown-linux-gnu/rustup-init)
  - [sha256 file](https://static.rust-lang.org/rustup/dist/mips-unknown-linux-gnu/rustup-init.sha256)
- [mips64-unknown-linux-gnuabi64](https://static.rust-lang.org/rustup/dist/mips64-unknown-linux-gnuabi64/rustup-init)
  - [sha256 file](https://static.rust-lang.org/rustup/dist/mips64-unknown-linux-gnuabi64/rustup-init.sha256)
- [mips64el-unknown-linux-gnuabi64](https://static.rust-lang.org/rustup/dist/mips64el-unknown-linux-gnuabi64/rustup-init)
  - [sha256 file](https://static.rust-lang.org/rustup/dist/mips64el-unknown-linux-gnuabi64/rustup-init.sha256)
- [mipsel-unknown-linux-gnu](https://static.rust-lang.org/rustup/dist/mipsel-unknown-linux-gnu/rustup-init)
  - [sha256 file](https://static.rust-lang.org/rustup/dist/mipsel-unknown-linux-gnu/rustup-init.sha256)
- [powerpc-unknown-linux-gnu](https://static.rust-lang.org/rustup/dist/powerpc-unknown-linux-gnu/rustup-init)
  - [sha256 file](https://static.rust-lang.org/rustup/dist/powerpc-unknown-linux-gnu/rustup-init.sha256)
- [powerpc64-unknown-linux-gnu](https://static.rust-lang.org/rustup/dist/powerpc64-unknown-linux-gnu/rustup-init)
  - [sha256 file](https://static.rust-lang.org/rustup/dist/powerpc64-unknown-linux-gnu/rustup-init.sha256)
- [powerpc64le-unknown-linux-gnu](https://static.rust-lang.org/rustup/dist/powerpc64le-unknown-linux-gnu/rustup-init)
  - [sha256 file](https://static.rust-lang.org/rustup/dist/powerpc64le-unknown-linux-gnu/rustup-init.sha256)
- [powerpc64le-unknown-linux-musl](https://static.rust-lang.org/rustup/dist/powerpc64le-unknown-linux-musl/rustup-init)
  - [sha256 file](https://static.rust-lang.org/rustup/dist/powerpc64le-unknown-linux-musl/rustup-init.sha256)
- [s390x-unknown-linux-gnu](https://static.rust-lang.org/rustup/dist/s390x-unknown-linux-gnu/rustup-init)
  - [sha256 file](https://static.rust-lang.org/rustup/dist/s390x-unknown-linux-gnu/rustup-init.sha256)
- [x86_64-apple-darwin](https://static.rust-lang.org/rustup/dist/x86_64-apple-darwin/rustup-init)
  - [sha256 file](https://static.rust-lang.org/rustup/dist/x86_64-apple-darwin/rustup-init.sha256)
- [x86_64-linux-android](https://static.rust-lang.org/rustup/dist/x86_64-linux-android/rustup-init)
  - [sha256 file](https://static.rust-lang.org/rustup/dist/x86_64-linux-android/rustup-init.sha256)
- [x86_64-pc-windows-gnu](https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-gnu/rustup-init.exe)[^msys2]
  - [sha256 file](https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-gnu/rustup-init.exe.sha256)
- [x86_64-pc-windows-msvc](https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe)[^msvc]
  - [sha256 file](https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe.sha256)
- [x86_64-unknown-freebsd](https://static.rust-lang.org/rustup/dist/x86_64-unknown-freebsd/rustup-init)
  - [sha256 file](https://static.rust-lang.org/rustup/dist/x86_64-unknown-freebsd/rustup-init.sha256)
- [x86_64-unknown-illumos](https://static.rust-lang.org/rustup/dist/x86_64-unknown-illumos/rustup-init)
  - [sha256 file](https://static.rust-lang.org/rustup/dist/x86_64-unknown-illumos/rustup-init.sha256)
- [x86_64-unknown-linux-gnu](https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init)
  - [sha256 file](https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init.sha256)
- [x86_64-unknown-linux-musl](https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-musl/rustup-init)
  - [sha256 file](https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-musl/rustup-init.sha256)
- [x86_64-unknown-netbsd](https://static.rust-lang.org/rustup/dist/x86_64-unknown-netbsd/rustup-init)
  - [sha256 file](https://static.rust-lang.org/rustup/dist/x86_64-unknown-netbsd/rustup-init.sha256)

</details>

To get a previous version, use
`https://static.rust-lang.org/rustup/archive/{rustup-version}/{target-triple}/rustup-init[.exe]`.

SHA-256 checksums are also available by appending `.sha256` to the link.

[^msys2]: Windows GNU builds require no additional software for basic use.
    However, many library crates will not be able to compile until
    the full [MSYS2] with MinGW has been installed.

[MSYS2]: https://www.msys2.org/

[^msvc]: MSVC builds of `rustup` additionally require an [installation of
    Visual Studio 2019 or the Visual C++ Build Tools 2019][vs]. For Visual
    Studio, make sure to check the "C++ tools" and "Windows 10 SDK" option.

[vs]: https://visualstudio.microsoft.com/downloads/

## Self-compiled installation

To install `rustup` from source, check out the git repository from
<https://github.com/rust-lang/rustup> and run `cargo run --release`. Note that
after installation the `rustup` toolchains will supersede any pre-existing
toolchains by prepending `~/.cargo/bin` to the `PATH` environment variable.
