#!/bin/bash

set -ex

export RUST_BACKTRACE=1

rustc -vV
cargo -vV

if [ -n "$INSTALL_BINDGEN" ]; then
  # Install `cargo-binstall` first for faster installation.
  curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
  # An explicit `--target` is required to ensure that `bindgen-cli` is built for the
  # same target as the rest of the toolchain.
  # See: <https://github.com/rust-lang/rustup/issues/4396>
  cargo binstall -y --force --locked bindgen-cli "--target=$(rustc --print host-tuple)"
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
  *solaris* ) ;;
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
