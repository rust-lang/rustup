#!/bin/sh

set -ex

echo "toolchain versions\n------------------"

rustc -vV
cargo -vV

cargo build --locked -v --release --target $TARGET --features vendored-openssl

if [ -z "$SKIP_TESTS" ]; then
  cargo test --release -p download --target $TARGET --features vendored-openssl
  cargo test --release -p rustup-dist --target $TARGET --features vendored-openssl
  cargo test --release --target $TARGET --features vendored-openssl
fi
