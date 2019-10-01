#!/bin/bash

script_dir=$(cd "$(dirname "$0")" && pwd)
root_dir="${script_dir}/.."

. "${script_dir}/shared.bash"

set -e
# Disable cause it makes shared script not to work properly
#set -x

mkdir -p target
mkdir -p "${HOME}"/.cargo
mkdir -p "${HOME}"/.cache/sccache

DOCKER="$1"
TARGET="$2"
SKIP_TESTS="$3"

travis_fold start "fetch.image.${TARGET}"
travis_time_start
travis_do_cmd bash ci/fetch-rust-docker.bash "${TARGET}"
travis_time_finish
travis_fold end "fetch.image.${TARGET}"

if [ -f "ci/docker/$DOCKER/Dockerfile" ]; then
  travis_fold start "build.Dockerfile.${DOCKER}"
  travis_time_start
  travis_do_cmd docker build -t "$DOCKER" -f "ci/docker/${DOCKER}/Dockerfile" .
  travis_time_finish
  travis_fold end "build.Dockerfile.${DOCKER}"
fi

# shellcheck disable=SC2016
docker run \
  --entrypoint sh \
  --user "$(id -u)":"$(id -g)" \
  --volume "$(rustc --print sysroot)":/rustc-sysroot:ro \
  --volume "${root_dir}":/checkout:ro \
  --volume "${root_dir}"/target:/checkout/target \
  --workdir /checkout \
  --env TARGET="${TARGET}" \
  --env SKIP_TESTS="${SKIP_TESTS}" \
  --volume "${HOME}/.cargo:/cargo" \
  --env CARGO_HOME=/cargo \
  --env CARGO_TARGET_DIR=/checkout/target \
  --env LIBZ_SYS_STATIC=1 \
  --volume "${HOME}"/.cache/sccache:/sccache \
  --env SCCACHE_DIR=/sccache \
  --env RUSTC_WRAPPER=sccache \
  --tty \
  --init \
  --rm \
  "${DOCKER}" \
  -c 'PATH="${PATH}":/rustc-sysroot/bin bash ci/run.bash'

# check that rustup-init was built with ssl support
# see https://github.com/rust-lang/rustup.rs/issues/1051
if ! (nm target/"${TARGET}"/release/rustup-init | grep -q openssl_sys); then
  echo "Missing ssl support!!!!" >&2
  exit 1
fi
