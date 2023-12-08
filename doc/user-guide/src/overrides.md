# Overrides

`rustup` automatically determines which [toolchain] to use when one of the
installed commands like `rustc` is executed. There are several ways to control
and override which toolchain is used:

1. A [toolchain override shorthand] used on the command-line, such as `cargo
   +beta`.
2. The `RUSTUP_TOOLCHAIN` environment variable.
3. A [directory override], set with the `rustup override` command.
4. The [`rust-toolchain.toml`] file.
5. The [default toolchain].

The toolchain is chosen in the order listed above, using the first one that is
specified. There is one exception though: directory overrides and the
`rust-toolchain.toml` file are also preferred by their proximity to the current
directory. That is, these two override methods are discovered by walking up
the directory tree toward the filesystem root, and a `rust-toolchain.toml` file
that is closer to the current directory will be preferred over a directory
override that is further away.

To verify which toolchain is active, you can use `rustup show`.

[toolchain]: concepts/toolchains.md
[toolchain override shorthand]: #toolchain-override-shorthand
[directory override]: #directory-overrides
[`rust-toolchain.toml`]: #the-toolchain-file
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
called `rust-toolchain.toml` or `rust-toolchain`. If both files are present in
a directory, the latter is used for backwards compatibility. The files use the
[TOML] format and have the following layout:

[TOML]: https://toml.io/

``` toml
[toolchain]
channel = "nightly-2020-07-10"
components = [ "rustfmt", "rustc-dev" ]
targets = [ "wasm32-unknown-unknown", "thumbv2-none-eabi" ]
profile = "minimal"
```

The `[toolchain]` section is mandatory, and at least one property must be
specified. `channel` and `path` are mutually exclusive.

For backwards compatibility, `rust-toolchain` files also support a legacy
format that only contains a toolchain name without any TOML encoding, e.g.
just `nightly-2021-01-21`. The file has to be encoded in US-ASCII in this case
(if you are on Windows, check the encoding and that it does not start with a
BOM). The legacy format is not available in `rust-toolchain.toml` files.

If you see the following error (when running `rustc`, `cargo` or other command)

```
error: invalid channel name '[toolchain]' in '/PATH/TO/DIRECTORY/rust-toolchain'
```

it means you're running `rustup` pre-1.23.0 and trying to interact with a project
that uses the new TOML encoding in the `rust-toolchain` file. You need to upgrade
`rustup` to 1.23.0+.

The `rust-toolchain.toml`/`rust-toolchain` files are suitable to check in to
source control. If that's done, `Cargo.lock` should probably be tracked too if
the toolchain is pinned to a specific release, to avoid potential compatibility
issues with dependencies.

### Toolchain file settings

#### channel

The `channel` setting specifies which [toolchain] to use. The value is a
string in the following form:

```
(<channel>[-<date>])|<custom toolchain name>

<channel>       = stable|beta|nightly|<major.minor.patch>
<date>          = YYYY-MM-DD
```

[toolchain]: concepts/toolchains.md

#### path

The `path` setting allows a custom toolchain to be used. The value is an
absolute path string.

Since a `path` directive directly names a local toolchain, other options
like `components`, `targets`, and `profile` have no effect.

`channel` and `path` are mutually exclusive, since a `path` already
points to a specific toolchain.

#### profile

The `profile` setting names a group of components to be installed. The
value is a string. The valid options are: `minimal`, `default`, and
`complete`. See [profiles] for details of each.

Note that if not specified, the `default` profile is not necessarily
used, as a different default profile might have been set with `rustup
set profile`.

[profiles]: concepts/profiles.md

#### components

The `components` setting contains a list of additional components to
install. The value is a list of strings. See [components] for a list of
components. Note that different toolchains may have different components
available.

The components listed here are additive with the current profile.

[components]: concepts/components.md

#### targets

The `targets` setting contains a list of platforms to install for
[cross-compilation]. The value is a list of strings.

The host platform is automatically included; the targets listed here are
additive.

[cross-compilation]: https://rust-lang.github.io/rustup/cross-compilation.html

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
[#1397]: https://github.com/rust-lang/rustup/issues/1397
