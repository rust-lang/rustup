#!/bin/bash

script_dir=$(cd "$(dirname "$0")" && pwd)
. "$script_dir/shared.bash"

set -ex

export RUST_BACKTRACE=1

rustc -vV
cargo -vV


declare -a FEATURES
set_features_by_target "$TARGET"
try_export_CFLAGS_by_target "$TARGET"

cargo build --locked --release --target "$TARGET" "${FEATURES[@]}"

runtest () {
  cargo test --locked --release --target "$TARGET" "${FEATURES[@]}" "$@"
}

run_download_pkg_test() {
  features=('--no-default-features' '--features' 'curl-backend,reqwest-backend,reqwest-default-tls')
  case "$TARGET" in
    # these platforms aren't supported by ring:
    powerpc* ) ;;
    mips* ) ;;
    riscv* ) ;;
    s390x* ) ;;
    aarch64-pc-windows-msvc ) ;;
    # default case, build with rustls enabled
    * ) features+=('--features' 'reqwest-rustls-tls') ;;
  esac

  cargo test --locked --release --target "$TARGET" "${features[@]}" -p download
}

if [ -z "$SKIP_TESTS" ]; then
  cargo run --locked --release --target "$TARGET" "${FEATURES[@]}" -- --dump-testament
  run_download_pkg_test 
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
