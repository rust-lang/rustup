#!/bin/bash

root_dir="$TRAVIS_BUILD_DIR"
script_dir="$root_dir/ci"
objdir="$root_dir"/obj

. "$script_dir/shared.sh"

set -e
# Disable cause it makes shared script not to work properly
#set -x

mkdir -p "$HOME"/.cargo
mkdir -p "$objdir"/cores
mkdir -p "$HOME"/.cache/sccache

# Enable core dump on Linux
sudo sh -c 'echo "/checkout/obj/cores/core.%p.%E" > /proc/sys/kernel/core_pattern';

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

# Run containers as privileged as it should give them access to some more
# syscalls such as ptrace and whatnot. In the upgrade to LLVM 5.0 it was
# discovered that the leak sanitizer apparently needs these syscalls nowadays so
# we'll need `--privileged` for at least the `x86_64-gnu` builder, so this just
# goes ahead and sets it for all builders.
# shellcheck disable=SC2016
docker run \
  --entrypoint /bin/sh \
  --user "$(id -u)":"$(id -g)" \
  --volume "$(rustc --print sysroot)":/rustc-sysroot:ro \
  --volume "$root_dir":/checkout:ro \
  --volume "$root_dir"/target:/checkout/target \
  --volume "$objdir":/checkout/obj \
  --workdir /checkout \
  --privileged \
  --env TARGET="$TARGET" \
  --env SKIP_TESTS="$SKIP_TESTS" \
  --volume "$HOME/.cargo:/cargo" \
  --env CARGO_HOME=/cargo \
  --env CARGO_TARGET_DIR=/checkout/target \
  --env LIBZ_SYS_STATIC=1 \
  --volume "$HOME"/.cache/sccache:/sccache \
  --env SCCACHE_DIR=/sccache \
  --tty \
  --init \
  --rm \
  "$DOCKER" \
  -c 'PATH="$PATH":/rustc-sysroot/bin sh ci/run.sh'

# check that rustup-init was built with ssl support
# see https://github.com/rust-lang/rustup.rs/issues/1051
if ! (nm target/"$TARGET"/release/rustup-init | grep -q Curl_ssl_version); then
  echo "Missing ssl support!!!!" >&2
  exit 1
fi
