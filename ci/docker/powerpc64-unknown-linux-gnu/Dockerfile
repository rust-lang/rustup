FROM ubuntu:16.04

RUN apt-get update && apt-get install -y \
  gcc-powerpc64-linux-gnu \
  curl \
  ca-certificates \
  perl \
  make \
  gcc

ENV CC_powerpc64_unknown_linux_gnu=powerpc64-linux-gnu-gcc
