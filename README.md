# rustup: the Rust toolchain installer

| Build Status |                                                                              |
|--------------|------------------------------------------------------------------------------|
| Travis       | [![Travis Build Status][travis-build-status-svg]][travis-build-status]       |
| AppVeyor     | [![AppVeyor Build Status][appveyor-build-status-svg]][appveyor-build-status] |

*rustup* installs [The Rust Programming Language] from the official
release channels, enabling you to easily switch between stable, beta,
and nightly compilers and keep them updated. It makes cross-compiling
simpler with binary builds of the standard library for common platforms.
And it runs on all platforms Rust supports, including Windows.

[The Rust Programming Language]: https://www.rust-lang.org

* [Installation](#installation)
* [How rustup works](#how-rustup-works)
* [Keeping Rust up to date](#keeping-rust-up-to-date)
* [Working with nightly Rust](#working-with-nightly-rust)
* [Directory overrides](#directory-overrides)
* [Toolchain specification](#toolchain-specification)
* [Cross-compilation](#cross-compilation)
* [Working with Rust on Windows](#working-with-rust-on-windows)
* [Working with custom toolchains](#working-with-custom-toolchains-and-local-builds)
* [Working with HTTP proxies](#working-with-http-proxies)
* [Examples](#examples)
* [Environment variables](#environment-variables)
* [Other installation methods](#other-installation-methods)
* [Security](#security)
* [FAQ](#faq)
* [License](#license)
* [Contributing](#contributing)

## Installation

Follow the instructions at [www.rustup.rs](https://www.rustup.rs). If
that doesn't work for you there are [other installation
methods](#other-installation-methods).

`rustup` installs `rustc`, `cargo`, `rustup` and other standard tools
to Cargo's `bin` directory. On Unix it is located at
`$HOME/.cargo/bin` and on Windows at `%USERPROFILE%\.cargo\bin`. This
is the same directory that `cargo install` will install Rust programs
and Cargo plugins.

This directory will be in your `$PATH` environment variable, which
means you can run them from the shell without further
configuration. Open a *new* shell and type the following:

```
rustc --version
```

If you see something like `rustc 1.7.0 (a5d1e7a59 2016-02-29)` then
you are ready to Rust. If you decide Rust isn't your thing, you can
completely remove it from your system by running `rustup self
uninstall`.

#### Enable tab completion for Bash, Fish, or Zsh

`rustup` now supports generating completion scripts for Bash, Fish,
and Zsh. See `rustup help completions` for full details, but the
gist is as simple as using one of the following:

```
# Bash
$ rustup completions bash > /etc/bash_completion.d/rustup.bash-completion

# Fish
$ rustup completions fish > ~/.config/fish/completions/rustup.fish

# Zsh
$ rustup completions zsh > ~/.zfunc/_rustup
```

*Note:* you may need to restart your shell in order for the changes to take
 effect.

For `zsh`, you must then add the following line in your `~/.zshrc` before
`compinit`:

```zsh
fpath+=~/.zfunc
```

## How rustup works

`rustup` is a *toolchain multiplexer*. It installs and manages many
Rust toolchains and presents them all through a single set of tools
installed to `~/.cargo/bin`. The `rustc` and `cargo` installed to
`~/.cargo/bin` are *proxies* that delegate to the real
toolchain. `rustup` then provides mechanisms to easily change the
active toolchain by reconfiguring the behavior of the proxies.

So when `rustup` is first installed running `rustc` will run the proxy
in `$HOME/.cargo/bin/rustc`, which in turn will run the stable
compiler. If you later *change the default toolchain* to nightly with
`rustup default nightly`, then that same proxy will run the `nightly`
compiler instead.

This is similar to Ruby's [rbenv], Python's [pyenv], or Node's [nvm].

[rbenv]: https://github.com/rbenv/rbenv
[pyenv]: https://github.com/yyuu/pyenv
[nvm]: https://github.com/creationix/nvm

## Keeping Rust up to date

Rust is distributed on three different [release channels]: stable,
beta, and nightly. `rustup` is configured to use the stable channel by
default, which represents the latest release of Rust,
and is released every six weeks.

[release channels]: https://github.com/rust-lang/rfcs/blob/master/text/0507-release-channels.md

When a new version of Rust is released, you can type `rustup update` to update
to it:

```
$ rustup update
info: syncing channel updates for 'stable'
info: downloading component 'rustc'
info: downloading component 'rust-std'
info: downloading component 'rust-docs'
info: downloading component 'cargo'
info: installing component 'rustc'
info: installing component 'rust-std'
info: installing component 'rust-docs'
info: installing component 'cargo'
info: checking for self-updates
info: downloading self-updates

  stable updated: rustc 1.7.0 (a5d1e7a59 2016-02-29)

```

This is the essence of `rustup`.

### Keeping rustup up to date

Running `rustup update` also checks for updates to `rustup` and automatically
installs the latest version. To manually check for updates and install the
latest version of `rustup` without updating installed toolchains type `rustup
self update`:

```
$ rustup self update
info: checking for self-updates
info: downloading self-updates
```

## Working with nightly Rust

Rustup gives you easy access to the nightly compiler and its
[experimental features]. To add it just run `rustup install
nightly`:

[experimental features]: http://doc.rust-lang.org/nightly/book/nightly-rust.html

```
$ rustup install nightly
info: syncing channel updates for 'nightly'
info: downloading toolchain manifest
info: downloading component 'rustc'
info: downloading component 'rust-std'
info: downloading component 'rust-docs'
info: downloading component 'cargo'
info: installing component 'rustc'
info: installing component 'rust-std'
info: installing component 'rust-docs'
info: installing component 'cargo'

  nightly installed: rustc 1.9.0-nightly (02310fd31 2016-03-19)

```

Now Rust nightly is installed, but not activated. To test it out you
can run a command from the nightly toolchain like

```
$ rustup run nightly rustc --version
rustc 1.9.0-nightly (02310fd31 2016-03-19)
```

But more likely you want to use it for a while. To switch to nightly
globally, change the default with `rustup default nightly`:

```
$ rustup default nightly
info: using existing install for 'nightly'
info: default toolchain set to 'nightly'

  nightly unchanged: rustc 1.9.0-nightly (02310fd31 2016-03-19)

```

Now any time you run `cargo` or `rustc` you will be running the
nightly compiler.

With nightly installed any time you run `rustup update`, the nightly channel
will be updated in addition to stable:

```
$ rustup update
info: syncing channel updates for 'stable'
info: syncing channel updates for 'nightly'
info: checking for self-updates
info: downloading self-updates

   stable unchanged: rustc 1.7.0 (a5d1e7a59 2016-02-29)
  nightly unchanged: rustc 1.9.0-nightly (02310fd31 2016-03-19)

```

## Directory overrides

Directories can be assigned their own Rust toolchain with
`rustup override`. When a directory has an override then
any time `rustc` or `cargo` is run inside that directory,
or one of its child directories, the override toolchain
will be invoked.

To pin to a specific nightly:

```
rustup override set nightly-2014-12-18
```

Or a specific stable release:

```
rustup override set 1.0.0
```

To see the active toolchain use `rustup show`. To remove the override
and use the default toolchain again, `rustup override unset`.

## Toolchain specification

Many `rustup` commands deal with *toolchains*, a single installation
of the Rust compiler. `rustup` supports multiple types of
toolchains. The most basic track the official release channels:
'stable', 'beta' and 'nightly'; but `rustup` can also install
toolchains from the official archives, for alternate host platforms,
and from local builds.

Standard release channel toolchain names have the following form:

```
<channel>[-<date>][-<host>]

<channel>       = stable|beta|nightly|<version>
<date>          = YYYY-MM-DD
<host>          = <target-triple>
```

'channel' is either a named release channel or an explicit version
number, such as "1.8.0". Channel names can be optionally appended with
an archive date, as in 'nightly-2014-12-18', in which case the
toolchain is downloaded from the archive for that date.

Finally, the host may be specified as a target triple. This is most
useful for installing a 32-bit compiler on a 64-bit platform, or for
installing the [MSVC-based toolchain] on Windows. For example:

```
$ rustup install stable-x86_64-pc-windows-msvc
```

For convenience, elements of the target triple that are omitted will be
inferred, so the above could be written:

```
$ rustup install stable-msvc
```

Toolchain names that don't name a channel instead can be used to name
[custom toolchains].

[MSVC-based toolchain]: https://www.rust-lang.org/downloads.html#win-foot
[custom toolchains]: #working-with-custom-toolchains-and-local-builds

## Cross-compilation

Rust [supports a great number of platforms][p]. For many of these
platforms The Rust Project publishes binary releases of the standard
library, and for some the full compiler. `rustup` gives easy access
to all of them.

[p]: http://doc.rust-lang.org/nightly/book/getting-started.html#platform-support

When you first install a toolchain, `rustup` installs only the
standard library for your *host* platform - that is, the architecture
and operating system you are presently running. To compile to other
platforms you must install other *target* platforms. This is done
with the `rustup target add` command. For example, to add the
Android target:

```
$ rustup target add arm-linux-androideabi
info: downloading component 'rust-std' for 'arm-linux-androideabi'
info: installing component 'rust-std' for 'arm-linux-androideabi'
```

With the `arm-linux-androideabi` target installed you can then build
for Android with Cargo by passing the `--target` flag, as in `cargo
build --target=arm-linux-androideabi`.

Note that `rustup target add` only installs the Rust standard library
for a given target. There are typically other tools necessary to
cross-compile, particularly a linker. For example, to cross compile
to Android the [Android NDK] must be installed. In the future, `rustup`
will provide assistance installing the NDK components as well.

[Android NDK]: https://developer.android.com/tools/sdk/ndk/index.html

To see a list of available targets, `rustup target list`. To remove a
previously-added target, `rustup target remove`.

## Working with Rust on Windows

`rustup` works the same on Windows as it does on Unix, but there are
some special considerations for Rust developers on Windows. As
[mentioned on the Rust download page][d], there are two [ABIs] in use
on Windows: the native (MSVC) ABI used by [Visual Studio], and the GNU
ABI used by the [GCC toolchain]. Which version of Rust you need depends
largely on what C/C++ libraries you want to interoperate with: for
interop with software produced by Visual Studio use the MSVC build of
Rust; for interop with GNU software built using the [MinGW/MSYS2
toolchain] use the GNU build.

When target the MSVC ABI, Rust additionally require an [installation
of Visual Studio 2013 (or later) or the Visual C++ Build Tools
2015][vs] so rustc can use its linker. For Visual Studio, make sure to
check the "C++ tools" option. No additional software installation is
necessary for basic use of the GNU build.

By default rustup on Windows configures Rust to target the 32-bit MSVC
ABI, that is the `i686-pc-windows-msvc` target triple. More
specifically, the toolchains that rustup chooses to install, unless
told otherwise through the [toolchain specification], will be compiled
to run on a `i686-pc-windows-msvc` host, and will target that platform
by default. When you write `rustup update nightly`, rustup interprets
it as `rustup update nightly-i686-pc-windows-msvc`. You can change this
behavior with `rustup set default-host` or during installation.

```
$ rustup set default-host x86_64-pc-windows-msvc
```

[toolchain specification]: #toolchain-specification

Since the MSVC ABI provides the best interoperation with other Windows software
it is recommended for most purposes. The GNU toolchain is always available, even
if you don't use it by default. Just install it with `rustup install`:

```
$ rustup install stable-gnu
```

You don't need to switch toolchains to support all windows targets though;
a single toolchain supports all four x86 windows targets:

```
$ rustup target add x86_64-pc-windows-msvc
$ rustup target add x86_64-pc-windows-gnu
$ rustup target add i686-pc-windows-msvc
$ rustup target add i686-pc-windows-gnu
```

[d]: https://www.rust-lang.org/downloads.html#win-foot
[ABIs]: https://en.wikipedia.org/wiki/Application_binary_interface
[Visual Studio]: https://www.visualstudio.com/
[GCC toolchain]: https://gcc.gnu.org/
[MinGW/MSYS2 toolchain]: https://msys2.github.io/
[vs]: https://www.visualstudio.com/downloads

## Working with custom toolchains and local builds

For convenience of developers working on Rust itself, `rustup` can manage
local builds of the Rust toolchain. To teach `rustup` about your build,
run:

```
$ rustup toolchain link my-toolchain path/to/my/toolchain/sysroot
```

For example, on Ubuntu you might clone `rust-lang/rust` into `~/rust`, build it, and then run:

```
$ rustup toolchain link myrust ~/rustg/x86_64-unknown-linux-gnu/stage2/
$ rustup default myrust
```

Now you can name `my-toolchain` as any other `rustup`
toolchain. Create a `rustup` toolchain for each of your
`rust-lang/rust` workspaces and test them easily with `rustup run
my-toolchain rustc`.

Because the `rust-lang/rust` tree does not include Cargo, *when `cargo`
is invoked for a custom toolchain and it is not available, `rustup`
will attempt to use `cargo` from one of the release channels*,
preferring 'nightly', then 'beta' or 'stable'.

## Working with HTTP proxies

Enterprise networks often don't have direct outside HTTP access, but enforce
the use of HTTP proxies. If you're on such a network, you can request that
rustup uses a proxy by setting its URL in the environment. In most cases,
setting `http_proxy` should be sufficient. On a Unix-like system with a
shell like __bash__ or __zsh__, you could use:

```
export http_proxy=http://proxy.example.com:8080
```

On Windows, the command would be:

```
set http_proxy=http://proxy.example.com:8080
```

If you need a more complex setup, rustup supports the convention used by
the __curl__ program, documented in the ENVIRONMENT section of
[its manual page][curlman].

[curlman]: https://curl.haxx.se/docs/manpage.html


## Examples


Command | Description
--- | ---
`rustup default nightly` | Set the default toolchain to the latest nightly
`rustup target list` | List all available targets for the active toolchain
`rustup target add arm-linux-androideabi` | Install the Android target
`rustup target remove arm-linux-androideabi` | Remove the Android target
`rustup run nightly rustc foo.rs` | Run the nightly regardless of the active toolchain
`rustc +nightly foo.rs` | Shorthand way to run a nightly compiler
`rustup run nightly bash` | Run a shell configured for the nightly compiler
`rustup default stable-msvc` | On Windows, use the MSVC toolchain instead of GNU
`rustup override nightly-2015-04-01` | For the current directory, use a nightly from a specific date
`rustup toolchain link my-toolchain "C:\RustInstallation"` | Install a custom toolchain by symlinking an existing installation
`rustup show` | Show which toolchain will be used in the current directory

## Environment variables

- `RUSTUP_HOME` (default: `~/.multirust` or `%USERPROFILE%/.multirust`)
  Sets the root rustup folder, used for storing installed
  toolchains and configuration options. (rustup used to be called multirust. Use
  of `.multirust` is a holdover from that. See #537.)

- `RUSTUP_TOOLCHAIN` (default: none)
  If set, will override the toolchain used for all rust tool
  invocations. A toolchain with this name should be installed, or
  invocations will fail.

- `RUSTUP_DIST_SERVER` (default: `https://static.rust-lang.org`)
  Sets the root URL for downloading static resources related to Rust.
  You can change this to instead use a local mirror,
  or to test the binaries from the staging directory.

- `RUSTUP_DIST_ROOT` (default: `https://static.rust-lang.org/dist`)
  Deprecated. Use `RUSTUP_DIST_SERVER` instead.

- `RUSTUP_UPDATE_ROOT` (default `https://static.rust-lang.org/rustup`)
  Sets the root URL for downloading self-updates.

## Other installation methods

The primary installation method, as described at
[www.rustup.rs](https://www.rustup.rs), differs by platform:

* On Windows, download and run the [rustup-init.exe built for
  `i686-pc-windows-gnu` target][setup]. Despite being built against the GNU
  toolchain, _the Windows build of rustup will install Rust for the MSVC
  toolchain if it detects that MSVC is installed_. If you prefer to install GNU
  toolchains or x86_64 toolchains by default this can be modified at install
  time, either interactively or with the `--default-host` flag, or after
  installation via `rustup set default-host`. In general, this is the build
  of rustup one should install on Windows.
* On Unix, run `curl https://sh.rustup.rs -sSf | sh` in your
  shell. This downloads and runs [`rustup-init.sh`], which in turn
  downloads and runs the correct version of the `rustup-init`
  executable for your platform.

[setup]: https://static.rust-lang.org/rustup/dist/i686-pc-windows-gnu/rustup-init.exe
[`rustup-init.sh`]: https://static.rust-lang.org/rustup/rustup-init.sh

`rustup-init` accepts arguments, which can be passed through
the shell script. Some examples:

```
$ curl https://sh.rustup.rs -sSf | sh -s -- --help
$ curl https://sh.rustup.rs -sSf | sh -s -- --no-modify-path
$ curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain nightly
```

If you prefer you can directly download `rustup-init` for the
platform of your choice:

- [aarch64-unknown-linux-gnu](https://static.rust-lang.org/rustup/dist/aarch64-unknown-linux-gnu/rustup-init)
- [arm-unknown-linux-gnueabi](https://static.rust-lang.org/rustup/dist/arm-unknown-linux-gnueabi/rustup-init)
- [arm-unknown-linux-gnueabihf](https://static.rust-lang.org/rustup/dist/arm-unknown-linux-gnueabihf/rustup-init)
- [armv7-unknown-linux-gnueabihf](https://static.rust-lang.org/rustup/dist/armv7-unknown-linux-gnueabihf/rustup-init)
- [i686-apple-darwin](https://static.rust-lang.org/rustup/dist/i686-apple-darwin/rustup-init)
- [i686-pc-windows-gnu](https://static.rust-lang.org/rustup/dist/i686-pc-windows-gnu/rustup-init.exe)
- [i686-pc-windows-msvc](https://static.rust-lang.org/rustup/dist/i686-pc-windows-msvc/rustup-init.exe)<sup>[†](#vs2015)</sup>
- [i686-unknown-linux-gnu](https://static.rust-lang.org/rustup/dist/i686-unknown-linux-gnu/rustup-init)
- [x86_64-apple-darwin](https://static.rust-lang.org/rustup/dist/x86_64-apple-darwin/rustup-init)
- [x86_64-pc-windows-gnu](https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-gnu/rustup-init.exe)
- [x86_64-pc-windows-msvc](https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe)<sup>[†](#vs2015)</sup>
- [x86_64-unknown-freebsd](https://static.rust-lang.org/rustup/dist/x86_64-unknown-freebsd/rustup-init)
- [x86_64-unknown-linux-gnu](https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init)
- [x86_64-unknown-netbsd](https://static.rust-lang.org/rustup/dist/x86_64-unknown-netbsd/rustup-init)

<a name="vs2015">†</a>
MSVC builds of `rustup` additionally require an [installation of
Visual Studio 2015 or the Visual C++ Build Tools 2015][vs]. For
Visual Studio, make sure to check the "C++ tools" option. No
additional software installation is necessary for basic use of
the GNU build.

[vs]: https://www.visualstudio.com/downloads

To install from source just run `cargo run --release`. Note that
currently rustup only builds on nightly Rust, and that after
installation the rustup toolchains will supersede any pre-existing
toolchains by prepending `~/.cargo/bin` to the `PATH` environment
variable.

## Security

`rustup` is secure enough for the non-paranoid, but it [still needs
work][s]. `rustup` performs all downloads over HTTPS, but does not
yet validate signatures of downloads.

[s]: https://github.com/rust-lang-nursery/rustup.rs/issues?q=is%3Aopen+is%3Aissue+label%3Asecurity

## FAQ

### Is this an official Rust project?

Yes. rustup is an official Rust project. Once it is complete it will
be the recommended way to install Rust and www.rust-lang.org will be
updated to reflect that.

### How is this related to multirust?

rustup is the successor to [multirust]. rustup began as multirust-rs,
a rewrite of multirust from shell script to Rust, by [Diggory Blake],
and is now maintained by The Rust Project.

### Can rustup download the Rust source code?

The Rust source can be obtained by running `rustup component add rust-src`. 
It will be downloaded to the `<toolchain root>/lib/rustlib/src/rust` 
directory of the current toolchain.

### rustup fails with Windows error 32

If rustup fails with Windows error 32, it may be due to antivirus
scanning in the background. Disable antivirus scanner and try again.

[multirust]: https://github.com/brson/multirust
[Diggory Blake]: https://github.com/Diggsey

## License

Copyright Diggory Blake, the Mozilla Corporation, and rustup
contributors.

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contributing

1. Fork it!
2. Create your feature branch: `git checkout -b my-new-feature`
3. Commit your changes: `git commit -am 'Add some feature'`
4. Push to the branch: `git push origin my-new-feature`
5. Submit a pull request :D

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the
Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.

<!-- Badges -->
[travis-build-status]: https://travis-ci.org/rust-lang-nursery/rustup.rs
[travis-build-status-svg]: https://img.shields.io/travis/rust-lang-nursery/rustup.rs.svg
[appveyor-build-status]: https://ci.appveyor.com/project/brson/rustup-rs
[appveyor-build-status-svg]: https://img.shields.io/appveyor/ci/brson/rustup-rs.svg
