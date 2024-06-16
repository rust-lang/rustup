# Linting

We use `cargo clippy` to ensure high-quality code and to enforce a set of best practices for Rust programming.
However, not all lints provided by `cargo clippy` are relevant or applicable to our project.
We may choose to ignore some lints if they are unstable, experimental, or specific to our project.
If you are unsure about a lint, please ask us in the [rustup Discord channel](https://discord.com/channels/442252698964721669/463480252723888159).

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

When checking the codebase using `rust-analyzer`, the first thing to do remains unchanged:
enabling the features.

This is done by setting the `rust-analyzer.cargo.features` property to `"all"`.

For example, if you are using `rust-analyzer` within VSCode, you would want to
add the following to your project's `.vscode/settings.json`[^vscode-global-cfg]:

```jsonc
"rust-analyzer.cargo.features": "all",
```

[^vscode-global-cfg]:
    Alternatively, if you want to apply the configuration to all your Rust projects,
    you can add it to your global configuration at `~/.config/Code/User/settings.json` instead.

Alternatively, if you want to enable the `test` feature only, you should set the
following instead:

```jsonc
"rust-analyzer.cargo.features": ["test"]
```

Next, as `rust-analyzer` depends on `cargo check` by default, it is also recommended to
enable the `cargo clippy` integration by adding the following:

```jsonc
"rust-analyzer.check.command": "clippy",
```

You might also want to refer to the
[`rust-analyzer` manual](https://rust-analyzer.github.io/manual.html#configuration)
for more details on properly setting up `rust-analyzer` in your IDE of choice.
