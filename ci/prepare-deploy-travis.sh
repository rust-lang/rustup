#!/bin/bash

set -u -e

if [ "$TRAVIS_PULL_REQUEST" = "true" ] || [ "$TRAVIS_BRANCH" = "auto" ]; then
    exit 0
fi

# Upload docs
if [ "$TARGET" = "x86_64-unknown-linux-gnu" ] && [ "$TRAVIS_BRANCH" = "stable" ]; then
    # FIXME rust-lang/rust#32532
    printf "not uploading docs"
    #git config --global credential.helper store;
    #echo "https://${TOKEN}:x-oauth-basic@github.com" >> ~/.git-credentials;
    #cargo doc --release;
    #echo '<meta http-equiv=refresh content=0;url=rustup/index.html>' > target/doc/index.html;
    #sudo pip install ghp-import;
    #ghp-import -n target/doc;
    #git push -qf https://${TOKEN}@github.com/${TRAVIS_REPO_SLUG}.git gh-pages;
fi;

# Copy rustup-init to rustup-setup for backwards compatibility
cp target/"$TARGET"/release/rustup-init target/"$TARGET"/release/rustup-setup

# Generate hashes
if [ "$TRAVIS_OS_NAME" = "osx" ]; then
    find target/"$TARGET"/release/ -maxdepth 1 -type f -exec sh -c 'fn="$1"; shasum -a 256 -b "$fn" > "$fn".sha256' _ {} \;
else
    find target/"$TARGET"/release/ -maxdepth 1 -type f -exec sh -c 'fn="$1"; sha256sum -b "$fn" > "$fn".sha256' _ {} \;
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
