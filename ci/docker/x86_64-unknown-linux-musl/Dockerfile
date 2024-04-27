FROM ubuntu:24.04

COPY ci/docker/scripts/sccache.bash /scripts/

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
  bash /scripts/sccache.bash
