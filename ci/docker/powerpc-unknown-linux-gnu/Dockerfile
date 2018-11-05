FROM ubuntu:18.04

RUN apt-get update && apt-get install -y \
  gcc-powerpc-linux-gnu \
  curl \
  ca-certificates \
  perl \
  make \
  gcc

ENV CC_powerpc_unknown_linux_gnu=powerpc-linux-gnu-gcc
ENV CARGO_TARGET_POWERPC_UNKNOWN_LINUX_GNU_LINKER=powerpc-linux-gnu-gcc
