# Contributing to rustup

1. Fork it!
2. Create your feature branch: `git checkout -b my-new-feature`
3. Test it: `cargo test --features=test`
4. [Lint it!](linting.md)
5. Commit your changes: `git commit -am 'Add some feature'`
6. Push to the branch: `git push origin my-new-feature`
7. Submit a pull request :D

For developing on `rustup` itself, the easiest way is to run the development
build on your current installation. This approach is best used for minor fixes
or improvements. See the documentation for [`cargo run-rustup`] and
[`RUSTUP_FORCE_ARG0`] for more info.

[`cargo run-rustup`]: tips-and-tricks.md#cargo-run-rustup
[`RUSTUP_FORCE_ARG0`]: tips-and-tricks.md#rustup_force_arg0

A more formal solution involves installing rustup into a temporary directory as
your dedicated test environment.
To do so, you can run a series of commands similar to this:

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

## AI policy

Using AI (LLMs) as tools for coding is welcome. A high bar is held for all contributions to this
project. Moreover, the project maintainers remain responsible for any code that is published as
part of a release. Contributors are expected to be responsible for any code they publish.

AI should not be used to generate comments when communicating with maintainers. Comments are
expected to be written by humans. Comments that are believed to be written by AI may be hidden
without notice.

If you are opening an issue, you should be able to describe the problem in your own words.

If you are opening a pull request, you are expected to be able to explain the proposed changes in
your own words. This includes the pull request body and responses to questions. Make sure you have
reviewed the PR yourself before submitting it for review to the maintainers. Do not copy responses
from the AI when replying to questions from maintainers. As an exception, issues marked as `E-easy`
are meant for new contributors as a learning opportunity; the use of an LLM when submitting PR for
such issues is disallowed except without explicit permission from the team. Failure to comply may
result in the PR being closed directly without further notice.

If you wish to include context from an interaction with AI in your comments, it must be in a
quote block (using `>`) and disclosed as such. It must be accompanied by human commentary
explaining the relevance and implications of the context. Do not share long snippets.

AI is useful when communicating as a non-native English speaker. If you are using AI to edit your
comments for this purpose, please take the time to ensure it reflects your own voice and ideas.
When using AI for translation, we recommend writing in your native language and including the AI
translation in a quote block.
