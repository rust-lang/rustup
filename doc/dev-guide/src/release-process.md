# Making a release

Before making a release, ensure that `rustup-init.sh` is behaving correctly,
and that you're satisfied that nothing in the ecosystem is breaking because
of the update. A useful set of things to check includes verifying that
real-world toolchains install okay, and that `rust-analyzer` isn't broken by
the release. While it's not our responsibility if they depend on non-stable
APIs, we should behave well if we can.

As a maintainer, you have two options to choose from when cutting a new
release: a beta release or an official release.
The main difference between the two is that they use different values for
the `RUSTUP_UPDATE_ROOT` environment variable:
- A beta release is deployed on `https://dev-static.rust-lang.org/rustup`.
- An official release is deployed on `https://static.rust-lang.org/rustup`.

By switching between those two values, Rustup effectively provides two "self
update channels", making beta testing possible with `rustup self update`.

Producing the final release artifacts is a bit involved because of the way
Rustup is distributed.
Below is a list of things to be done in order to cut a new [b]eta release
or an official [r]elease:

1. [b/r] In a separate PR:
   1. If the version strings in `Cargo.toml`s haven't been updated:
      - Decide what the new version number `$VER_NUM` should be.
        > **Note:** We always increment the *minor* number unless:
        > - A major incompatibility has been introduced in this release:
        >   increment the *major* number instead.
        > - This release is a hotfix because the last one had a defect:
        >   increment the *patch* number instead.
      - Update `Cargo.toml` and `download/Cargo.toml` to have that same new
        version number, then run `cargo build` and review `Cargo.lock` changes.
      If all looks well, make a commit.
   2. Update `CHANGELOG.md` accordingly if necessary.
2. [b/r] After merging the PR made in step 1, in a separate PR:
   1. Update the commit shasum in `rustup-init.sh` to match the latest commit
      on `master`.
3. [b/r] After merging the PR made in step 2, sync `master` to `stable` using
   `--ff-only`:
   - `git fetch origin master:master`
   - `git checkout stable && git merge --ff-only master`
   - `git push origin HEAD:stable`
4. [b/r] While you wait for green CI on `stable`, double-check the
   functionality of `rustup-init.sh` and `rustup-init` just in case.
5. [b/r] Ensure all of CI is green on the `stable` branch.
   Once it is, check through a representative proportion of the builds looking
   for the reported version statements to ensure that
   we definitely built something cleanly which reports as the right version
   number when run `--version`.
6. [r] Make a new PR to the [Rust Blog] adding a new release announcement post.
7. [b/r] Ping someone in the release team to perform the actual release.
   They can find instructions in `ci/sync-dist.py`.
   > **Note:** Some manual testing occurs here, so hopefully they'll catch
     anything egregious in which case abort the change and roll back.
8. [b] Once the beta release has happened, post a new topic named "Seeking beta
   testers for Rustup $VER_NUM" on the [Internals Forum].
9. [r] Once the official release has happened, prepare and push a tag on the
   latest `stable` commit.
   - `git tag -as $VER_NUM -m $VER_NUM` (optionally without `-s` if not GPG
     signing the tag)
   - `git push origin $VER_NUM`

[Rust Blog]: https://github.com/rust-lang/blog.rust-lang.org
[Internals Forum]: https://internals.rust-lang.org
