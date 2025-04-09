# Already installed Rust?

Other package managers also ship Rust, and you may wish to use the packaged
toolchain, such as for distribution package development. You may also wish to
use a `rustup`-managed toolchain such as nightly or beta. Normally, `rustup`
will complain that you already have Rust installed in `/usr` and refuse to
install. However, you can install Rust via `rustup` and have it coexist with
your packaged Rust toolchain.

## Set up rustup with an existing Rust toolchain

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

If you wish to develop with the system toolchain (e.g. for distribution packages),
you may want to make it your [default toolchain]:

```console
rustup default system
```

## Ensure the correct `$PATH` configuration

There are times when the above steps don't work, and you may see strange error
messages when running commands that should have been proxied by rustup.
For example, when running `cargo +stable --version`, you may encounter the
following error:

```text
error: no such command: `+stable`

        Cargo does not handle `+toolchain` directives.
        Did you mean to invoke `cargo` through `rustup` instead?
```

This means `cargo` is currently not a `rustup` proxy, and your `$PATH` needs
to be fixed.

In fact, on any machine with rustup installed, you would like to have **rustup
proxies showing up first in `$PATH`**, shadowing any other Rust installations.
Don't worry: these shadowed installations can then be adopted by rustup with the
`rustup toolchain link` command as mentioned above.

The exact steps to be taken to make rustup proxies come first may vary according
to your system environment, but usually it is about changing the evaluation
order of certain lines in your shell configuration file(s).

To make it clearer, let's look at the example of a Mac with both regular rustup
fetched from [rustup.rs] and homebrew-installed `rust`.
The **right way** to configure `.profile` in this environment would be:

```bash
eval $(/opt/homebrew/bin/brew shellenv)
. $HOME/.cargo/env
```

In this example, both of these lines all _prepend_ to `$PATH`, so the last one
takes over, letting the rustup proxies shadow the homebrew-installed `rust`.
On the other hand, putting these lines the other way around will cause the
aforementioned error.

When in doubt, you can always debug your shell configuration by printing the
status of your current `$PATH` with `echo $PATH | xargs -n1` and paying
attention to the order of `$CARGO_HOME/bin` (which defaults to
`$HOME/.cargo/bin`) compared to your package manager's `bin` directory.

After the fix, the output of `cargo +stable --version` should be similar to one
of the following, depending on whether you have had the `stable` toolchain
installed:

- ```text
  cargo 1.85.1 (d73d2caf9 2024-12-31)
  ```

- ```text
  error: toolchain 'stable' is not installed
  ```

[rustup.rs]: https://rustup.rs
[toolchain override shorthand]: ../overrides.md#toolchain-override-shorthand
[default toolchain]: ../overrides.md#default-toolchain
