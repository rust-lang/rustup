#!/bin/bash

set -u -e

if [ "$TRAVIS_PULL_REQUEST" == "true" ]; then
    exit 0
fi

if [ "$TRAVIS_BRANCH" == "auto" ]; then
    exit 0
fi

# Upload docs
if [[ "$TARGET" == "x86_64-unknown-linux-gnu" && "$TRAVIS_BRANCH" == "stable" ]]; then
    # FIXME rust-lang/rust#32532
    printf "not uploading docs"
    #git config --global credential.helper store;
    #echo "https://${TOKEN}:x-oauth-basic@github.com" >> ~/.git-credentials;
    #cargo doc --release;
    #echo '<meta http-equiv=refresh content=0;url=multirust/index.html>' > target/doc/index.html;
    #sudo pip install ghp-import;
    #ghp-import -n target/doc;
    #git push -qf https://${TOKEN}@github.com/${TRAVIS_REPO_SLUG}.git gh-pages;
fi;

# Generate hashes
if [ "$TRAVIS_OS_NAME" == "osx" ]; then
    find "target/$TARGET/release/" -maxdepth 1 -type f -exec sh -c 'shasum -a 256 -b "{}" > "{}.sha256"' \;;
else
    find "target/$TARGET/release/" -maxdepth 1 -type f -exec sh -c 'sha256sum -b "{}" > "{}.sha256"' \;;
fi

# The directory for deployment artifacts
dest="deploy"

# Prepare bins for upload
bindest="$dest/dist/$TARGET"
mkdir -p "$bindest/"
cp target/$TARGET/release/rustup-setup "$bindest/"
cp target/$TARGET/release/rustup-setup.sha256 "$bindest/"

if [ "$TARGET" != "x86_64-unknown-linux-gnu" ]; then
    exit 0
fi

cp rustup-setup.sh "$dest/"

# Prepare website for upload
cp -R www "$dest/www"
