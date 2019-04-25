#!/bin/sh

set -ex

sh ./rustup-init.sh --default-toolchain none -y
. "$HOME"/.cargo/env
rustup -Vv
