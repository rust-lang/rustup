# Rustup GitHub Actions Workflows

This directory contains all the workflows we use in Rustup for GitHub Actions.

## Triggers for CI builds

Rustup has five situations in which we perform CI builds:

1. On PR changes
2. On merge to master
3. Time-based rebuilds of master
4. Pushes to the stable branch
5. Renovate branches with dependency updates, tested before opening a PR or
   merging. They are assessed the same as a PR: if it would be good enough as a
   human proposed change, it's good enough as a robot proposed change.

The goals for each of those situations are subtly different. For PR changes,
we want to know as quickly as possible if the change is likely to be an issue.

Once a change hits master, we want to know that all our targets build.

Time based rebuilds of master are about determining if updates to the toolchain
have caused us problems, and also to try and highlight if we have flaky tests.

The stable branch is about making releases. Builds from that branch are uploaded
to S3 so that we can then make a release of rustup.

## Targets we need to build

We follow `rustc`'s [platform support policy] closely, and so `rustup` is expected
to build for all targets listed in the _tier 1_ or the _tier 2 with host tools_ section.

[platform support policy]: https://doc.rust-lang.org/nightly/rustc/platform-support.html

In order to reduce the maintainance burden, targets listed in the _tier 2 without host
tools_ section might get limited support, but should by no means become a blocker.
We should not build for targets listed in the _tier 3_ section, and if a target gets
downgraded to tier 3, its CI workflows should be dropped accordingly.

If a platform is directly supported by GitHub Action's free runners, we should always
build for it natively with the full test suite activated.
Otherwise, we might consider performing a cross-build, in which case we won't run the
tests for the the target.

## Useful notes about how we run builds

For the builds which run on x86_64 linux, we deliberately run inside a docker
image which comes from `rust-lang/rust`'s CI so that we know we're linking against
the same libc etc as the release of Rust.

For the builds which run on Windows, we retrieve mingw from Rust's CI as well
so that we're clearly using the right version of that.

In all cases, we attempt to use the `rustup-init.sh` from the branch under test
where at all possible, so that we spot errors in that ASAP.

Given that, we prefer to not use a preinstalled rust/rustup at all if we can,
so we start from as bare a VM as makes sense.

For Windows builds, we use a Visual Studio 2017 image if we can.

## The workflows

Due to limitations in how github workflows work, we have to create our workflows
from template files and then commit them.

The templates are in this directory, and the built workflows end up in the
`.github/workflows` directory. `-all` always runs, `-on-pr` `-on-master` and
`-on-stable` do the obvious.
