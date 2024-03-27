# Linting

## Manual linting

When checking the codebase with [`clippy`](https://doc.rust-lang.org/stable/clippy/index.html), it is recommended to use:

```console
$ cargo clippy --all --all-targets --all-features -- -D warnings
```

## Rust-Analyzer

When using  [`rust-analyzer`](https://rust-analyzer.github.io/) integration in the IDE of your choice, you might want to set the `rust-analyzer.cargo.features` configuration to `"all"` (check the [`rust-analyzer` manual](https://rust-analyzer.github.io/manual.html#configuration) for more details).

### VSCode/VSCodium setup

Add 

```json
"rust-analyzer.cargo.features": "all":,
```

in your project at `.vscode/settings.json`

or

to your global configuration `~/.config/Code/User/settings.json` (although you need to be aware that this will apply to all your Rust projects).


## Rationale

`rustup` uses cargo [features](https://doc.rust-lang.org/cargo/reference/features.html) in order to setup [conditional compilation](https://doc.rust-lang.org/reference/conditional-compilation.html) for integration tests as the `#[cfg(test)]` is only available for unit tests. To this end, the `test` feature has been created, however it then needs to be activated in order for tests and linting to fully work. As a shortcut we then propose to activate all features. However, if you encounter an issue, you could try activating only the `test` feature by setting the `rust-analyzer.cargo.features` configuration to `["test"]`.

