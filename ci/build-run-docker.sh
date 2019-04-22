#!/bin/bash

script_dir=$(cd "$(dirname "$0")" && pwd)
. "$script_dir/shared.sh"

set -e
# Disable cause it makes shared script not to work properly
#set -x

mkdir -p target

DOCKER="$1"
TARGET="$2"
SKIP_TESTS="$3"

travis_fold start "fetch.image.${TARGET}"
travis_time_start
travis_do_cmd sh ci/fetch-rust-docker.sh "$TARGET"
travis_time_finish
travis_fold end "fetch.image.${TARGET}"

if [ -f "ci/docker/$DOCKER/Dockerfile" ]; then
  travis_fold start "build.Dockerfile.${DOCKER}"
  travis_time_start
  travis_do_cmd docker build -t "$DOCKER" "ci/docker/$DOCKER/"
  travis_time_finish
  travis_fold end "build.Dockerfile.${DOCKER}"
fi

# shellcheck disable=SC2016
docker run \
  --entrypoint sh \
  --user "$(id -u)":"$(id -g)" \
  --volume "$(rustc --print sysroot)":/travis-rust:ro \
  --volume "$(pwd)":/src:ro \
  --volume "$(pwd)"/target:/src/target \
  --workdir /src \
  --env TARGET="$TARGET" \
  --env SKIP_TESTS="$SKIP_TESTS" \
  --env CARGO_HOME=/src/target/cargo-home \
  --env CARGO_TARGET_DIR=/src/target \
  --env LIBZ_SYS_STATIC=1 \
  --tty \
  --init \
  "$DOCKER" \
  -c 'PATH="$PATH":/travis-rust/bin exec sh ci/run.sh'

# check that rustup-init was built with ssl support
# see https://github.com/rust-lang/rustup.rs/issues/1051
if ! (nm target/"$TARGET"/release/rustup-init | grep -q Curl_ssl_version); then
  echo "Missing ssl support!!!!" >&2
  exit 1
fi
