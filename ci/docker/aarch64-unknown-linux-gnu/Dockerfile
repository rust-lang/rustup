FROM alexcrichton/rust-slave-linux-cross:2016-11-11
USER root

ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc

COPY ci/docker/scripts/sccache.sh /scripts/
RUN sh /scripts/sccache.sh
