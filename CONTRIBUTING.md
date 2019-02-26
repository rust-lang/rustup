# Contributing to rustup

1. Fork it!
2. Create your feature branch: `git checkout -b my-new-feature`
3. Commit your changes: `git commit -am 'Add some feature'`
4. Push to the branch: `git push origin my-new-feature`
5. Submit a pull request :D

For developing on `rustup` itself, you may want to install into a temporary
directory, with a series of commands similar to this:

```bash
$ cargo build
$ mkdir home
$ RUSTUP_HOME=home CARGO_HOME=home target/debug/rustup-init --no-modify-path -y
```

You can then try out `rustup` with your changes by running `home/bin/rustup`, without
affecting any existing installation. Remember to keep those two environment variables
set when running your compiled `rustup-init` or the toolchains it installs, but _unset_
when rebuilding `rustup` itself.

We use `rustfmt` to keep our codebase consistently formatted.  Please ensure that
you have correctly formatted your code (most editors will do this automatically
when saving) or it may not pass the CI tests.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the
Apache-2.0 license, shall be dual licensed as in the README, without any
additional terms or conditions.

## Making a release

Producing the final release artifacts is a bit involved because of the way Rustup
is distributed. The steps for a release are:

* Update the version number in all Cargo.tomls
* `cargo build` to update the lock files.
* commit and tag (`git commit -a` and `git tag -a $VER_NUM -m $VER_NUM`)
* push the branch and tag (`git push upstream master` and `git push upstream $VER_NUM`)
* ping somebody on the release team to do the final steps.
