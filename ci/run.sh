#!/bin/sh

set -ex

RUST_BACKTRACE=1
export RUST_BACKTRACE

rustc -vV
cargo -vV

if [ "$TRAVIS_OS_NAME" = "windows" ]; then
  FEATURES=""
else
  FEATURES="--features vendored-openssl"
fi

# Sadly we need word splitting for $FEATURES
# shellcheck disable=SC2086
cargo build --locked -v --release --target "$TARGET" $FEATURES

if [ -z "$SKIP_TESTS" ]; then
  # shellcheck disable=SC2086
  cargo run --locked --release --target "$TARGET" $FEATURES -- --dump-testament
  # shellcheck disable=SC2086
  cargo test --release -p download --target "$TARGET" $FEATURES
  # shellcheck disable=SC2086
  cargo test --release --target "$TARGET" $FEATURES
fi
