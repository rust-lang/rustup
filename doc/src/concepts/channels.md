# Channels

## Working with nightly Rust

`rustup` gives you easy access to the nightly compiler and its [experimental
features]. To add it just run `rustup toolchain install nightly`:

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

Now Rust nightly is installed, but not activated. To test it out you can run a
command from the nightly toolchain like

```console
$ rustup run nightly rustc --version
rustc 1.9.0-nightly (02310fd31 2016-03-19)
```

But more likely you want to use it for a while. To switch to nightly globally,
change the default with `rustup default nightly`:

```console
$ rustup default nightly
info: using existing install for 'nightly'
info: default toolchain set to 'nightly'

  nightly unchanged: rustc 1.9.0-nightly (02310fd31 2016-03-19)

```

Now any time you run `cargo` or `rustc` you will be running the nightly
compiler.

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
Furthermore, nightly builds may be published with missing non-default
components (e.g. [`clippy`]). As such, it can be difficult to find
fully-working nightlies. Use the [rustup-components-history][rch] project to
find the build status of recent nightly toolchains and components.

[`clippy`]: https://github.com/rust-lang/rust-clippy
[rch]: https://rust-lang.github.io/rustup-components-history/
