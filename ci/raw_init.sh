#!/bin/sh

set -ex

sh ./rustup-init.sh --default-toolchain none -y
# shellcheck source=src/cli/self_update/env.sh
. "$HOME"/.cargo/env
rustup -Vv
