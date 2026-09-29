#!/bin/bash

export RUSTUP_HOME=/tmp/tuf-home CARGO_HOME=/tmp/tuf-home
rm -rf /tmp/tuf-home && mkdir -p /tmp/tuf-home
target/debug/rustup-init -y --no-modify-path --default-toolchain none
/tmp/tuf-home/bin/rustup set auto-self-update disable

export RUSTUP_TUF_ENABLE=on 
export RUSTUP_TUF_IGNOREDATE=2026-09-29T19:31:22Z
export RUSTUP_TUF_SERVER=/home/jaynus/work/tuf/demo/test_v4
export RUSTUP_TUF_ROOT=/home/jaynus/work/tuf/demo/test_v4/metadata/1.root.json

export RUSTUP_LOG=rustup::tuf=trace,rustup::download=debug,rustup::dist=trace

# current stable
/tmp/tuf-home/bin/rustup toolchain install stable --profile minimal
/tmp/tuf-home/bin/rustup run stable rustc --version
