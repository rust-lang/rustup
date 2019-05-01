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

Before making a release, ensure that `rustup-init.sh` is behaving correctly,
and that you're satisfied that nothing in the ecosystem is breaking because
of the update.  A useful set of things to check includes verifying that
real-world toolchains install okay, and that `rls-vscode` isn't broken by
the release.  While it's not our responsibility if they depend on non-stable
APIs, we should behave well if we can.

Producing the final release artifacts is a bit involved because of the way
Rustup is distributed. The steps for a release are:

1. Update all `Cargo.toml` to have the new version
   (optionally make a commit)
2. Run `cargo build` and review `Cargo.lock` changes
   if all looks well, make a commit
3. Update `rustup-init.sh` version to match the commit
   details for `Cargo.lock`
4. Push this to the `stable` branch (git push origin HEAD:stable)
5. While you wait for green CI, double-check the `rustup-init.sh` functionality
   and `rustup-init` just in case.
6. Ensure all of CI is green on the `stable` branch.
   Once it is, check through a representative proportion of the builds looking
   for the reported version statements to ensure that we definitely built something
   cleanly which reports as the right version number when run `--version`.
7. Ping someone in the release team to perform the actual release.
   They can find instructions in `ci/sync-dist.py`
   Note: Some manual testing occurs here, so hopefully they'll catch
   anything egregious in which case abort the change and roll back.
8. Once the official release has happened, prepare and push a tag
   of that commit, and also push the content to master
   * `git tag -as $VER_NUM -m $VER_NUM`  (optionally without -s if not GPG
     signing the tag)
   * `git push origin HEAD:master`
   * `git push origin $VER_NUM`
