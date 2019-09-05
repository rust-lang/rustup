#!/bin/bash

set -ex

export RUST_BACKTRACE=1

rustc -vV
cargo -vV

FEATURES=()
if [ "$TRAVIS_OS_NAME" != "windows" ]; then
  FEATURES=('--features' 'vendored-openssl')
fi

# rustc only supports armv7: https://forge.rust-lang.org/platform-support.html
if [ "$TARGET" = arm-linux-androideabi ]; then
  export CFLAGS='-march=armv7'
fi

cargo build --locked -v --release --target "$TARGET" "${FEATURES[@]}"

runtest () {
  cargo test --release --target "$TARGET" "${FEATURES[@]}" "$@"
}

if [ -z "$SKIP_TESTS" ]; then
  cargo run --locked --release --target "$TARGET" "${FEATURES[@]}" -- --dump-testament
  runtest -p download
  runtest --bin rustup-init
  runtest --lib --all
  runtest --doc --all

  runtest --test dist -- --test-threads 1

  find tests -maxdepth 1 -type f ! -path '*/dist.rs' -name '*.rs' \
  | sed -e 's@^tests/@@;s@\.rs$@@g' \
  | while read -r test; do
    runtest --test "${test}"
  done
fi
