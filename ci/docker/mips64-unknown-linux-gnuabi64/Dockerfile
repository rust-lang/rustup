FROM ubuntu:16.04

RUN apt-get update && apt-get install -y \
  gcc-mips64-linux-gnuabi64 \
  curl \
  ca-certificates \
  perl \
  make \
  gcc

ENV CC_mips64_unknown_linux_gnuabi64=mips64-linux-gnuabi64-gcc
