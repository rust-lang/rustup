# Concepts

## How rustup works

`rustup` is a *toolchain multiplexer*. It installs and manages many Rust
toolchains and presents them all through a single set of tools installed to
`~/.cargo/bin`. The `rustc` and `cargo` installed to `~/.cargo/bin` are
*proxies* that delegate to the real toolchain. `rustup` then provides
mechanisms to easily change the active toolchain by reconfiguring the behavior
of the proxies.

So when `rustup` is first installed running `rustc` will run the proxy in
`$HOME/.cargo/bin/rustc`, which in turn will run the stable compiler. If you
later *change the default toolchain* to nightly with `rustup default nightly`,
then that same proxy will run the `nightly` compiler instead.

This is similar to Ruby's [rbenv], Python's [pyenv], or Node's [nvm].

[rbenv]: https://github.com/rbenv/rbenv
[pyenv]: https://github.com/yyuu/pyenv
[nvm]: https://github.com/creationix/nvm
