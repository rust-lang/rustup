FROM ubuntu:16.04

RUN apt-get update && apt-get install -y \
  gcc-mips-linux-gnu \
  curl \
  ca-certificates \
  perl \
  make \
  gcc

ENV CC_mips_unknown_linux_gnu=mips-linux-gnu-gcc
