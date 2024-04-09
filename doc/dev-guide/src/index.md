# Contributing to rustup

1. Fork it!
2. Create your feature branch: `git checkout -b my-new-feature`
3. Test it: `cargo test --features=test`
4. [Lint it!](linting.md)
5. Commit your changes: `git commit -am 'Add some feature'`
6. Push to the branch: `git push origin my-new-feature`
7. Submit a pull request :D

For developing on `rustup` itself, you may want to install into a temporary
directory, with a series of commands similar to this:

```bash
cargo build
mkdir home
RUSTUP_HOME=home CARGO_HOME=home target/debug/rustup-init --no-modify-path -y
```

You can then try out `rustup` with your changes by running `home/bin/rustup`, without
affecting any existing installation. Remember to keep those two environment variables
set when running your compiled `rustup-init` or the toolchains it installs, but _unset_
when rebuilding `rustup` itself.

If you wish to install your new build to try out longer term in your home directory
then you can run `cargo dev-install` which is an alias in `.cargo/config` which
runs `cargo run -- --no-modify-path -y` to install your build into your homedir.

We use `rustfmt` to keep our codebase consistently formatted. Please ensure that
you have correctly formatted your code (most editors will do this automatically
when saving) or it may not pass the CI tests.

If you are moving, renaming or removing an existing mdBook page, please use mdBook's
[`output.html.redirect`] feature to ensure that the old URL gets redirected.

[`output.html.redirect`]: https://rust-lang.github.io/mdBook/format/configuration/renderers.html#outputhtmlredirect

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the
Apache-2.0 license, shall be dual licensed as in the README, without any
additional terms or conditions.
