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

## Version numbers

If you ever see a released version of rustup which has `::` in its version string
then something went wrong with the CI and that needs to be addressed.

We use `git-testament` to construct our version strings. This records, as a
struct, details of the git commit, tag description, and also an indication
of modifications to the working tree present when the binary was compiled.

During normal development you may get information from invoking `rustup --version`
which looks like `rustup-init 1.18.3+15 (a54051502 2019-05-26)` or even
`rustup-init 1.18.3+15 (a54051502 2019-05-26) dirty 1 modification`.

The first part is always the binary name as per `clap`'s normal operation. The
version number is a combination of the most recent tag in the git repo, and the
number of commits since that tag. The parenthesised information is, naturally,
the SHA of the most recent commit and the date of that commit. If the indication
of a dirty tree is present, the number of changes is indicated.  This combines
adds, deletes, modifies, and unknown entries.

You can request further information of a `rustup` binary with the
`rustup dump-testament` hidden command.  It produces output of the form:

```shell
$ rustup dump-testament
Rustup version renders as: 1.18.3+15 (a54051502 2019-05-26) dirty 1 modification
Current crate version: 1.18.3
Built from branch: kinnison/version-strings
Commit info: 1.18.3+15 (a54051502 2019-05-26)
Modified: CONTRIBUTING.md
```

This can be handy when you are testing development versions on your PC
and cannot remember exactly which version you had installed, or if you have given
a development copy (or instruction to build such) to a user, and wish to have them
confirm *exactly* what they are using.

Finally, we tell `git-testament` that we trust the `stable` branch to carry
releases.  If the build is being performed when not on the `stable` branch, and
the tag and `CARGO_PKG_VERSION` differ, then the short version string will include
both, in the form `rustup-init 1.18.3 :: 1.18.2+99 (a54051502 2019-05-26)` which
indicates the crate version before the rest of the commit.
On the other hand, if the build was on the `stable` branch then regardless
of the tag information, providing the commit was clean, the version is
always replaced by the crate version.  The `dump-testament` hidden command can
reveal the truth however.

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
