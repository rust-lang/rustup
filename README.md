# rustup: the Rust toolchain installer

| Master CI    | Build Status                                             |
|--------------|----------------------------------------------------------|
| Windows      | ![Windows builds][actions-windows-master]                |
| macOS        | ![maOS builds][actions-macos-master]                     |
| Linux Etc    | ![Linux (etc) builds][actions-linux-master]              |

*rustup* installs [The Rust Programming Language][rustlang] from the official
release channels, enabling you to easily switch between stable, beta,
and nightly compilers and keep them updated. It makes cross-compiling
simpler with binary builds of the standard library for common platforms.
And it runs on all platforms Rust supports, including Windows.

[rustlang]: https://www.rust-lang.org

* [Installation](#installation)
  * [Profiles](#profiles)
  * [Shell autocompletion](#enable-tab-completion-for-bash-fish-zsh-or-powershell)
  * [Where to install?](#choosing-where-to-install)
* [How rustup works](#how-rustup-works)
* [Keeping Rust up to date](#keeping-rust-up-to-date)
* [Working with nightly Rust](#working-with-nightly-rust)
* [Toolchain specification](#toolchain-specification)
* [Toolchain override shorthand](#toolchain-override-shorthand)
* [Directory overrides](#directory-overrides)
* [The toolchain file](#the-toolchain-file)
* [Override precedence](#override-precedence)
* [Cross-compilation](#cross-compilation)
* [Working with Rust on Windows](#working-with-rust-on-windows)
* [Working with custom toolchains](#working-with-custom-toolchains-and-local-builds)
* [Working with network proxies](#working-with-network-proxies)
* [Examples](#examples)
* [Configuration files](#configuration-files)
* [Environment variables](#environment-variables)
* [Other installation methods](#other-installation-methods)
* [Security](#security)
* [FAQ](#faq)
* [License](#license)
* [Contributing](CONTRIBUTING.md)

## Installation

Follow the instructions at https://rustup.rs. If
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

If you see something like `rustc 1.19.0 (0ade33941 2017-07-17)` then
you are ready to Rust. If you decide Rust isn't your thing, you can
completely remove it from your system by running `rustup self
uninstall`.

### Profiles

`rustup` has the concept of "profiles". They are groups of components you can
choose to download while installing a new Rust toolchain. The profiles
available at this time are `minimal`, `default`, and `complete`:

* The **minimal** profile includes as few components as possible to get a
working compiler (`rustc`, `rust-std`, and `cargo`). It's recommended to use
this component on Windows systems if you don't use local documentation, and in
CI.
* The **default** profile includes all the components previously installed by
default (`rustc`, `rust-std`, `cargo`, and `rust-docs`) plus `rustfmt` and
`clippy`. This profile will be used by `rustup` by default, and it's the one
recommended for general use.
* The **complete** profile includes all the components available through
`rustup`. This should never be used, as it includes *every* component ever included
in the metadata and thus will almost always fail. If you are looking for a way
to install devtools such as `miri` or IDE integration tools (`rls`, `rust-analysis`),
you should use the `default` profile and install the needed additional components
manually, either by using `rustup component add` or by using `-c` when installing
the toolchain.

To change the `rustup` profile you can use the `rustup set profile` command. For
example, to select the minimal profile you can use:

```
rustup set profile minimal
```

It's also possible to choose the profile when installing `rustup` for the first
time, either interactively by choosing the "Customize installation" option or
programmaticaly by passing the `--profile=<name>` flag. Profiles will only
affect newly installed toolchains: as usual it will be possible to install
individual components later with: `rustup component add`.

#### Enable tab completion for Bash, Fish, Zsh, or PowerShell

`rustup` now supports generating completion scripts for Bash, Fish,
Zsh, and PowerShell. See `rustup help completions` for full details,
but the gist is as simple as using one of the following:

```console
# Bash
$ rustup completions bash > ~/.local/share/bash-completion/completions/rustup

# Bash (macOS/Homebrew)
$ rustup completions bash > $(brew --prefix)/etc/bash_completion.d/rustup.bash-completion

# Fish
$ mkdir -p ~/.config/fish/completions
$ rustup completions fish > ~/.config/fish/completions/rustup.fish

# Zsh
$ rustup completions zsh > ~/.zfunc/_rustup

# PowerShell v5.0+
$ rustup completions powershell >> $PROFILE.CurrentUserCurrentHost
# or
$ rustup completions powershell | Out-String | Invoke-Expression
```

**Note**: you may need to restart your shell in order for the changes to take
effect.

For `zsh`, you must then add the following line in your `~/.zshrc` before
`compinit`:

```zsh
fpath+=~/.zfunc
```

#### Choosing where to install

`rustup` allows you to customise your installation by setting the environment
variables `CARGO_HOME` and `RUSTUP_HOME` before running the `rustup-init`
executable. As mentioned in the [Environment Variables] section, `RUSTUP_HOME`
sets the root `rustup` folder, which is used for storing installed
toolchains and configuration options. `CARGO_HOME` contains cache files used
by [cargo].

Note that you will need to ensure these environment variables are always
set and that `CARGO_HOME/bin` is in the `$PATH` environment variable when
using the toolchain.

[Environment Variables]: #environment-variables
[cargo]: https://github.com/rust-lang/cargo

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

```console
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

```console
$ rustup self update
info: checking for self-updates
info: downloading self-updates
```

**Note**: `rustup` will automatically update itself at the end of any toolchain
installation as well.  You can prevent this automatic behaviour by passing the
`--no-self-update` argument when running `rustup update` or `rustup toolchain install`.

## Working with nightly Rust

`rustup` gives you easy access to the nightly compiler and its
[experimental features]. To add it just run `rustup toolchain install
nightly`:

[experimental features]: https://doc.rust-lang.org/unstable-book/

```console
$ rustup toolchain install nightly
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

```console
$ rustup run nightly rustc --version
rustc 1.9.0-nightly (02310fd31 2016-03-19)
```

But more likely you want to use it for a while. To switch to nightly
globally, change the default with `rustup default nightly`:

```console
$ rustup default nightly
info: using existing install for 'nightly'
info: default toolchain set to 'nightly'

  nightly unchanged: rustc 1.9.0-nightly (02310fd31 2016-03-19)

```

Now any time you run `cargo` or `rustc` you will be running the
nightly compiler.

With nightly installed any time you run `rustup update`, the nightly channel
will be updated in addition to stable:

```console
$ rustup update
info: syncing channel updates for 'stable'
info: syncing channel updates for 'nightly'
info: checking for self-updates
info: downloading self-updates

   stable unchanged: rustc 1.7.0 (a5d1e7a59 2016-02-29)
  nightly unchanged: rustc 1.9.0-nightly (02310fd31 2016-03-19)

```

_A note about nightly stability_: Nightly toolchains may fail to build, so for
any given date and target platform there may not be a toolchain available.
Furthermore, nightly builds may be published with missing non-default components
(e.g. [`clippy`]). As such, it can be difficult to find fully-working nightlies.
Use the [rustup-components-history][rch] project to find the build status of
recent nightly toolchains and components.

[`clippy`]: https://github.com/rust-lang/rust-clippy
[rch]: https://rust-lang.github.io/rustup-components-history/

## Toolchain specification

Many `rustup` commands deal with *toolchains*, a single installation
of the Rust compiler. `rustup` supports multiple types of
toolchains. The most basic track the official release channels:
*stable*, *beta* and *nightly*; but `rustup` can also install
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
number, such as '1.8.0'. Channel names can be optionally appended with
an archive date, as in 'nightly-2014-12-18', in which case the
toolchain is downloaded from the archive for that date.

Finally, the host may be specified as a target triple. This is most
useful for installing a 32-bit compiler on a 64-bit platform, or for
installing the [MSVC-based toolchain][msvc-toolchain] on Windows. For example:

```console
$ rustup toolchain install stable-x86_64-pc-windows-msvc
```

For convenience, elements of the target triple that are omitted will be
inferred, so the above could be written:

```console
$ rustup toolchain install stable-msvc
```

Toolchain names that don't name a channel instead can be used to name
[custom toolchains].

[msvc-toolchain]: https://www.rust-lang.org/tools/install?platform_override=win
[custom toolchains]: #working-with-custom-toolchains-and-local-builds

## Toolchain override shorthand

The `rustup` toolchain proxies can be instructed directly to use a
specific toolchain, a convenience for developers who often test
different toolchains. If the first argument to `cargo`, `rustc` or
other tools in the toolchain begins with `+`, it will be interpreted
as a `rustup` toolchain name, and that toolchain will be preferred,
as in

```console
cargo +beta test
```

## Directory overrides

Directories can be assigned their own Rust toolchain with `rustup
override`. When a directory has an override then any time `rustc` or
`cargo` is run inside that directory, or one of its child directories,
the override toolchain will be invoked.

To use to a specific nightly for a directory:

```console
rustup override set nightly-2014-12-18
```

Or a specific stable release:

```console
rustup override set 1.0.0
```

To see the active toolchain use `rustup show`. To remove the override
and use the default toolchain again, `rustup override unset`.

## The toolchain file

`rustup` directory overrides are a local configuration, stored in
`$RUSTUP_HOME`. Some projects though find themselves 'pinned' to a
specific release of Rust and want this information reflected in their
source repository. This is most often the case for nightly-only
software that pins to a revision from the release archives.

In these cases the toolchain can be named in the project's directory
in a file called `rust-toolchain`, the content of which is the name of
a single `rustup` toolchain, and which is suitable to check in to
source control.

The toolchains named in this file have a more restricted form than
`rustup` toolchains generally, and may only contain the names of the
three release channels, 'stable', 'beta', 'nightly', Rust version
numbers, like '1.0.0', and optionally an archive date, like
'nightly-2017-01-01'. They may not name custom toolchains, nor
host-specific toolchains.

## Override precedence

There are several ways to specify which toolchain `rustup` should
execute:

* An explicit toolchain, e.g. `cargo +beta`,
* The `RUSTUP_TOOLCHAIN` environment variable,
* A directory override, ala `rustup override set beta`,
* The `rust-toolchain` file,
* The default toolchain,

and they are preferred by `rustup` in that order, with the explicit
toolchain having highest precedence, and the default toolchain having
the lowest. There is one exception though: directory overrides and the
`rust-toolchain` file are also preferred by their proximity to the
current directory. That is, these two override methods are discovered
by walking up the directory tree toward the filesystem root, and a
`rust-toolchain` file that is closer to the current directory will be
preferred over a directory override that is further away.

To verify which toolchain is active use `rustup show`.

## Cross-compilation

Rust [supports a great number of platforms][p]. For many of these
platforms The Rust Project publishes binary releases of the standard
library, and for some the full compiler. `rustup` gives easy access
to all of them.

[p]: https://forge.rust-lang.org/release/platform-support.html

When you first install a toolchain, `rustup` installs only the
standard library for your *host* platform - that is, the architecture
and operating system you are presently running. To compile to other
platforms you must install other *target* platforms. This is done
with the `rustup target add` command. For example, to add the
Android target:

```console
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

To install a target for a toolchain that isn't the default toolchain
use the `--toolchain` argument of `rustup target add`, like so:

```console
$ rustup target add --toolchain <toolchain> <target>...
```

To see a list of available targets, `rustup target list`. To remove a
previously-added target, `rustup target remove`.

## Working with Rust on Windows

`rustup` works the same on Windows as it does on Unix, but there are
some special considerations for Rust developers on Windows. As
[mentioned on the Rust download page][msvc-toolchain], there are two [ABIs] in use
on Windows: the native (MSVC) ABI used by [Visual Studio], and the GNU
ABI used by the [GCC toolchain]. Which version of Rust you need depends
largely on what C/C++ libraries you want to interoperate with: for
interop with software produced by Visual Studio use the MSVC build of
Rust; for interop with GNU software built using the [MinGW/MSYS2
toolchain] use the GNU build.

When targeting the MSVC ABI, Rust additionally requires an [installation
of Visual Studio 2013 (or later) or the Visual C++ Build Tools
2019][vs] so rustc can use its linker. For Visual Studio, make sure to
check the "C++ tools" and "Windows 10 SDK" option. No additional software
installation is necessary for basic use of the GNU build.

By default `rustup` on Windows configures Rust to target the MSVC
ABI, that is a target triple of either `i686-pc-windows-msvc` or
`x86_64-pc-windows-msvc` depending on the CPU architecture of the
host Windows OS. The toolchains that `rustup` chooses to install, unless
told otherwise through the [toolchain specification], will be compiled
to run on that target triple host and will target that triple by default.

You can change this behavior with `rustup set default-host` or during installation.

For example, to explicitly select the 32-bit MSVC host:

```console
$ rustup set default-host i686-pc-windows-msvc
```

Or to choose the 64 bit GNU toolchain:

```console
$ rustup set default-host x86_64-pc-windows-gnu
```

[toolchain specification]: #toolchain-specification

Since the MSVC ABI provides the best interoperation with other Windows software
it is recommended for most purposes. The GNU toolchain is always available, even
if you don't use it by default. Just install it with `rustup toolchain install`:

```console
$ rustup toolchain install stable-gnu
```

You don't need to switch toolchains to support all windows targets though;
a single toolchain supports all four x86 windows targets:

```console
$ rustup target add x86_64-pc-windows-msvc
$ rustup target add x86_64-pc-windows-gnu
$ rustup target add i686-pc-windows-msvc
$ rustup target add i686-pc-windows-gnu
```

[ABIs]: https://en.wikipedia.org/wiki/Application_binary_interface
[Visual Studio]: https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2019
[GCC toolchain]: https://gcc.gnu.org/
[MinGW/MSYS2 toolchain]: https://msys2.github.io/

## Working with distribution Rust packages

Several Linux distributions package Rust, and you may wish to use the packaged
toolchain, such as for distribution package development. You may also wish to
use a `rustup`-managed toolchain such as nightly or beta. Normally, `rustup`
will complain that you already have Rust installed in /usr and refuse to
install. However, you can install Rust via `rustup` and have it coexist with
your distribution's packaged Rust.

When you initially install Rust with `rustup`, pass the `-y` option to make it
ignore the packaged Rust toolchain and install a `rustup`-managed toolchain into
`~/.cargo/bin`. Add that directory to your `$PATH` (or let `rustup` do it for
you by not passing `--no-modify-path`). Then, to tell `rustup` about your system
toolchain, run:

```console
rustup toolchain link system /usr
```

You can then use +system as a `rustup` toolchain, just like +nightly; for
instance, you can run cargo +system build to build with the system toolchain,
or cargo +nightly build to build with nightly.

If you do distribution Rust development, you should likely make +system your
default toolchain:

```console
rustup default system
```

## Working with custom toolchains and local builds

For convenience of developers working on Rust itself, `rustup` can manage
local builds of the Rust toolchain. To teach `rustup` about your build,
run:

```console
$ rustup toolchain link my-toolchain path/to/my/toolchain/sysroot
```

For example, on Ubuntu you might clone `rust-lang/rust` into `~/rust`, build it, and then run:

```console
$ rustup toolchain link myrust ~/rust/build/x86_64-unknown-linux-gnu/stage2/
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

## Working with network proxies

Enterprise networks often don't have direct outside HTTP access, but enforce
the use of proxies. If you're on such a network, you can request that
`rustup` uses a proxy by setting its URL in the environment. In most cases,
setting `https_proxy` should be sufficient. On a Unix-like system with a
shell like __bash__ or __zsh__, you could use:

```bash
export https_proxy=socks5://proxy.example.com:1080 # or http://proxy.example.com:8080
```

On Windows, the command would be:

```cmd
set https_proxy=socks5://proxy.example.com:1080
```

If you need a more complex setup, `rustup` supports the convention used by
the __curl__ program, documented in the ENVIRONMENT section of
[its manual page][curlman].

The use of `curl` is presently **deprecated**, however it can still be used by
providing the `RUSTUP_USE_CURL` environment variable, for example:

```bash
RUSTUP_USE_CURL=1 rustup update
```

Note that some versions of `libcurl` apparently require you to drop the
`http://` or `https://` prefix in environment variables. For example,
`export http_proxy=proxy.example.com:1080` (and likewise for HTTPS).
If you are getting an SSL `unknown protocol` error from `rustup` via `libcurl`
but the command-line `curl` command works fine, this may be the problem.

[curlman]: https://curl.haxx.se/docs/manpage.html


## Examples


Command                                                     | Description
----------------------------------------------------------- | ------------------------------------------------------------
`rustup default nightly`                                    | Set the default toolchain to the latest nightly
`rustup set profile minimal`                                | Set the default "profile" (see [profiles](#profiles))
`rustup target list`                                        | List all available targets for the active toolchain
`rustup target add arm-linux-androideabi`                   | Install the Android target
`rustup target remove arm-linux-androideabi`                | Remove the Android target
`rustup run nightly rustc foo.rs`                           | Run the nightly regardless of the active toolchain
`rustc +nightly foo.rs`                                     | Shorthand way to run a nightly compiler
`rustup run nightly bash`                                   | Run a shell configured for the nightly compiler
`rustup default stable-msvc`                                | On Windows, use the MSVC toolchain instead of GNU
`rustup override set nightly-2015-04-01`                    | For the current directory, use a nightly from a specific date
`rustup toolchain link my-toolchain "C:\RustInstallation"`  | Install a custom toolchain by symlinking an existing installation
`rustup show`                                               | Show which toolchain will be used in the current directory
`rustup toolchain uninstall nightly`                        | Uninstall a given toolchain
`rustup toolchain help`                                     | Show the `help` page for a subcommand (like `toolchain`)
`rustup man cargo`                                          | \(*Unix only*\) View the man page for a given command (like `cargo`)

## Configuration files

Rustup has a settings file in [TOML](https://github.com/toml-lang/toml) format
at `${RUSTUP_HOME}/settings.toml`. The schema for this file is not part of the
public interface for rustup - the rustup CLI should be used to query and set
settings.

On Unix operating systems a fallback settings file is consulted for some
settings. This fallback file is located at `/etc/rustup/settings.toml` and
currently can define only `default_toolchain`.

## Environment variables


- `RUSTUP_HOME` (default: `~/.rustup` or `%USERPROFILE%/.rustup`)
  Sets the root `rustup` folder, used for storing installed
  toolchains and configuration options.

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

- `RUSTUP_IO_THREADS` *unstable* (defaults to reported cpu count). Sets the
  number of threads to perform close IO in. Set to `disabled` to force
  single-threaded IO for troubleshooting, or an arbitrary number to
  override automatic detection.

- `RUSTUP_TRACE_DIR` *unstable* (default: no tracing)
  Enables tracing and determines the directory that traces will be
  written too. Traces are of the form PID.trace. Traces can be read
  by the Catapult project [tracing viewer][tv].

  [tv]: (https://github.com/catapult-project/catapult/blob/master/tracing/README.md)

- `RUSTUP_UNPACK_RAM` *unstable* (default 400M, min 100M)
  Caps the amount of RAM `rustup` will use for IO tasks while unpacking.

- `RUSTUP_NO_BACKTRACE`
  Disables backtraces on non-panic errors even when `RUST_BACKTRACE` is set.

## Other installation methods

The primary installation method, as described at https://rustup.rs, differs by platform:

* On Windows, download and run the [rustup-init.exe built for
  `i686-pc-windows-gnu` target][setup]. In general, this is the build of
  `rustup` one should install on Windows. Despite being built against the GNU
  toolchain, _the Windows build of `rustup` will install Rust for the MSVC
  toolchain if it detects that MSVC is installed_. If you prefer to install GNU
  toolchains or x86_64 toolchains by default this can be modified at install
  time, either interactively or with the `--default-host` flag, or after
  installation via `rustup set default-host`.
* On Unix, run `curl https://sh.rustup.rs -sSf | sh` in your
  shell. This downloads and runs [`rustup-init.sh`], which in turn
  downloads and runs the correct version of the `rustup-init`
  executable for your platform.

[setup]: https://static.rust-lang.org/rustup/dist/i686-pc-windows-gnu/rustup-init.exe
[`rustup-init.sh`]: https://static.rust-lang.org/rustup/rustup-init.sh

`rustup-init` accepts arguments, which can be passed through
the shell script. Some examples:

```console
$ curl https://sh.rustup.rs -sSf | sh -s -- --help
$ curl https://sh.rustup.rs -sSf | sh -s -- --no-modify-path
$ curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain nightly
$ curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain none
$ curl https://sh.rustup.rs -sSf | sh -s -- --profile minimal --default-toolchain nightly
```

If you prefer you can directly download `rustup-init` for the
platform of your choice:

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
- [i686-pc-windows-msvc](https://static.rust-lang.org/rustup/dist/i686-pc-windows-msvc/rustup-init.exe)<sup>[†](#vs2019)</sup>
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
- [x86_64-pc-windows-msvc](https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe)<sup>[†](#vs2019)</sup>
- [x86_64-unknown-freebsd](https://static.rust-lang.org/rustup/dist/x86_64-unknown-freebsd/rustup-init)
- [x86_64-unknown-linux-gnu](https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init)
- [x86_64-unknown-linux-musl](https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-musl/rustup-init)
- [x86_64-unknown-netbsd](https://static.rust-lang.org/rustup/dist/x86_64-unknown-netbsd/rustup-init)

<a name="vs2019">†</a>
MSVC builds of `rustup` additionally require an [installation of
Visual Studio 2019 or the Visual C++ Build Tools 2019][vs]. For
Visual Studio, make sure to check the "C++ tools" and "Windows
10 SDK" option. No additional software installation is necessary
for basic use of the GNU build.

[vs]: https://visualstudio.microsoft.com/downloads/

You can fetch an older version from `https://static.rust-lang.org/rustup/archive/{rustup-version}/{target-triple}/rustup-init[.exe]`

To install from source just run `cargo run --release`. Note that
currently `rustup` only builds on nightly Rust, and that after
installation the `rustup` toolchains will supersede any pre-existing
toolchains by prepending `~/.cargo/bin` to the `PATH` environment
variable.

## Security

`rustup` is secure enough for most people, but it [still needs
work][s]. `rustup` performs all downloads over HTTPS, but does not
yet validate signatures of downloads.

[s]: https://github.com/rust-lang/rustup/issues?q=is%3Aopen+is%3Aissue+label%3Asecurity

File modes on installation honor umask as of 1.18.4, use umask if
very tight controls are desired.

## FAQ

### Is this an official Rust project?

Yes. rustup is an official Rust project. It is the recommended way
to install Rust at https://www.rust-lang.org.

### How is this related to multirust?

rustup is the successor to [multirust]. rustup began as multirust-rs,
a rewrite of multirust from shell script to Rust, by [Diggory Blake],
and is now maintained by The Rust Project.

### Can rustup download the Rust source code?

The Rust source can be obtained by running `rustup component add rust-src`.
It will be downloaded to the `<toolchain root>/lib/rustlib/src/rust`
directory of the current toolchain.

### rustup fails with Windows error 32

If `rustup` fails with Windows error 32, it may be due to antivirus
scanning in the background. Disable antivirus scanner and try again.

[multirust]: https://github.com/brson/multirust
[Diggory Blake]: https://github.com/Diggsey

### I get "error: could not remove 'rustup-bin' file: 'C:\Users\USER\\.cargo\bin\rustup.exe'"

If `rustup` fails to self-update in this way it's usually because RLS is
running (your editor is open and running RLS). The solution is to stop RLS (by
closing your editor) and try again.

## License

Copyright Diggory Blake, the Mozilla Corporation, and rustup
contributors.

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or https://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or https://opensource.org/licenses/MIT)

at your option.

<!-- Badges -->
[actions-windows-master]: https://github.com/rust-lang/rustup/workflows/Windows%20(master)/badge.svg
[actions-macos-master]: https://github.com/rust-lang/rustup/workflows/macOS/badge.svg?branch=master
[actions-linux-master]: https://github.com/rust-lang/rustup/workflows/Linux%20(master)/badge.svg
