#!/bin/bash

script_dir=$(cd "$(dirname "$0")" && pwd)
. "$script_dir/shared.bash"

set -e
# Disable cause it makes shared script not to work properly
#set -x

TARGET="$1"

RUST_REPO="https://github.com/rust-lang/rust"
ARTIFACTS_BASE_URL="https://ci-artifacts.rust-lang.org/rustc-builds"
LOCAL_DOCKER_TAG="rust-$TARGET"

# Use images from rustc master
case "$TARGET" in
  aarch64-unknown-linux-gnu)       image=dist-aarch64-linux ;;
  arm-unknown-linux-gnueabi)       image=dist-arm-linux ;;
  arm-unknown-linux-gnueabihf)     image=dist-armhf-linux ;;
  armv7-unknown-linux-gnueabihf)   image=dist-armv7-linux ;;
  i686-unknown-linux-gnu)          image=dist-i686-linux ;;
  *-linux-android*)                image=dist-android; LOCAL_DOCKER_TAG=rust-android ;;
  mips-unknown-linux-gnu)          image=dist-mips-linux ;;
  mips64-unknown-linux-gnuabi64)   image=dist-mips64-linux ;;
  mips64el-unknown-linux-gnuabi64) image=dist-mips64el-linux ;;
  mipsel-unknown-linux-gnu)        image=dist-mipsel-linux ;;
  powerpc-unknown-linux-gnu)       image=dist-powerpc-linux ;;
  powerpc64-unknown-linux-gnu)     image=dist-powerpc64-linux ;;
  powerpc64le-unknown-linux-gnu)   image=dist-powerpc64le-linux ;;
  s390x-unknown-linux-gnu)         image=dist-s390x-linux ;;
  x86_64-unknown-freebsd)          image=dist-x86_64-freebsd ;;
  x86_64-unknown-linux-gnu)        image=dist-x86_64-linux ;;
  x86_64-unknown-netbsd)           image=dist-x86_64-netbsd ;;
  *) exit ;;
esac

master=$(git ls-remote "$RUST_REPO" refs/heads/master | cut -f1)
image_url="$ARTIFACTS_BASE_URL/$master/image-$image.txt"
info="/tmp/image-$image.txt"

rm -f "$info"
curl -o "$info" "$image_url"
digest=$(grep -m1 ^sha "$info")

if [ -z "$(docker images -q "${LOCAL_DOCKER_TAG}")" ]; then
  url=$(grep -m1 ^https "$info")
  cache=/tmp/rustci_docker_cache
  echo "Attempting to download $url"
  rm -f "$cache"
  set +e
  command_retry curl -y 30 -Y 10 --connect-timeout 30 -f -L -C - -o "$cache" "$url"
  set -e
  docker load --quiet -i "$cache"
  docker tag "$digest" "${LOCAL_DOCKER_TAG}"
fi
