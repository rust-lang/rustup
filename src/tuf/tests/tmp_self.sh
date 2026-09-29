#!/bin/bash

export RUSTUP_HOME=/tmp/tuf-home CARGO_HOME=/tmp/tuf-home
rm -rf /tmp/tuf-home && mkdir -p /tmp/tuf-home
target/debug/rustup-init -y --no-modify-path --default-toolchain none

export RUSTUP_TUF_ENABLE=on
export RUSTUP_TUF_IGNOREDATE=2026-09-29T19:31:22Z
export RUSTUP_TUF_SERVER=/home/jaynus/work/tuf/demo/test_v4
export RUSTUP_TUF_ROOT=/home/jaynus/work/tuf/demo/test_v4/metadata/1.root.json

# self-update files must resolve to targets under the dist root
export RUSTUP_UPDATE_ROOT=https://static.rust-lang.org/dist/rustup

export RUSTUP_LOG=rustup::tuf=debug,rustup::download=debug

/tmp/tuf-home/bin/rustup self update
/tmp/tuf-home/bin/rustup --version