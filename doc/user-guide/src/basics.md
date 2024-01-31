# Basic usage

## Keeping Rust up to date

Rust is distributed on three different [release channels]: stable, beta, and
nightly. `rustup` uses the stable channel by default, which
represents the latest release of Rust. Stable publishes new releases every six weeks.

[release channels]: concepts/channels.md

When a new version of Rust is released, simply type `rustup update` to update:

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
info: checking for self-update
info: downloading self-update

  stable updated: rustc 1.7.0 (a5d1e7a59 2016-02-29)

```

## Keeping `rustup` up to date

If your `rustup` was built with the [no-self-update feature](https://github.com/rust-lang/rustup/blob/master/Cargo.toml#L25), it can not update
itself. This is not the default, and only versions of `rustup` built with
`--no-default-features`, or obtained from a third-party distributor who has
disabled it (such as NixOS).

Otherwise Rustup can update itself. It is possible to control Rustup's automatic
self update mechanism with the `auto-self-update` configuration variable. This
setting supports three values: `enable` and `disable` and `check-only`.

* `disable` will ensure that no automatic self updating actions are taken.
* `enable` will mean that `rustup update` and similar commands will also check for, and install, any update to Rustup.
* `check-only` will cause any automatic self update to check and report on any updates, but not to automatically install them.

Whether `auto-self-update` is `enable` or not, you can request that Rustup
update itself to the latest version of `rustup` by running `rustup self update`.
This will not download new toolchains:

```console
$ rustup self update
info: checking for self-update
info: downloading self-update
```

### Disabling self updates on a per-invocation basis
> Self updates can also be suppressed on individual invocations of `rustup` by
> passing the argument `--no-self-update`  when running `rustup update` or
> `rustup toolchain install`.

## Help system

The `rustup` command-line has a built-in help system that provides more
information about each command. Run `rustup help` for an overview. Detailed
help for each subcommand is also available. For example, run `rustup toolchain
install --help` for specifics on installing [toolchains].

[toolchains]: concepts/toolchains.md

