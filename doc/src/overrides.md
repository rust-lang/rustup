# Overrides

`rustup` automatically determines which [toolchain] to use when one of the
installed commands like `rustc` is executed. There are several ways to control
and override which toolchain is used:

1. A [toolchain override shorthand] used on the command-line, such as `cargo
   +beta`.
2. The `RUSTUP_TOOLCHAIN` environment variable.
3. A [directory override], set with the `rustup override` command.
4. The [`rustup-toolchain`] file.
5. The [default toolchain].

The toolchain is chosen in the order listed above, using the first one that is
specified. There is one exception though: directory overrides and the
`rust-toolchain` file are also preferred by their proximity to the current
directory. That is, these two override methods are discovered by walking up
the directory tree toward the filesystem root, and a `rust-toolchain` file
that is closer to the current directory will be preferred over a directory
override that is further away.

To verify which toolchain is active use `rustup show`.

[toolchain]: concepts/toolchains.md
[toolchain override shorthand]: #toolchain-override-shorthand
[directory override]: #directory-overrides
[`rustup-toolchain`]: #the-toolchain-file
[default toolchain]: #default-toolchain

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

The per-directory overrides are stored in [a configuration file] in `rustup`'s
home directory.

[a configuration file]: configuration.md

## The toolchain file

Some projects find themselves 'pinned' to a specific release of Rust and want
this information reflected in their source repository. This is most often the
case for nightly-only software that pins to a revision from the release
archives.

In these cases the toolchain can be named in the project's directory in a file
called `rust-toolchain`, the content of which is the name of a single `rustup`
toolchain, and which is suitable to check in to source control. This file has
to be encoded in US-ASCII (if you are on Windows, check the encoding and that
it does not starts with a BOM).

The toolchains named in this file have a more restricted form than `rustup`
toolchains generally, and may only contain the names of the three release
channels, 'stable', 'beta', 'nightly', Rust version numbers, like '1.0.0', and
optionally an archive date, like 'nightly-2017-01-01'. They may not name
custom toolchains, nor host-specific toolchains.

## Default toolchain

If no other overrides are set, the global default toolchain will be used. This
default can be chosen when `rustup` is [installed]. The `rustup default`
command can be used to set and query the current default. Run `rustup default`
without any arguments to print the current default. Specify a toolchain as an
argument to change the default:

```console
rustup default nightly-2020-07-27
```

[installed]: installation/index.md
