#!/bin/bash

set -u -e

# Copy rustup-init to rustup-setup for backwards compatibility
cp target/"$TARGET"/release/rustup-init target/"$TARGET"/release/rustup-setup

# Generate hashes
if [ "$(uname -s)" = "Darwin" ]; then
    find target/"$TARGET"/release/ -maxdepth 1 -type f -exec sh -c 'fn="$1"; shasum -a 256 -b "$fn" > "$fn".sha256' sh {} \;
else
    find target/"$TARGET"/release/ -maxdepth 1 -type f -exec sh -c 'fn="$1"; sha256sum -b "$fn" > "$fn".sha256' sh {} \;
fi

# The directory for deployment artifacts
dest="deploy"

# Prepare bins for upload
bindest="$dest/dist/$TARGET"
mkdir -p "$bindest/"
cp target/"$TARGET"/release/rustup-init "$bindest/"
cp target/"$TARGET"/release/rustup-init.sha256 "$bindest/"
cp target/"$TARGET"/release/rustup-setup "$bindest/"
cp target/"$TARGET"/release/rustup-setup.sha256 "$bindest/"

if [ "$TARGET" != "x86_64-unknown-linux-gnu" ]; then
    exit 0
fi

cp rustup-init.sh "$dest/"

# Prepare website for upload
cp -R www "$dest/www"
