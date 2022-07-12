# Channels

Rust is released to three different "channels": stable, beta, and nightly. The
stable releases are made every 6 weeks (with occasional point releases). Beta
releases are the version that will appear in the next stable release. Nightly
releases are made every night. See [The Rust Book][channels] for more details
on Rust's train release model. The release schedule is posted to the [Rust
Forge]. `rustup` assists with installing different channels, keeping them
up-to-date, and easily switching between them.

After a release channel has been installed, `rustup` can be used to update the
installed version to the latest release on that channel. See the [Keeping rust
up to date] section for more information.

`rustup` can also install specific versions of Rust, such as `1.45.2` or
`nightly-2020-07-27`. See the [Toolchains] chapter for more information on
installing different channels and releases. See the [Overrides] chapter for
details on switching between toolchains and pinning your project to a specific
toolchain.

[channels]: https://doc.rust-lang.org/book/appendix-07-nightly-rust.html
[Keeping rust up to date]: ../basics.md#keeping-rust-up-to-date
[rust forge]: https://forge.rust-lang.org/
[toolchains]: toolchains.md

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
change [the default] with `rustup default nightly`:

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
info: checking for self-update
info: downloading self-update

   stable unchanged: rustc 1.7.0 (a5d1e7a59 2016-02-29)
  nightly unchanged: rustc 1.9.0-nightly (02310fd31 2016-03-19)

```

[the default]: ../overrides.md#default-toolchain

## Nightly availability

Nightly toolchains may fail to build, so for any given date and target
platform there may not be a toolchain available. Furthermore, nightly builds
may be published with missing non-default [components] (such as [`clippy`]).
As such, it can be difficult to find fully-working nightlies. Use the
[rustup-components-history][rch] project to find the build status of recent
nightly toolchains and components.

When you attempt to install or update the `nightly` channel, `rustup` will
check if a required or previously installed component is missing. If it is
missing, `rustup` will automatically search for an older release that contains
the required components. There are several ways to change this behavior:

* Use the `--force` flag to `rustup toolchain install` to force it to install
  the most recent version even if there is a missing component.
* Use the `--profile` flag to `rustup toolchain install` to use a different
  profile that does not contain the missing component. For example,
  `--profile=minimal` should always work, as the minimal set is required to
  exist. See the [Profiles] chapter for more detail.
* Install a specific date that contains the components you need. For example,
  `rustup toolchain install nightly-2020-07-27`. You can then use [overrides]
  to pin to that specific release.

[`clippy`]: https://github.com/rust-lang/rust-clippy
[rch]: https://rust-lang.github.io/rustup-components-history/
[components]: components.md
[profiles]: profiles.md
[overrides]: ../overrides.md
