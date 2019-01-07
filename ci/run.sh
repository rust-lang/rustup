#!/bin/sh

set -ex

echo "toolchain versions\n------------------"

rustc -vV
cargo -vV
rustfmt -vV

cargo build --locked -v --release --target $TARGET --features vendored-openssl

if [ -z "$SKIP_TESTS" ]; then
  cargo test --release -p download --target $TARGET --features vendored-openssl
  cargo test --release -p rustup-dist --target $TARGET --features vendored-openssl
  cargo test --release --target $TARGET --features vendored-openssl
fi

# Check the formatting last because test failures are more interesting to have
# discovered for contributors lacking some platform access for testing beforehand
cargo fmt --all -- --check
