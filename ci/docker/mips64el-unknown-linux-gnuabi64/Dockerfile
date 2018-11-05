FROM ubuntu:18.04

RUN apt-get update && apt-get install -y \
  gcc-mips64el-linux-gnuabi64 \
  curl \
  ca-certificates \
  perl \
  make \
  gcc

ENV CC_mips64el_unknown_linux_gnuabi64=mips64el-linux-gnuabi64-gcc
ENV CARGO_TARGET_MIPS64EL_UNKNOWN_LINUX_GNUABI64_LINKER=mips64el-linux-gnuabi64-gcc
