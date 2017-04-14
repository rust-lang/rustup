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
  -u `id -u`:`id -g` \
  -v $HOME/rust:/travis-rust:ro \
  -v `pwd`:/buildslave:ro \
  -v `pwd`/target:/buildslave/target \
  -e TARGET=$TARGET \
  -e SKIP_TESTS=$SKIP_TESTS \
  -it $DOCKER \
  ci/run-docker.sh

# check that rustup-init was built with ssl support
# see https://github.com/rust-lang-nursery/rustup.rs/issues/1051
if ! (nm target/$TARGET/release/rustup-init | grep Curl_ssl_version &> /dev/null); then
  echo "Missing ssl support!!!!"
  exit 1
fi
