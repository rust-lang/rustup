#!/bin/bash
set -xe

VERSION=0.2.12
TARGET=x86_64-unknown-linux-musl
BIN_DIR=/usr/local/bin
TEMP_DIR=$(mktemp -d)

cd "${TEMP_DIR}"
mkdir -p "${BIN_DIR}"

curl -sSL "https://github.com/mozilla/sccache/releases/download/${VERSION}/sccache-${VERSION}-${TARGET}.tar.gz" | \
tar -xzf - --strip-components 1
cp sccache "${BIN_DIR}/sccache"
chmod +x "${BIN_DIR}/sccache"
