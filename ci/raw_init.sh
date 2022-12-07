#!/bin/sh

set -ex

sh ./rustup-init.sh --default-toolchain none -y
# shellcheck source=/dev/null
. "$HOME"/.cargo/env
rustup -Vv
