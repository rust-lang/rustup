# Installation

Follow the instructions at <https://www.rust-lang.org/tools/install>. If that
doesn't work for you there are [other installation methods].

`rustup` installs `rustc`, `cargo`, `rustup` and other standard tools to
Cargo's `bin` directory. On Unix it is located at `$HOME/.cargo/bin` and on
Windows at `%USERPROFILE%\.cargo\bin`. This is the same directory that `cargo
install` will install Rust programs and Cargo plugins.

This directory will be in your `$PATH` environment variable, which means you
can run them from the shell without further configuration. Open a *new* shell
and type the following:

```console
rustc --version
```

If you see something like `rustc 1.19.0 (0ade33941 2017-07-17)` then you are
ready to Rust. If you decide Rust isn't your thing, you can completely remove
it from your system by running `rustup self uninstall`.

[other installation methods]: other.md

## Choosing where to install

`rustup` allows you to customise your installation by setting the environment
variables `CARGO_HOME` and `RUSTUP_HOME` before running the `rustup-init`
executable. As mentioned in the [Environment Variables] section, `RUSTUP_HOME`
sets the root `rustup` folder, which is used for storing installed toolchains
and configuration options. `CARGO_HOME` contains cache files used by [cargo].

Note that you will need to ensure these environment variables are always set
and that `CARGO_HOME/bin` is in the `$PATH` environment variable when using
the toolchain.

[Environment Variables]: ../environment-variables.md
[cargo]: https://doc.rust-lang.org/cargo/

## Installing nightly

If you specify the [nightly channel] when installing `rustup`, the
`rustup-init` script will do a "forced" installation by default. A "forced"
installation means it will install the nightly channel regardless of whether
it might be missing [components] that you want. If you want to install rustup
with the nightly channel, and ensure it has the components that you want, you
will need to do this in two phases. For example, if you want to make a fresh
installation of `rustup` and then install `nightly` along with `clippy` or
`miri`, first install `rustup` without a toolchain:

```console
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --default-toolchain none -y
```

Next you can install `nightly` allowing `rustup` to downgrade until it finds
the components you need:

```console
rustup toolchain install nightly --allow-downgrade --profile minimal --component clippy
```

This can be used to great effect in CI, to get you a toolchain rapidly which
meets your criteria.

[nightly channel]: ../concepts/channels.md
[components]: ../concepts/components.md

## Enable tab completion for Bash, Fish, Zsh, or PowerShell

`rustup` now supports generating completion scripts for Bash, Fish, Zsh, and
PowerShell. See `rustup help completions` for full details, but the gist is as
simple as using one of the following:

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
