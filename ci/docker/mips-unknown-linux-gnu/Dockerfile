FROM ubuntu:18.04

RUN apt-get update && apt-get install -y \
  gcc-mips-linux-gnu \
  curl \
  ca-certificates \
  perl \
  make \
  gcc

ENV CC_mips_unknown_linux_gnu=mips-linux-gnu-gcc
ENV CARGO_TARGET_MIPS_UNKNOWN_LINUX_GNU_LINKER=mips-linux-gnu-gcc
