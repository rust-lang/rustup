# Concepts

## How rustup works

`rustup` is a *toolchain multiplexer*. It installs and manages many Rust
toolchains and presents them all through a single set of tools installed to
`~/.cargo/bin`. The [`rustc`] and [`cargo`] executables installed in
`~/.cargo/bin` are *[proxies]* that delegate to the real toolchain. `rustup`
then provides mechanisms to easily change the active toolchain by
reconfiguring the behavior of the proxies.

So when `rustup` is first installed, running `rustc` will run the proxy in
`$HOME/.cargo/bin/rustc`, which in turn will run the stable compiler. If you
later *change the default toolchain* to nightly with `rustup default nightly`,
then that same proxy will run the `nightly` compiler instead.

This is similar to Ruby's [rbenv], Python's [pyenv], or Node's [nvm].

[rbenv]: https://github.com/rbenv/rbenv
[pyenv]: https://github.com/yyuu/pyenv
[nvm]: https://github.com/creationix/nvm
[`rustc`]: https://doc.rust-lang.org/rustc/
[`cargo`]: https://doc.rust-lang.org/cargo/
[proxies]: proxies.md

## Terminology

* **channel** --- Rust is released to three different "channels": stable, beta,
  and nightly. See the [Channels] chapter for more details.

* **toolchain** --- A "toolchain" is a complete installation of the Rust
  compiler (`rustc`) and related tools (like `cargo`). A [toolchain
  specification] includes the release channel or version, and the host
  platform that the toolchain runs on.

* **target** --- `rustc` is capable of generating code for many platforms. The
  "target" specifies the platform that the code will be generated for. By
  default, `cargo` and `rustc` use the host toolchain's platform as the
  target. To build for a different target, usually the target's standard
  library needs to be installed first via the `rustup target` command. See the
  [Cross-compilation] chapter for more details.

* **component** --- Each release of Rust includes several "components", some of
  which are required (like `rustc`) and some that are optional (like
  [`clippy`]). See the [Components] chapter for more detail.

* **profile** --- In order to make it easier to work with components, a
  "profile" defines a grouping of components. See the [Profiles] chapter for
  more details.

* **proxy** ---  A wrapper for a common Rust component (like `rustc`), built to forward
  CLI invocations to the active Rust toolchain. See the [Proxies] chapter for more details.

[`clippy`]: https://github.com/rust-lang/rust-clippy
[components]: components.md
[cross-compilation]: ../cross-compilation.md
[profiles]: profiles.md
[toolchain specification]: toolchains.md
[channels]: channels.md
[proxies]: proxies.md
