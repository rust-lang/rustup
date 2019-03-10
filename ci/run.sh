#!/bin/sh

set -ex

printf 'toolchain versions\n------------------\n'

rustc -vV
cargo -vV
rustfmt -vV

cargo build --locked -v --release --target "$TARGET" --features vendored-openssl

if [ -z "$SKIP_TESTS" ]; then
  cargo test --release -p download --target "$TARGET" --features vendored-openssl
  cargo test --release --target "$TARGET" --features vendored-openssl
fi

# Check the formatting last because test failures are more interesting to have
# discovered for contributors lacking some platform access for testing beforehand
cargo fmt --all -- --check

# Then check the shell scripts, if shellcheck is present (which it is on Travis CI)
if command -v shellcheck > /dev/null 2>&1; then
  shellcheck --version
  shellcheck -- *.sh ci/*.sh
else
  echo No shellcheck found, skipping.
fi
