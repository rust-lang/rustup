# Overrides

There are several ways to specify which toolchain `rustup` should execute:

* An explicit toolchain, e.g. `cargo +beta`,
* The `RUSTUP_TOOLCHAIN` environment variable,
* A directory override, ala `rustup override set beta`,
* The `rust-toolchain` file,
* The default toolchain,

and they are preferred by `rustup` in that order, with the explicit toolchain
having highest precedence, and the default toolchain having the lowest. There
is one exception though: directory overrides and the `rust-toolchain` file are
also preferred by their proximity to the current directory. That is, these two
override methods are discovered by walking up the directory tree toward the
filesystem root, and a `rust-toolchain` file that is closer to the current
directory will be preferred over a directory override that is further away.

To verify which toolchain is active use `rustup show`.

## Toolchain override shorthand

The `rustup` toolchain proxies can be instructed directly to use a specific
toolchain, a convenience for developers who often test different toolchains.
If the first argument to `cargo`, `rustc` or other tools in the toolchain
begins with `+`, it will be interpreted as a `rustup` toolchain name, and that
toolchain will be preferred, as in

```console
cargo +beta test
```

## Directory overrides

Directories can be assigned their own Rust toolchain with `rustup override`.
When a directory has an override then any time `rustc` or `cargo` is run
inside that directory, or one of its child directories, the override toolchain
will be invoked.

To use to a specific nightly for a directory:

```console
rustup override set nightly-2014-12-18
```

Or a specific stable release:

```console
rustup override set 1.0.0
```

To see the active toolchain use `rustup show`. To remove the override and use
the default toolchain again, `rustup override unset`.

## The toolchain file

`rustup` directory overrides are a local configuration, stored in
`$RUSTUP_HOME`. Some projects though find themselves 'pinned' to a specific
release of Rust and want this information reflected in their source
repository. This is most often the case for nightly-only software that pins to
a revision from the release archives.

In these cases the toolchain can be named in the project's directory in a file
called `rust-toolchain`, the content of which is either the name of a single
`rustup` toolchain, or a TOML file with the following layout:

``` toml
[toolchain]
channel = "nightly-2020-07-10"
components = [ "rustfmt", "rustc-dev" ]
targets = [ "wasm32-unknown-unknown", "thumbv2-none-eabi" ]
```

If the TOML format is used, the `[toolchain]` section is mandatory, and at
least one property must be specified.

The `rust-toolchain` file is suitable to check in to source control. This file
has to be encoded in US-ASCII (if you are on Windows, check the encoding and
that it does not starts with a BOM).

The toolchains named in this file have a more restricted form than `rustup`
toolchains generally, and may only contain the names of the three release
channels, 'stable', 'beta', 'nightly', Rust version numbers, like '1.0.0', and
optionally an archive date, like 'nightly-2017-01-01'. They may not name
custom toolchains, nor host-specific toolchains.
