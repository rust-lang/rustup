#!/bin/sh

set -ex

# only enable core dump on Linux
if [ -f /proc/sys/kernel/core_pattern ]; then
  # shellcheck disable=SC2169
  # `-c` exists in Ubuntu 14.04 and later at least
  ulimit -c unlimited
fi

rustc -vV
cargo -vV
rustfmt -vV

cargo build --locked -v --release --target "$TARGET" --features vendored-openssl

if [ -z "$SKIP_TESTS" ]; then
  cargo test --release -p download --target "$TARGET" --features vendored-openssl
  cargo test --release --target "$TARGET" --features vendored-openssl
fi
