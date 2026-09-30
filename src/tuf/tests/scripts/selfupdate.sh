#!/bin/bash

export RUSTUP_TUF_ENABLE=on
export RUSTUP_TUF_IGNOREDATE=2026-09-26T17:06:53Z
export RUSTUP_HOME=/tmp/tuf-home CARGO_HOME=/tmp/tuf-home
export RUSTUP_LOG=rustup::tuf=debug,rustup::download=debug

rm -rf /tmp/tuf-home && mkdir -p /tmp/tuf-home
target/debug/rustup-init -y --no-modify-path --default-toolchain none

/tmp/tuf-home/bin/rustup self update
/tmp/tuf-home/bin/rustup --version