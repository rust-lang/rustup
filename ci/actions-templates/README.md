# Rustup GitHub Actions Workflows

This directory contains all the workflows we use in Rustup for GitHub Actions.

## Triggers for CI builds

Rustup has four situations in which we perform CI builds:

1. On PR changes
2. On merge to master
3. Time-based rebuilds of master
4. Pushes to the stable branch

The goals for each of those situations are subtly different. For PR changes,
we want to know as quickly as possible if the change is likely to be an issue.

Once a change hits master, we want to know that all our targets build.

Time based rebuilds of master are about determining if updates to the toolchain
have caused us problems, and also to try and hilight if we have flaky tests.

The stable branch is about making releases. Builds from that branch are uploaded
to S3 so that we can then make a release of rustup.

## Targets we need to build

We build for all the Tier one targets and a non-trivial number of the tier two
targets of Rust. We do not even attempt tier three builds.

We don't run the tests on all the targets because many are cross-built. If we
cross-build we don't run the tests. All the builds which aren't mac or windows
are built on an x86_64 system because that's the easiest way to get a performant
system.

| Target                        | Cross      | Tier  | On PR? | On master? |
| ----------------------------- | ---------- | ----- | ------ | ---------- |
| x86_64-unknown-linux-gnu      | No         | One   | Yes    | Yes        |
| armv7-unknown-linux-gnueabihf | Yes        | Two   | Yes    | Yes        |
| aarch64-linux-android         | Yes        | Two   | Yes    | Yes        |
| aarch64-unknown-linux-gnu     | Yes        | Two   | No     | Yes        |
| aarch64-unknown-linux-musl    | Yes        | Two   | No     | Yes        |
| powerpc64-unknown-linux-gnu   | Yes        | Two   | No     | Yes        |
| x86_64-unknown-linux-musl     | Yes        | Two   | No     | Yes        |
| i686-unknown-linux-gnu        | Yes        | One   | No     | No         |
| arm-unknown-linux-gnueabi     | Yes        | Two   | No     | No         |
| arm-unknown-linux-gnueabihf   | Yes        | Two   | No     | No         |
| x86_64-unknown-freebsd        | Yes        | Two   | No     | No         |
| x86_64-unknown-netbsd         | Yes        | Two   | No     | No         |
| powerpc-unknown-linux-gnu     | Yes        | Two   | No     | No         |
| powerpc64le-unknown-linux-gnu | Yes        | Two   | No     | No         |
| mips-unknown-linux-gnu        | Yes        | Two   | No     | No         |
| mips64-unknown-linux-gnu      | Yes        | Two   | No     | No         |
| mipsel-unknown-linux-gnu      | Yes        | Two   | No     | No         |
| mips64el-unknown-linux-gnu    | Yes        | Two   | No     | No         |
| s390x-unknown-linux-gnu       | Yes        | Two   | No     | No         |
| arm-linux-androideabi         | Yes        | Two   | No     | No         |
| armv7-linux-androideabi       | Yes        | Two   | No     | No         |
| i686-linux-android            | Yes        | Two   | No     | No         |
| x86_64-linux-android          | Yes        | Two   | No     | No         |
| riscv64gc-unknown-linux-gnu   | Yes        | ---   | No     | No         |
| ----------------------------- | ---------- | ----- | ------ | ---------- |
| aarch64-apple-darwin          | Yes        | Two   | Yes    | Yes        |
| x86_64-apple-darwin           | No         | One   | Yes    | Yes        |
| ----------------------------- | ---------- | ----- | ------ | ---------- |
| x86_64-pc-windows-msvc        | No         | One   | Yes    | Yes        |
| x86_64-pc-windows-gnu         | No         | One   | No     | Yes        |
| i686-pc-windows-msvc          | No         | One   | No     | No         |
| i686-pc-windows-gnu           | No         | One   | No     | No         |

We also have a clippy/shellcheck target which runs on x86_64 linux and is
run in all cases. It does a `cargo fmt` check, a `cargo clippy` check on the
beta toolchain, and also runs `rustup-init.sh` through to completion inside
a centos 6 docker to ensure that we continue to work on there OK.

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
