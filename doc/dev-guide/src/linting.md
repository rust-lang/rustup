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
