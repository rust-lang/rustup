# Cross-compilation

Rust [supports a great number of platforms][p]. For many of these platforms
The Rust Project publishes binary releases of the standard library, and for
some the full compiler. `rustup` gives easy access to all of them.

[p]: https://doc.rust-lang.org/nightly/rustc/platform-support.html

When you first install a toolchain, `rustup` installs only the standard
library for your *host* platform - that is, the architecture and operating
system you are presently running. To compile to other platforms you must
install other *target* platforms. This is done with the `rustup target add`
command. For example, to add the Android target:

```console
$ rustup target add arm-linux-androideabi
info: downloading component 'rust-std' for 'arm-linux-androideabi'
info: installing component 'rust-std' for 'arm-linux-androideabi'
```

With the `arm-linux-androideabi` target installed you can then build for
Android with Cargo by passing the `--target` flag, as in `cargo build
--target=arm-linux-androideabi`.

Note that `rustup target add` only installs the Rust standard library for a
given target. There are typically other tools necessary to cross-compile,
particularly a linker. For example, to cross compile to Android the [Android
NDK] must be installed. In the future, `rustup` will provide assistance
installing the NDK components as well. See the [target section] of the
`cargo` configuration for how to setup a linker to use for a certain target.

[Android NDK]: https://developer.android.com/tools/sdk/ndk/index.html
[target section]: https://doc.rust-lang.org/cargo/reference/config.html#target

To install a target for a toolchain that isn't the default toolchain use the
`--toolchain` argument of `rustup target add`, like so:

```console
$ rustup target add --toolchain <toolchain> <target>...
```

To see a list of available targets, `rustup target list`. To remove a
previously-added target, `rustup target remove`.
