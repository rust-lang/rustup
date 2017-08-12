#!/bin/sh

set -ex

mkdir -p target

DOCKER="$1"
TARGET="$2"
SKIP_TESTS="$3"

if [ -f "ci/docker/$DOCKER/Dockerfile" ]; then
  docker build -t "$DOCKER" "ci/docker/$DOCKER/"
fi

docker run \
  --entrypoint bash \
  --user `id -u`:`id -g` \
  --volume `rustc --print sysroot`:/travis-rust:ro \
  --volume `pwd`:/src:ro \
  --volume `pwd`/target:/src/target \
  --workdir /src \
  --env TARGET=$TARGET \
  --env SKIP_TESTS=$SKIP_TESTS \
  $DOCKER \
  ci/run-docker.sh

# check that rustup-init was built with ssl support
# see https://github.com/rust-lang-nursery/rustup.rs/issues/1051
if ! (nm target/$TARGET/release/rustup-init | grep Curl_ssl_version &> /dev/null); then
  echo "Missing ssl support!!!!" >&2
  exit 1
fi
