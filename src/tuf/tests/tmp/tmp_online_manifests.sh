#!/bin/bash

curl -o /tmp/tuf-root.json https://storage.googleapis.com/tufops/metadata/1.root.json

export RUSTUP_TUF_ENABLE=on
export RUSTUP_TUF_IGNOREDATE=2026-09-26T17:06:53Z
export RUSTUP_TUF_SERVER=https://storage.googleapis.com/tufops
export RUSTUP_TUF_ROOT=/tmp/tuf-root.json
export RUSTUP_LOG=rustup::tuf=trace,rustup::download=debug,rustup::dist=trace
export RUSTUP_HOME=/tmp/tuf-home CARGO_HOME=/tmp/tuf-home

rm -rf /tmp/tuf-home && mkdir -p /tmp/tuf-home
target/debug/rustup-init -y --no-modify-path --default-toolchain none
/tmp/tuf-home/bin/rustup set auto-self-update disable

# current stable
/tmp/tuf-home/bin/rustup toolchain install stable --profile minimal
/tmp/tuf-home/bin/rustup run stable rustc --version
