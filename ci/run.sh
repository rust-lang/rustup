#!/bin/sh

set -ex

cargo build --release --target $TARGET

if [ -z "$SKIP_TESTS" ]; then
  cargo test --release -p multirust-dist --target $TARGET
  cargo test --release --target $TARGET
fi
