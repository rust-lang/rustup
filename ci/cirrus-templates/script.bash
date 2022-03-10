#!/bin/bash                                                                                            

set -ex

# First, we check that this script is not run as root because it would fail tests.
if [ "root" == "$(whoami)" ]; then exit 1; fi

echo "========="
echo "Acquire tags for the repo"

git fetch --no-tags --prune --depth=1 origin +refs/tags/*:refs/tags/*

echo "========="
echo "Display the current git status"

git status
git describe

echo "========="
echo "Prep cargo dirs"

mkdir -p ~/.cargo/{registry,git}

echo "========="
echo "Install Rustup using ./rustup-init.sh"

sh rustup-init.sh --default-toolchain=stable --profile=minimal -y
# It's the equivalent of `source`
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
bash ci/run.bash
