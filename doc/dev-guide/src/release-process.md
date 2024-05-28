# Release Process

This document describes the process for making a new release of Rustup. It is
split into the two sections. The first section explains the release process,
while the second section documents for maintainers how to make a release.

- [About the Release Process](#about-the-release-process)
    - [Historic Context](#historic-context)
    - [Beta and Stable Releases](#beta-and-stable-releases)
    - [Release Artifacts](#release-artifacts)
    - [Automation](#automation)
- [How to Cut a Release](#how-to-cut-a-release)
    - [1. Test `rustup-init.sh`](#1-test-rustup-initsh)
    - [2. Update Version](#2-update-version)
    - [3. Update Checksum](#3-update-checksum)
    - [4. Sync `master` to `stable`](#4-sync-master-to-stable)
    - [5. Test the `beta` Release](#5-test-the-beta-release)
    - [6. Prepare Blog Post for `stable` Release](#6-prepare-blog-post-for-stable-release)
    - [7. Request the `stable` Release](#7-request-the-stable-release)
    - [8. Tag the `stable` Release](#8-tag-the-stable-release)

## About the Release Process

This section explains the release process for Rustup.

### Historic Context

The Rustup release process has evolved over time. In the past, the process
involved more manual steps and large parts of it were executed locally. This
required careful execution and coordination to avoid accidentally overwriting
exiting releases, since the tooling did not implement sanity checks that would
prevent destructive actions.

A full step-by-step guide for the old process can be found in
the [old developer guide](https://github.com/rust-lang/rustup/blob/1cf1b5a6d80c978e0dcaabbce5f10b3861612425/doc/dev-guide/src/release-process.md).
The following summarizes how release artifacts were created and copied around.

In the past, both beta and stable releases were started by merging a new commit
into the `stable` branch in [rust-lang/rustup]. This started a new build on
GitHub Actions that produced release
artifacts ([source](https://github.com/rust-lang/rustup/blob/1cf1b5a6d80c978e0dcaabbce5f10b3861612425/.github/workflows/ci.yaml#L144-L151)),
which were uploaded to `s3://dev-static-rust-lang-org/rustup/dist`. As new
commits were merged into `stable`, they would overwrite the artifacts from prior
builds.

The release artifacts were then copied to the final location by running a script
on a local machine. This script would download the artifacts from the
`dev-static` bucket on S3 and upload them to the `static` bucket. The script
also generated a new manifest with the given version, and uploaded a copy of the
files to an archive directory.

Given that the process was manual, involved copying files to the local machine,
and waiting between creating a `beta` and a `stable` release, there was a risk
of human error. When we had to update the script after four years without a
change, we [decided](https://github.com/rust-lang/rustup/pull/3819) to redesign
and automate the release process.

### Beta and Stable Releases

Rustup can be released to two different environments: `beta` and `stable`. The
main difference between the two is that they use different values for the
`RUSTUP_UPDATE_ROOT` environment variable:

- A beta release is deployed on `https://dev-static.rust-lang.org/rustup`.
- An official release is deployed on `https://static.rust-lang.org/rustup`.

By switching between those two values, Rustup effectively provides two
"self update channels", making beta testing possible with `rustup self update`.

### Release Artifacts

The release artifacts are produced by CI when a new commit is merged into the
`stable` branch, and they are uploaded to the `dev-static` bucket on S3. There,
they are put into a folder named after their commit hash, for example the
artifacts for commit `1cf1b5a` would be uploaded to
`s3://dev-static-rust-lang-org/rustup/builds/1cf1b5a`.

When a new `beta` release is cut, the artifacts are copied to two new locations
within the same bucket:

- One copy is put into an archive named after the version, e.g.
  `/rustup/archive/1.0.0`.
- Another copy is put into the `/rustup/dist` folder, which is where clients
  look for new versions.

When a new `stable` release is cut, the artifacts are copied to the `static`
bucket on S3, following the same process as the `beta` release:

- One copy is archived into `/rustup/archive/1.0.0`.
- Another copy is put into `/rustup/dist`.

This ensures backwards compatibility with the old release process, while also
ensuring that release artifacts are not overwritten by new builds.

### Automation

The interaction with the release artifacts is fully automated.

First, artifacts are produced automatically by
the [`CI`](https://github.com/rust-lang/rustup/blob/master/.github/workflows/ci.yaml)
job on GitHub Actions when a new commit is merged into the `stable` branch, and
are then automatically uploaded to their respective S3 bucket by the action as
well.

Second, when making a release, the artifacts are copied to their final locations
by the [promote-release] tool. This reduces the risk of human error and ensures
that the release process is consistent and reliable. The tool is also run in a
secure environment on AWS CodeBuild, reducing the risk of leaking sensitive
credentials that would give write access (past) releases.

For a `beta` release, `promote-release` performs the following actions:

1. Query GitHub's API to get the latest commit on the `stable` branch
2. Confirm that `/rustup/builds/${commit}` exists in the `dev-static` bucket
3. Get the new version number from the `stable` branch
    1. Download `Cargo.toml` from `stable`
    2. Parse the file and read the `version` field
4. Confirm that `/rustup/archive/${version}` does not exist yet
5. Copy `/rustup/builds/${commit}` to `/rustup/archive/${version}`
6. Copy `/rustup/builds/${commit}` to `/rustup/dist`
7. Generate a new manifest and upload it to `/rustup/dist`

For a new `stable` release, the process is the same. The only difference is that
the steps 4-6 copy the artifacts to the `static` bucket.

## How to Cut a Release

This section documents the steps that a maintainer should follow to cut a new
release of Rustup.

### 1. Test `rustup-init.sh`

Before cutting a release, ensure that `rustup-init.sh` is behaving correctly,
and that you're satisfied that nothing in the ecosystem is breaking because
of the update. A useful set of things to check includes verifying that
real-world toolchains install okay, and that `rust-analyzer` isn't broken by
the release. While it's not our responsibility if they depend on non-stable
APIs, we should behave well if we can.

### 2. Update Version

The release process gets metadata from the `Cargo.toml` file, so ensure that
the version number in `Cargo.toml` is correct.

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

Submit the changes in a PR and merge it.

### 3. Update Checksum

After merging the PR in the previous step, update the commit SHA checksum in
`rustup-init.sh` to match the latest commit on `master`. Submit the change in a
PR and merge it.

### 4. Sync `master` to `stable`

After merging the PR in the previous step, sync `master` to `stable` using
`--ff-only`:

```shell
git fetch origin master:master
git checkout stable && git merge --ff-only master
git push origin HEAD:stable
```

This will kick off a new build on GitHub Actions, which will produce the release
artifacts, upload them to `dev-static`, and make the new `beta` release
available at `RUSTUP_UPDATE_ROOT=https://dev-static.rust-lang.org/rustup`.

### 5. Test the `beta` Release

While you wait for green CI on `stable`, double-check the functionality of
`rustup-init.sh` and `rustup-init` just in case.

Ensure all of CI is green on the `stable` branch. Once it is, check through a
representative proportion of the builds looking for the reported version
statements to ensure that we definitely built something cleanly which reports as
the right version number when run `--version`.

Once the beta release has happened, post a new topic named "Seeking beta testers
for Rustup $VER_NUM" on the [Internals Forum].

### 6. Prepare Blog Post for `stable` Release

Make a new PR to the [Rust Blog] adding a new release announcement post.

### 7. Request the `stable` Release

Ping someone in the release team to perform the `stable` release.

They will have to start a new CodeBuild job on AWS to run the `promote-release`
tool for Rustup.

### 8. Tag the `stable` Release

Once the official release has happened, prepare and push a tag on the latest
`stable` commit.

- `git tag -as $VER_NUM -m $VER_NUM` (optionally without `-s` if not GPG
  signing the tag)
- `git push origin $VER_NUM`

[internals forum]: https://internals.rust-lang.org
[promote-release]: https://github.com/rust-lang/promote-release
[rust-lang/rustup]:https://github.com/rust-lang/rustup
[rust blog]: https://github.com/rust-lang/blog.rust-lang.org

