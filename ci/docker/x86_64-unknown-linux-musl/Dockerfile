FROM alpine:3.8

RUN apk update && \
  apk add \
  curl \
  ca-certificates \
  perl \
  make \
  gcc

ENV CC_x86_64_unknown_linux_musl=gcc
