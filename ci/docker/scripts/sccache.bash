#!/bin/bash
set -xe

VERSION=0.2.12
TARGET=x86_64-unknown-linux-musl
# from https://github.com/mozilla/sccache/releases
SHA="26fd04c1273952cc2a0f359a71c8a1857137f0ee3634058b3f4a63b69fc8eb7f"
DL_URL="https://github.com/mozilla/sccache/releases/download"
BIN_DIR=/usr/local/bin
TEMP_DIR=$(mktemp -d)
TAR_NAME="sccache-${VERSION}-${TARGET}.tar.gz"

cd "${TEMP_DIR}"
mkdir -p "${BIN_DIR}"

curl -sSL -O "${DL_URL}/${VERSION}/${TAR_NAME}"
echo "${SHA}  ${TAR_NAME}" | sha256sum --check -
tar -xzf "${TAR_NAME}" --strip-components 1
cp sccache "${BIN_DIR}/sccache"
chmod +x "${BIN_DIR}/sccache"
