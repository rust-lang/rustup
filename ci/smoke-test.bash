#!/bin/bash

script_dir=$(cd "$(dirname "$0")" && pwd)
. "$script_dir/shared.bash"

set -ex

if [ -n "$SKIP_TESTS" ]; then
  exit 0
fi

export RUST_BACKTRACE=1
declare -a FEATURES
set_features_by_target "$TARGET"
try_export_CFLAGS_by_target "$TARGET"

cargo build --locked --release --target "$TARGET" "${FEATURES[@]}"

# Install and test with a freshly compiled rustup.
mkdir -p home
export RUSTUP_HOME=home 
export CARGO_HOME=home

case "$(uname -s)" in
  *NT* ) # Windows NT
    target/"$TARGET"/release/rustup-init.exe --no-modify-path  --default-toolchain=none --profile default -y
    setx PATH "%PATH%;%USERPROFILE%\.cargo\bin"
    ;; 
  * )
    target/"$TARGET"/release/rustup-init --no-modify-path  --default-toolchain=none --profile default -y
    # shellcheck source=/dev/null
    source home/env
    ;;
esac

# Testing the proxy. 
# 2022-02-23 the proxy components to be tested are present.
rustup toolchain install --profile=default nightly-2022-02-23

rustc --version
rustdoc --version
cargo --version
rustfmt --version
cargo-fmt --version
cargo-clippy --version
clippy-driver --version
# Temporarily not working, for more information see: 
# https://github.com/rust-lang/rustup/issues/2838
# rust-lldb --version
# rust-gdb --version

# Temporarily not working, for more information see: 
# https://github.com/rust-lang/rust/issues/61282
# rust-gdbgui -h

rustup component add rls miri
rls --version
cargo miri --version
