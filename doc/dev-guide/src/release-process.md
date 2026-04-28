# Making a release

In rustup, there are two possible release "modes": the beta release and the
official release. The main difference between the two is that they use
different values for the `RUSTUP_UPDATE_ROOT` environment variable:

- A beta release is deployed on `https://dev-static.rust-lang.org/rustup`.
- An official release is deployed on `https://static.rust-lang.org/rustup`.

By switching between those two values, rustup effectively provides two "self
update channels", making beta testing possible with `rustup self update`.

Currently, rustup does one beta release followed by one official release for
each version number in the increasing order. In other words, we don't release
any `1.28.x` once the `1.29.0` beta release is out, and the latter is followed
by the `1.29.0` stable release, and so on.

## Bumping the version number

The version number is registered in the `Cargo.toml` file of the project.

The general principle for version numbers is that we always increment the
_minor_ number unless:

- A major incompatibility has been introduced in this release:
  increment the _major_ number instead.
- This release is a hotfix because the last one had a defect:
  increment the _patch_ number instead.

### Minor version bumps

> NOTE: Rustup hasn't been doing major version bumps since a long time ago, but
> if we ever do, the procedure for it should be similar to that of a minor one.

A minor version bump should be performed immediately after the latest `X.Y.0`
(e.g. `1.29.0`) beta release, and to do so, the following steps should be
taken:

- In the `main` branch, note the current minor version number `X.Y` (e.g.
  `1.29`) and create a new branch from `main` named `release/X.Y` (e.g.
  `release/1.29`). This will be the active backport branch from now on.
- In a separate PR targeting `main`:
  - Bump the minor version number in `Cargo.toml` (e.g. to `1.30.0`).
  - Run `cargo build` and review `Cargo.lock` changes.

### Patch version bumps

A patch version bump should be performed immediately after any latest release
other than `X.Y.0` betas if the backport branch `release/X.Y` is still
considered active (i.e. it is expected to cut new patch releases from the
branch):

- In a separate PR targeting that backport branch:
  - Bump the patch version number in `Cargo.toml` (e.g. to `1.29.1`).
  - Run `cargo build` and review `Cargo.lock` changes.

## Maintaining the backport branch

When the backport branch `release/X.Y` is active, you are expected to backport
to it any relevant non-breaking changes one would like to see in new patch
releases. This includes, but is not limited to:

- Bug fixes.
- Patch-compatible documentation improvements.
- Minor features if they are not expected to cause any breakage.
- CI adjustments if relevant to the active backport branch.

The backport PRs should bear the `backport` label and target the active
backport branch in a rebased, commit-preserving manner.

It is OK to backport multiple original PRs at once as long as the conflict
resolution is straightforward (we would expect this to be the case for the most
part otherwise it would be against the point of patch releases in the first
place).

The backport branches already have similar CI setup like that of `main`, but
the full CI must be manually triggered rather than scheduled. To do so, you can
use the [GitHub CLI] under the project directory:

```console
$ gh workflow run ci.yaml --ref release/X.Y
```

## Cutting a new release

Before making a release, ensure that `rustup-init.sh` is behaving correctly,
and that you're satisfied that nothing in the ecosystem is breaking because
of the update. A useful set of things to check includes verifying that
real-world toolchains install okay, and that `rust-analyzer` isn't broken by
the release. While it's not our responsibility if they depend on non-stable
APIs, we should behave well if we can.

The next step is to check whether you are cutting a beta or an official release,
and determine which `$BRANCH` you should be working on:

- `main` for `X.Y.0` beta releases.
- `release/X.Y` for any other `X.Y.*` release.

Producing the final release artifacts is a bit involved because of the way
rustup is distributed. Below is a list of things to be done in order to cut a
new [b]eta release or an official release [r]:

1. [b/r] Make sure that the desired version number for the new release
   `$VER_NUM` already exists in `$BRANCH`'s `Cargo.toml` file. Then in a new PR
   targeting `$BRANCH`:
   1. Update `CHANGELOG.md` accordingly if necessary.
   2. Update `rustup-init.sh` so that:
      - The version number matches `$VER_NUM`.
      - The commit shasum matches the latest commit on `$BRANCH`.
   3. Update the test snapshot of `rustup-init.sh --help`.
      At the moment of writing, this is done by running:
      ```console
      $ SNAPSHOTS=overwrite cargo test --features=test -- cli_rustup_init_ui
      ```
2. [b/r] After merging the PR made in the previous step:
   1. Pull the latest remote `$BRANCH` changes to the local `$BRANCH`.
   2. Hard-reset the local `stable` to `$BRANCH`'s tip.
   3. Double-check that the current local `stable` is indeed what is expected
      for the next release (version number, commit history, etc.).
   4. Force-push the local `stable` to the remote `stable`.
3. [b/r] While you wait for green CI on `stable`, double-check the
   functionality of `rustup-init.sh` and `rustup-init` just in case.
4. [b/r] Ensure all of CI is green on the `stable` branch.
   Once it is, check through a representative proportion of the builds looking
   for the reported version statements to ensure that we definitely built
   something cleanly which reports as the right version number when run
   `--version`.
5. [b] Make a new PR to the [Inside Rust Blog] adding a new "Call for Testing"
   announcement post.
6. [r] Make a new PR to the [Rust Blog] adding a new release announcement post.
7. [b/r] Ping someone in the release team to perform the actual release.
   They can find instructions in `ci/sync-dist.py`.
   > **Note:** Some manual testing occurs here, so hopefully they'll catch
   > anything egregious in which case abort the change and roll back.
8. [b] Once the beta release has happened, post a new topic named "Seeking beta
   testers for rustup $VER_NUM" on the [Internals Forum] to point to the blog
   post made previously.
9. [r] Once the official release has happened, prepare and push a tag on the
   latest `stable` commit.
   - `git tag -as $VER_NUM -m $VER_NUM` (optionally without `-s` if not GPG
     signing the tag)
   - `git push origin $VER_NUM`
10. [b/r] Immediately perform the corresponding version bump for the next
    release as described in the previous sections.

[Rust Blog]: https://github.com/rust-lang/blog.rust-lang.org
[Inside Rust Blog]: https://github.com/rust-lang/blog.rust-lang.org/tree/main/content/inside-rust
[Internals Forum]: https://internals.rust-lang.org
[GitHub CLI]: https://cli.github.com
