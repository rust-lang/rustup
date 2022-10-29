#!/bin/bash

set -eux -o pipefail

TARGET=$1
SKIP_TESTS=$2
DOCKER=$3

mkdir -p "${PWD}/target"
chown -R "$(id -u)":"$(id -g)" "${PWD}/target"
docker run \
    --entrypoint sh \
    --user "$(id -u)":"$(id -g)" \
    --volume "$(rustc --print sysroot)":/rustc-sysroot:ro \
    --volume "${PWD}":/checkout:ro \
    --volume "${PWD}"/target:/checkout/target \
    --workdir /checkout \
    --env TARGET="${TARGET}" \
    --env SKIP_TESTS="${SKIP_TESTS}" \
    --volume "${HOME}/.cargo:/cargo" \
    --env CARGO_HOME=/cargo \
    --env CARGO_TARGET_DIR=/checkout/target \
    --env LIBZ_SYS_STATIC=1 \
    --tty \
    --init \
    --rm \
    "${DOCKER}" \
    -c 'PATH="${PATH}":/rustc-sysroot/bin bash ci/run.bash'