#!/bin/bash

set -ex

TARGET="$1"

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

# Use images from rustc 1.35.0-nightly (94fd04589 2019-03-21)
# https://travis-ci.com/rust-lang/rust/builds/105351531
case "$TARGET" in

  mips-unknown-linux-gnu)
    # https://travis-ci.com/rust-lang/rust/jobs/186817407
    sha512=621751b996faaea4c97645afcb77aba84bbaf066ef50f9ef06c8fb28a488632781c2f59bd1fc07000db49d2145ff64abc449d13ed67a91cdd53f2806cf5034df
    sha256=ad1720127b9ebbc34ba4a36da2b5f5dc44ed2a5d0a46aa80e361ac8ede9df89b
    ;;

  mips64-unknown-linux-gnuabi64)
    # https://travis-ci.com/rust-lang/rust/jobs/186817408
    sha512=2a439812d28fca596323a3093d0032ae4ec77cb410ecf6be2d78df939138b72cf9a5660f6ed08c0f40996e932a163fed9225935796da608d3fb51458ee587053
    sha256=ff8e8673ae70a226570ddd41dfe07f0d8758218d4647c28b319b1cb4d715bc5f
    ;;

  mips64el-unknown-linux-gnuabi64)
    # https://travis-ci.com/rust-lang/rust/jobs/186817409
    sha512=f614e6f3632b28e5985599533dbed264cf626b014cfbb075a47c4fae59facc5e90a76272fa1c903bd9fa86a31fca4cc7c5ce6512c4abc5c0a588fa709b4f4514
    sha256=7b41705da7040451b9c275b2261c7056167cb3b592c9f6b0ecb15dc503c7eab5
    ;;

  mipsel-unknown-linux-gnu)
    # https://travis-ci.com/rust-lang/rust/jobs/186817410
    sha512=12094e9ef43e514b56f55eb622883e7be14668643804abd2e8c2811449176500ddc3f4ec15ff39cf83b60659d490f7828a3629118d0a1fbeed1d6a6cdeecaf25
    sha256=20b104f2b74aea708813448a146435a63c88ffa614f344db8a3067c9cd56680c
    ;;

  powerpc-unknown-linux-gnu)
    # https://travis-ci.com/rust-lang/rust/jobs/186817411
    sha512=a682cba347d2f1439b87a4c94edf234ea7a467cafb3c9158e324a976e69bb9f1b811a849af365ce8ab603b806ee162b738ecfd7da6f71af0c33f859e7575506e
    sha256=006bf866680845dfbf2f61d8a9d7e2b38d5d1604f3ba5313ec8187739ede8d26
    ;;

  powerpc64-unknown-linux-gnu)
    # https://travis-ci.com/rust-lang/rust/jobs/186817412
    sha512=ebdbb7a385b131f5d505eb75496978fc8bea2111e7eb9986323cac98ec869890eaf8bca164c7cfec03b6e990d049f8edcd4e3f127b2a2848a9c719cc2ba0fe4b
    sha256=7f6021816874b4e28cb46bcb55df52d26c3d43eefc173bd27feb7463b254b575
    ;;

  powerpc64le-unknown-linux-gnu)
    # https://travis-ci.com/rust-lang/rust/jobs/186817413
    sha512=4338d249c42d25d3d6cdd6626d43aaeec993e1320327694b957b3e3fc37243b238c2da5e206b0db831d98e9bf34158292473f7351d09271368425b3c35bb766b
    sha256=641b7f80f19b4f7d282ded96d910883a8572efb8bab0f261c07d2f4b56205a2c
    ;;

  s390x-unknown-linux-gnu)
    # https://travis-ci.com/rust-lang/rust/jobs/186817414
    sha512=19a5532aa1de3f58971ac796fb35114dc565f0fad06cba767df6fc29dfde559c7f6b2c437bc9be62e0b27e6e520eab442725d5bd1dee659b3860deb839a8513e
    sha256=0f9c5c37525fa000cbdacf55db5ebe485ffb6e0e19d9768e125157ac3f9650fd
    ;;

esac

if [ -n "$sha512" -a -n "$sha256" ]; then
  if ! docker tag "$sha256" "rust-$TARGET"; then
    url="https://s3-us-west-1.amazonaws.com/rust-lang-ci-sccache2/docker/$sha512"
    echo "Attempting to download $url"
    rm -f /tmp/rustci_docker_cache
    set +e
    retry curl -y 30 -Y 10 --connect-timeout 30 -f -L -C - -o /tmp/rustci_docker_cache "$url"
    docker load -i /tmp/rustci_docker_cache
    set -e
    docker tag "$sha256" "rust-$TARGET"
  fi
fi
