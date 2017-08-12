FROM ubuntu:16.04

RUN apt-get update && apt-get install -y \
  gcc-mipsel-linux-gnu \
  curl \
  ca-certificates \
  perl \
  make \
  gcc

ENV CC_mipsel_unknown_linux_gnu=mipsel-linux-gnu-gcc
