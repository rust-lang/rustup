FROM ubuntu:16.04

RUN apt-get update && apt-get install -y \
  musl-tools \
  curl \
  ca-certificates \
  perl \
  make \
  gcc

ENV CC_x86_64_unknown_linux_musl=musl-gcc
