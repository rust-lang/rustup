#!/bin/bash

set -efu

set -o pipefail

# This script encapsulates the job of preparing snapcract configuration for the
# rustup snap.  We construct either an edge snap (builds on PRs or master)
# or a beta snap if built from a non-master branch.

# If built on master, we should publish the edge snap
# If built on stable, we should publish the beta snap

# To parameterise the build, the following environment variables are needed:
# DO_SNAP=1  <-- without this, we don't run
# SNAP_EDGE=1 <-- with this, we build an edge/devel snap, otherwise a release snap
# SNAP_ARCH=... <-- without this we cannot build, this is the snap architecture we're building

if [ -z "${DO_SNAP:-}" ]; then
  echo "SNAP: Not attempting to snap"
  exit 0
fi

if [ -z "${SNAP_ARCH:-}" ]; then
  echo "SNAP: Unable to generate snap, architecture unset"
  exit 1
else
  echo "SNAP: Constructing snap for architecture $SNAP_ARCH"
fi

if [ -n "${SNAP_EDGE:-}" ]; then
  echo "SNAP: Generated snap will be an grade:devel snap"
  GRADE=devel
else
  echo "SNAP: Generated snap will be a grade:stable snap"
  GRADE=stable
fi


# This is the list of proxies which the snap will contain.  This needs to match
# the set of aliases which the snap has been allocated.  If we add to this
# list but don't get the aliases added, then people won't see the proxy except
# as rustup.$PROXY.  If we have an alias which is not a supported proxy name
# then rustup might get sad.

PROXIES="cargo cargo-clippy cargo-fmt cargo-miri clippy-driver rls rustc rustdoc rustfmt"

# From now on, things should be automagic

VERSION=$(grep "^version" Cargo.toml | head -1 | cut -d\" -f2)

rm -rf snapcraft

mkdir -p snapcraft/snap

cd snapcraft

cat > snap/snapcraft.yaml <<SNAP
name: rustup
version: $VERSION
confinement: classic
base: core18
architectures:
  - build-on: [amd64]
    run-on: [$SNAP_ARCH]
description: |
  Rustup is the Rust Language's installer and primary front-end.  You probably
  want this if you want to develop anything written in Rust.
SNAP

if [ -n "${SNAP_EDGE:-1}" ]; then
  cat >> snap/snapcraft.yaml <<SNAP

  Please note, this snap is experimental and functionality cannot be guaranteed
  to be consistent with released snaps.
summary: "EXPERIMENTAL: The Rust Language Installer"
grade: $GRADE
SNAP
else
  cat >> snap/snapcraft.yaml <<SNAP
summary: "The Rust Language Installer"
grade: $GRADE
SNAP
fi

cat >> snap/snapcraft.yaml <<SNAP
parts:
  rustup:
    plugin: nil
    build-attributes: [no-patchelf]
    source: inputs
    override-build: |
      mkdir -p \$SNAPCRAFT_PART_INSTALL/bin
      cp rustup-init \$SNAPCRAFT_PART_INSTALL/bin/rustup
SNAP

for PROXY in $PROXIES; do
  cat >> snap/snapcraft.yaml <<SNAP
      ln \$SNAPCRAFT_PART_INSTALL/bin/rustup \$SNAPCRAFT_PART_INSTALL/bin/$PROXY
SNAP
done

cat >> snap/snapcraft.yaml << 'SNAP'
    prime:
     - "-rustup-init"

environment:
  RUSTUP_HOME: "$SNAP_USER_COMMON/rustup"

apps:
  rustup:
    command: bin/rustup
SNAP

for PROXY in $PROXIES; do
  cat >> snap/snapcraft.yaml <<SNAP
  $PROXY:
    command: bin/$PROXY
SNAP
done

mkdir inputs
cp ../target/"$TARGET"/release/rustup-init inputs/

ls -l snap/snapcraft.yaml
