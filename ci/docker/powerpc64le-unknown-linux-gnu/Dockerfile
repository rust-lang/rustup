FROM ubuntu:18.04

RUN apt-get update && apt-get install -y \
  gcc-powerpc64le-linux-gnu \
  curl \
  ca-certificates \
  perl \
  make \
  gcc

ENV CC_powerpc64le_unknown_linux_gnu=powerpc64le-linux-gnu-gcc
ENV CARGO_TARGET_POWERPC64LE_UNKNOWN_LINUX_GNU_LINKER=powerpc64le-linux-gnu-gcc
