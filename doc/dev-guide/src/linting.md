# Linting

We use `cargo clippy` to ensure high-quality code and to enforce a set of best practices for Rust programming.
However, not all lints provided by `cargo clippy` are relevant or applicable to our project.
We may choose to ignore some lints if they are unstable, experimental, or specific to our project.
If you are unsure about a lint, please ask us in the
[rustup Zulip channel](https://rust-lang.zulipchat.com/#narrow/channel/490103-t-rustup).

## Manual linting

When checking the codebase with [`clippy`](https://doc.rust-lang.org/stable/clippy/index.html),
it is recommended to use the following command:

```console
$ cargo clippy --all --all-targets --all-features -- -D warnings
```

Please note the `--all-features` flag: it is used because we need to enable the `test` feature
to make lints fully work, for which `--all-features` happens to be a convenient shortcut.

The `test` feature is required because `rustup` uses
[cargo features](https://doc.rust-lang.org/cargo/reference/features.html) to
[conditionally compile](https://doc.rust-lang.org/reference/conditional-compilation.html)
support code for integration tests, as `#[cfg(test)]` is only available for unit tests.

If you encounter an issue or wish to speed up the initial analysis, you could also try
activating only the `test` feature by replacing `--all-features` with `--features=test`.

## Rust-Analyzer

To work with the codebase using `rust-analyzer`, you may want to configure it
upfront. To do so, you can find an example configuration file in
`rust-analyzer.example.toml` in the root of the repository. Then, you can copy
it to `rust-analyzer.toml` and adjust the settings as needed.

You might also want to refer to the
[`rust-analyzer` manual](https://rust-analyzer.github.io/manual.html#configuration)
for more details on properly setting up `rust-analyzer` in your IDE of choice.

If you are using `rust-analyzer` within VSCode, you may also add the
corresponding configuration items to your project's
`.vscode/settings.json`[^vscode-global-cfg]. For example:

```toml
[cargo]
features = "all"
```

... will become:

```jsonc
"rust-analyzer.cargo.features": "all",
```

[^vscode-global-cfg]:
    Alternatively, if you want to apply the configuration to all your Rust
    projects, you can add them to your global configuration at
    `~/.config/Code/User/settings.json` instead.

## Checking Windows-specific code on Unix

You can lint Windows-specific code (`#[cfg(windows)]`) without a Windows VM
with `cargo clippy` targeting `x86_64-pc-windows-gnu`.

> **Note**: This is for linting and diagnosis only. For building
> distributable Windows binaries, prefer relying on our CI.

### Prerequisites

You need to install the corresponding cross-compilation target first:

```console
$ rustup target add x86_64-pc-windows-gnu
```

### Recommended method: mingw-w64 gcc

This is the most reliable approach across all platforms. The full gcc
cross-toolchain includes its own sysroot, so no manual `--sysroot` tuning
is needed.

#### Install the dependencies

| Platform      | Install Command                         |
| ------------- | --------------------------------------- |
| Debian/Ubuntu | `sudo apt install gcc-mingw-w64-x86-64` |
| Fedora        | `sudo dnf install mingw64-gcc`          |
| Arch Linux    | `sudo pacman -S mingw-w64-gcc`          |
| macOS         | `brew install mingw-w64`                |

#### Lint

In most cases with mingw-w64-gcc, cargo auto-detects the cross-compiler:

```console
$ cargo clippy --target x86_64-pc-windows-gnu
```

Or modify `.cargo/config.windows-cross.example.toml` based on sysroot matrix, then copy it as `.cargo/config.windows-cross.toml`.

```console
$ cargo clippy --config .cargo/config.windows-cross.toml
```

If your distro does not auto-detect, set the compiler explicitly:

```console
$ CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc \
  CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
  cargo clippy --target x86_64-pc-windows-gnu
```

### Alternate method: clang + mingw-w64 headers

This uses a lighter install (clang + headers only instead of full gcc
toolchain), but requires per-distro sysroot tuning because clang does not
always auto-detect the correct MinGW header paths.

#### Install the dependencies

| Platform      | Install Command                                                |
| ------------- | -------------------------------------------------------------- |
| Debian/Ubuntu | `sudo apt install clang mingw-w64-x86-64-dev`                  |
| Fedora        | `sudo dnf install clang mingw64-headers mingw64-winpthreads`   |
| Arch Linux    | `sudo pacman -S clang mingw-w64-headers mingw-w64-winpthreads` |

#### Lint

When running `clippy`, you may need to inform the Rust toolchain of your sysroot, which requires passing a `cargo` config either via a file or with environment variables.

_Sysroot Matrix_

| Platform      | Need explicit sysroot | Sysroot                                |
| ------------- | --------------------- | -------------------------------------- |
| Debian/Ubuntu | Yes                   | /usr/x86_64-w64-mingw32                |
| Fedora        | No                    | /usr/x86_64-w64-mingw32/sys-root/mingw |
| Arch Linux    | No                    | /usr/x86_64-w64-mingw32                |

Modify `.cargo/config.windows-cross.example.toml` based on sysroot matrix, then copy it as `.cargo/config.windows-cross.toml`.

```console
$ cargo clippy --config .cargo/config.windows-cross.toml
```

Or, you can pass them as environment variables directly, e.g.:

```console
$ CC_x86_64_pc_windows_gnu="clang --sysroot=/usr/x86_64-w64-mingw32" \
  CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=clang \
  cargo clippy --target x86_64-pc-windows-gnu
```

### Rust-Analyzer

It is also possible for the above setup to work with `rust-analyzer`. See the
relevant sections in the [example configuration](#rust-analyzer) for more
information.
