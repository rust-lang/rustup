#!/bin/sh

set -ex

echo "toolchain versions\n------------------"

rustc -vV
cargo -vV

cargo build --release --target $TARGET

if [ -z "$SKIP_TESTS" ]; then
  cargo test --release -p rustup-dist --target $TARGET
  cargo test --release --target $TARGET
fi
