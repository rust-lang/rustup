#!/bin/bash

set -ex

# First, we check that this script is not run as root because it would fail tests.
if [ "root" == "$(whoami)" ]; then exit 1; fi

echo "========="
echo "Display the current git status"

git status
git tag --list
git describe

echo "========="
echo "Prep cargo dirs"

mkdir -p ~/.cargo/{registry,git}

echo "========="
echo "Install Rustup using ./rustup-init.sh"

sh rustup-init.sh --default-toolchain=stable --profile=minimal -y
# It's the equivalent of `source`
# shellcheck source=src/cli/self_update/env.sh
source "$HOME"/.cargo/env

echo "========="
echo "Ensure we have the components we need"

rustup component add rustfmt
rustup component add clippy

echo "========="
echo "Run the freebsd check"

unset SKIP_TESTS
export LIBZ_SYS_STATIC=1
export CARGO_BUILD_JOBS=1
export TARGET="x86_64-unknown-freebsd"
# TODO: This should be split into two as the other jobs are.
export BUILD_PROFILE="release"

# HACK: Works around `aws-lc-rs`' issue with internal bindgen on FreeBSD.
# See: https://github.com/aws/aws-lc-rs/issues/476#issuecomment-2263118015
export AWS_LC_SYS_EXTERNAL_BINDGEN=1

bash ci/run.bash
