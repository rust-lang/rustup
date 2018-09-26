FROM ubuntu:18.04

COPY ci/docker/scripts/sccache.sh /scripts/

RUN \
  apt-get update && \
  apt-get install -qy \
    musl-dev \
    musl-tools \
    curl \
    ca-certificates \
    perl \
    make \
    gcc && \
  sh /scripts/sccache.sh
