# Toolchains

Many `rustup` commands deal with *toolchains*, a single installation of the
Rust compiler. `rustup` supports multiple types of toolchains. The most basic
track the official release [channels]: *stable*, *beta* and *nightly*; but
`rustup` can also install toolchains from the official archives, for alternate
host platforms, and from local builds.

[channels]: channels.md

## Toolchain specification

Standard release channel toolchain names have the following form:

```
<channel>[-<date>][-<host>]

<channel>       = stable|beta|nightly|<versioned>[-<prerelease>]
<versioned>     = <major.minor>|<major.minor.patch>
<prerelease>    = beta[.<number>]
<date>          = YYYY-MM-DD
<host>          = <target-triple>
```

'channel' is a named release channel, a major and minor version number such as
`1.42`, or a fully specified version number, such as `1.42.0`. Channel names
can be optionally appended with an archive date, as in `nightly-2014-12-18`, in
which case the toolchain is downloaded from the archive for that date.

Finally, the host may be specified as a target triple. This is most useful for
installing a 32-bit compiler on a 64-bit platform, or for installing the
[MSVC-based toolchain][msvc-toolchain] on Windows. For example:

```console
$ rustup toolchain install stable-x86_64-pc-windows-msvc
```

For convenience, elements of the target triple that are omitted will be
inferred, so the above could be written:

```console
$ rustup toolchain install stable-msvc
```

Toolchain names that don't name a channel instead can be used to name [custom
toolchains].

[msvc-toolchain]: https://www.rust-lang.org/tools/install?platform_override=win
[custom toolchains]: #custom-toolchains

## Custom toolchains

For convenience of developers working on Rust itself, `rustup` can manage
local builds of the Rust toolchain. To teach `rustup` about your build, run:

```console
$ rustup toolchain link my-toolchain path/to/my/toolchain/sysroot
```

For example, on Ubuntu you might clone `rust-lang/rust` into `~/rust`, build
it, and then run:

```console
$ rustup toolchain link my-toolchain ~/rust/build/x86_64-unknown-linux-gnu/stage2/
$ rustup default my-toolchain
```

Now you can name `my-toolchain` as any other `rustup` toolchain. Create a
`rustup` toolchain for each of your `rust-lang/rust` workspaces and test them
easily with `rustup run my-toolchain rustc`.

Because the `rust-lang/rust` tree does not include Cargo, *when `cargo` is
invoked for a custom toolchain and it is not available, `rustup` will attempt
to use `cargo` from one of the release channels*, preferring 'nightly', then
'beta' or 'stable'.
