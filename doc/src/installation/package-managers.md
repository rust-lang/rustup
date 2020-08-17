# Package managers

Several Linux distributions package Rust, and you may wish to use the packaged
toolchain, such as for distribution package development. You may also wish to
use a `rustup`-managed toolchain such as nightly or beta. Normally, `rustup`
will complain that you already have Rust installed in `/usr` and refuse to
install. However, you can install Rust via `rustup` and have it coexist with
your distribution's packaged Rust.

When you initially install Rust with `rustup`, pass the `-y` option to make it
ignore the packaged Rust toolchain and install a `rustup`-managed toolchain
into `~/.cargo/bin`. Add that directory to your `$PATH` (or let `rustup` do it
for you by not passing `--no-modify-path`). Then, to tell `rustup` about your
system toolchain, run:

```console
rustup toolchain link system /usr
```

You can then use "system" as a `rustup` toolchain, just like "nightly".
For example, using the [toolchain override shorthand], you can run `cargo +system build`
to build with the system toolchain, or `cargo +nightly build` to build with nightly.

If you do distribution Rust development, you should likely make "system" your
[default toolchain]:

```console
rustup default system
```

[toolchain override shorthand]: ../overrides.md#toolchain-override-shorthand
[default toolchain]: ../overrides.md#default-toolchain
