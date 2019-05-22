FROM ubuntu:18.04

RUN apt-get update && apt-get install -y \
  musl-dev \
  musl-tools \
  curl \
  ca-certificates \
  perl \
  make \
  gcc

RUN curl -L https://github.com/mozilla/sccache/releases/download/0.2.8/sccache-0.2.8-x86_64-unknown-linux-musl.tar.gz | tar xzf - \
  && mv ./sccache-0.2.8-x86_64-unknown-linux-musl/sccache /usr/bin/sccache && rm -rf ./sccache-0.2.8-x86_64-unknown-linux-musl
