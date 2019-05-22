#!/bin/sh
set -xe

VERSION=0.2.8
TARGET=x86_64-unknown-linux-musl
BIN_DIR=/usr/local/bin
TEMP_DIR=$(mktemp -d)

mkdir -p "${BIN_DIR}"

curl \
  -o "${TEMP_DIR}/sccache.tar.gz" \
  -sSL \
  "https://github.com/mozilla/sccache/releases/download/${VERSION}/sccache-${VERSION}-${TARGET}.tar.gz"

tar \
  -xzf "${TEMP_DIR}/sccache.tar.gz" \
  -C "${BIN_DIR}" \
  --strip-components 1 \
  "sccache-${VERSION}-${TARGET}/sccache"

chmod +x "${BIN_DIR}/sccache"
