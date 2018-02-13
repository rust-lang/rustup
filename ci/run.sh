#!/bin/sh

set -ex

echo "toolchain versions\n------------------"

rustc -vV
cargo -vV

cargo build --locked -v --release -p rustup-init --target $TARGET

if [ -z "$SKIP_TESTS" ]; then
  cargo test --release --all --exclude rustup-win-installer --target $TARGET
fi
