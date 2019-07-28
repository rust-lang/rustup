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

runtest () {
  # shellcheck disable=SC2086
  cargo test --release --target "$TARGET" $FEATURES "$@"
}

if [ -z "$SKIP_TESTS" ]; then
  # shellcheck disable=SC2086
  cargo run --locked --release --target "$TARGET" $FEATURES -- --dump-testament
  runtest -p download
  runtest --bin rustup-init
  runtest --lib --all
  runtest --doc --all
  for TEST in $(cd tests; ls *.rs | cut -d. -f1); do
    if [ "x$TEST" = "xdist" ]; then
      runtest --test "$TEST" -- --test-threads 1
    else
      runtest --test "$TEST"
    fi
  done
fi
