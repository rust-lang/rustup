#!/bin/bash

set -ex

export RUST_BACKTRACE=1

rustc -vV
cargo -vV

if [ -n "$INSTALL_BINDGEN" ]; then
  if ! curl --proto '=https' --tlsv1.2 -LsSf https://github.com/rust-lang/rust-bindgen/releases/latest/download/bindgen-cli-installer.sh | sh -s -- --no-modify-path \
    | grep "everything's installed!";
    # Ignoring exit code since the script might fail to write the receipt after a successful installation.
  then
    cargo install --force --locked bindgen-cli
  fi
  mkdir "$CARGO_HOME"/bin/bindgen-cli
  mv "$CARGO_HOME"/bin/bindgen "$CARGO_HOME"/bin/bindgen-cli/
  export PATH="$CARGO_HOME/bin/bindgen-cli:$PATH"
fi


FEATURES=('--no-default-features' '--features' 'curl-backend,reqwest-native-tls')
case "$(uname -s)" in
  *NT* ) ;; # Windows NT
  * ) FEATURES+=('--features' 'vendored-openssl') ;;
esac

case "$TARGET" in
  # these platforms aren't supported by aws-lc-rs:
  powerpc64* ) ;;
  mips* ) ;;
  loongarch* ) ;;
  *netbsd* ) ;;
  *illumos* ) ;;
  # default case, build with rustls enabled
  * ) FEATURES+=('--features' 'reqwest-rustls-tls') ;;
esac

# rustc only supports armv7: https://doc.rust-lang.org/nightly/rustc/platform-support.html
if [ "$TARGET" = arm-linux-androideabi ]; then
  export CFLAGS='-march=armv7'
fi

target_cargo() {
    cmd="$1"
    shift
    cargo "${cmd}" --locked --profile "$BUILD_PROFILE" --target "$TARGET" "${FEATURES[@]}" "$@"
}

target_cargo build

# Machines have 7GB of RAM, and our target/ contents is large enough that
# thrashing will occur if we build-run-build-run rather than
# build-build-build-run-run-run. Since this is used solely for non-release
# artifacts, we try to keep features consistent across the builds, whether for
# docs/test/runs etc.
build_test() {
  cmd="$1"
  shift

  features=('--features' 'curl-backend,reqwest-native-tls')
  case "$TARGET" in
    # these platforms aren't supported by aws-lc-rs:
    powerpc* ) ;;
    mips* ) ;;
    riscv* ) ;;
    s390x* ) ;;
    # default case, build with rustls enabled
    * ) features+=('--features' 'reqwest-rustls-tls') ;;
  esac

  if [ "build" = "${cmd}" ]; then
    target_cargo "${cmd}" --workspace --all-targets "${features[@]}" --features test
  else
    target_cargo "${cmd}" --workspace "${features[@]}" --features test --tests
    target_cargo "${cmd}" --doc --workspace "${features[@]}" --features test
  fi
}

if [ -z "$SKIP_TESTS" ]; then
  target_cargo run --features test -- --dump-testament
  build_test build
  RUSTUP_CI=1 build_test test
fi
