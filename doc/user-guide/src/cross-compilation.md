# Targets and cross-compilation

Rust [supports a great number of platforms][p]. For many of these platforms,
the Rust Project publishes binary releases of the standard library, and for
some the full compiler. `rustup` gives easy access to all of them.

[p]: https://doc.rust-lang.org/nightly/rustc/platform-support.html

## Targets

Before going further, it is important to know the distinction between a
toolchain's _host platform_ and the _compilation targets_ the toolchain supports.

For example, if you develop on Linux and have installed the
`stable-x86_64-unknown-linux-gnu` toolchain, `x86_64-unknown-linux-gnu` is that
toolchain's _host tuple_. When this toolchain is first installed, it can
already compile to that same target, because it comes with the _target_ support
for the host tuple preinstalled.

Note that with _the same toolchain_, you can already compile to various other
_targets_, such as:

- `x86_64-unknown-linux-musl`
- `i686-unknown-linux-gnu`
- `armv7-unknown-linux-gnueabihf`

For this to work, you will need to install these targets to your toolchain, as
explained in the next section.

> NOTE: For this reason, it is generally recommended to rely on a single host
> tuple of your choice and compile to other targets as needed. For example, you
> don't need a `i686-pc-windows-msvc` host toolchain on your `x86_64` Windows
> machine to compile and test `i686-msvc` projects. Instead, you may want to
> stick to your existing `stable-x86_64-pc-windows-msvc` (or
> `stable-x86_64-pc-windows-gnu`) toolchain with the `i686-pc-windows-msvc`
> target added to it.

## Installing a new target

To install a new target to the _active toolchain_, you can run `rustup target
add` followed by the target tuple. Let's take Android as an example:

```console
$ rustup target add arm-linux-androideabi
info: downloading component 'rust-std' for 'arm-linux-androideabi'
info: installing component 'rust-std' for 'arm-linux-androideabi'
```

You can then build for Android with Cargo by passing the `--target` flag, as in
`cargo build --target=arm-linux-androideabi`.

Note that `rustup target add` only installs the Rust standard library for a
given target. This is sufficient for certain host/target combinations, but for
others, there are typically other tools necessary to cross-compile,
particularly a linker. For example, to cross compile to Android the [Android
NDK] must be installed. In the future, `rustup` will provide assistance
installing the NDK components as well. See the [target section] of the `cargo`
configuration for how to setup a linker to use for a certain target.

[Android NDK]: https://developer.android.com/tools/sdk/ndk/index.html
[target section]: https://doc.rust-lang.org/cargo/reference/config.html#target

## Managing targets

Below are some common commands to manage the targets for a given toolchain,
defaulting to the active one:

- To install a new target: `rustup target add`.
- To see a list of available targets: `rustup target list`.
- To see a list of installed targets: `rustup target list --installed`.
- To remove a previously-installed target: `rustup target remove`.

To do the same on a different toolchain than the active one, you can use
`--toolchain` like so:

```console
$ rustup target add --toolchain <toolchain> <target>...
```
