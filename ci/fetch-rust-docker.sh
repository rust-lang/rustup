#!/bin/bash

set -ex

TARGET="$1"

RUST_REPO="https://github.com/rust-lang/rust"
S3_BASE_URL="https://s3-us-west-1.amazonaws.com/rust-lang-ci2/rustc-builds"

# See http://unix.stackexchange.com/questions/82598
# Duplicated from rust-lang/rust/src/ci/shared.sh
function retry {
  echo "Attempting with retry:" "$@"
  local n=1
  local max=5
  while true; do
    "$@" && break || {
      if [[ $n -lt $max ]]; then
        sleep $n  # don't retry immediately
        ((n++))
        echo "Command failed. Attempt $n/$max:"
      else
        echo "The command has failed after $n attempts."
        return 1
      fi
    }
  done
}

# Use images from rustc master
case "$TARGET" in
  mips-unknown-linux-gnu)          image=dist-mips-linux ;;
  mips64-unknown-linux-gnuabi64)   image=dist-mips64-linux ;;
  mips64el-unknown-linux-gnuabi64) image=dist-mips64el-linux ;;
  mipsel-unknown-linux-gnu)        image=dist-mipsel-linux ;;
  powerpc-unknown-linux-gnu)       image=dist-powerpc-linux ;;
  powerpc64-unknown-linux-gnu)     image=dist-powerpc64-linux ;;
  powerpc64le-unknown-linux-gnu)   image=dist-powerpc64le-linux ;;
  s390x-unknown-linux-gnu)         image=dist-s390x-linux ;;
  *) exit ;;
esac

master=$(git ls-remote "$RUST_REPO" refs/heads/master | cut -f1)
image_url="$S3_BASE_URL/$master/image-$image.txt"
info="/tmp/image-$image.txt"

rm -f "$info"
curl -o "$info" "$image_url"
digest=$(grep -m1 ^sha "$info")

if ! docker tag "$digest" "rust-$TARGET"; then
  url=$(grep -m1 ^https "$info")
  cache=/tmp/rustci_docker_cache
  echo "Attempting to download $url"
  rm -f "$cache"
  set +e
  retry curl -y 30 -Y 10 --connect-timeout 30 -f -L -C - -o "$cache" "$url"
  docker load -i "$cache"
  set -e
  docker tag "$digest" "rust-$TARGET"
fi
