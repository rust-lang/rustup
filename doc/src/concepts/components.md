# Components

Each [toolchain] has several "components", some of which are required (like
`rustc`) and some that are optional (like [`clippy`][clippy]). The `rustup
component` command is used to manage the installed components. For example,
run `rustup component list` to see a list of available and installed
components.

Components can be added when installing a toolchain with the `--component`
flag. For example:

```console
rustup toolchain install nightly --component rust-docs
```

Components can be added to an already-installed toolchain with the `rustup
component` command:

```console
rustup component add rust-docs
```

To make it easier to choose which components are installed, `rustup` has the
concept of "profiles" which provide named groupings of different components.
See the [Profiles] chapter for more detail.

Most components have a target-triple suffix, such as
`rustc-x86_64-apple-darwin`, to signify the platform the component is for.

The set of available components may vary with different releases and
toolchains. The following is an overview of the different components:

* `rustc` — The Rust compiler and [Rustdoc].
* `cargo` — [Cargo] is a package manager and build tool.
* `rustfmt` — [Rustfmt] is a tool for automatically formatting code.
* `rust-std` — This is the Rust [standard library]. There is a separate
  `rust-std` component for each target that `rustc` supports, such as
  `rust-std-x86_64-pc-windows-msvc`. See the [Cross-compilation] chapter for
  more detail.
* `rust-docs` — This is a local copy of the [Rust documentation]. Use the
  `rustup doc` command to open the documentation in a web browser. Run `rustup
  doc --help` for more options.
* `rls` — [RLS] is a language server that provides support for editors and
  IDEs.
* `clippy` — [Clippy] is a lint tool that provides extra checks for common
  mistakes and stylistic choices.
* `miri` — [Miri] is an experimental Rust interpreter, which can be used for
  checking for undefined-behavior.
* `rust-src` — This is a local copy of the source code of the Rust standard
  library. This can be used by some tools, such as [RLS], to provide
  auto-completion for functions within the standard library; [Miri] which is a
  Rust interpreter; and Cargo's experimental [build-std] feature, which allows
  you to rebuild the standard library locally.
* `rust-analysis` — Metadata about the standard library, used by tools like
  [RLS].
* `rust-mingw` — This contains a linker and platform libraries for building on
  the `x86_64-pc-windows-gnu` platform.
* `llvm-tools-preview` — This is an experimental component which contains a
  collection of [LLVM] tools.
* `rustc-dev` — This component contains the compiler as a library. Most users
  will not need this; it is only needed for development *of* tools that link
  to the compiler, such as making modifications to [Clippy].

## Component availability

Not all components are available for all toolchains. Especially on the nightly
channel, some components may not be included if they are in a broken state.
The current status of all the components may be found on the [rustup
components history] page. See the [Nightly availability] section for more
details.

[toolchain]: toolchains.md
[standard library]: https://doc.rust-lang.org/std/
[rust documentation]: https://doc.rust-lang.org/
[cross-compilation]: ../cross-compilation.md
[build-std]: https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#build-std
[miri]: https://github.com/rust-lang/miri/
[RLS]: https://github.com/rust-lang/rls
[rustdoc]: https://doc.rust-lang.org/rustdoc/
[cargo]: https://doc.rust-lang.org/cargo/
[clippy]: https://github.com/rust-lang/rust-clippy
[LLVM]: https://llvm.org/
[rustfmt]: https://github.com/rust-lang/rustfmt
[rustup components history]: https://rust-lang.github.io/rustup-components-history/
[profiles]: profiles.md
[nightly availability]: channels.md#nightly-availability
