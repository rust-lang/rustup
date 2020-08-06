# Basic usage

## Keeping Rust up to date

Rust is distributed on three different [release channels]: stable, beta, and
nightly. `rustup` is configured to use the stable channel by default, which
represents the latest release of Rust, and is released every six weeks.

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

## Keeping rustup up to date

Running `rustup update` also checks for updates to `rustup` and automatically
installs the latest version. To manually check for updates and install the
latest version of `rustup` without updating installed toolchains type `rustup
self update`:

```console
$ rustup self update
info: checking for self-updates
info: downloading self-updates
```

**Note**: `rustup` will automatically update itself at the end of any
toolchain installation as well.  You can prevent this automatic behaviour by
passing the `--no-self-update` argument when running `rustup update` or
`rustup toolchain install`.
