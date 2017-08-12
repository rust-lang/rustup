FROM ubuntu:16.04

RUN apt-get update && apt-get install -y \
  gcc-s390x-linux-gnu \
  curl \
  ca-certificates \
  perl \
  make \
  gcc

ENV CC_s390x_unknown_linux_gnu=s390x-linux-gnu-gcc
