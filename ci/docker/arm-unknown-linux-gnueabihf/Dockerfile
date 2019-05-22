FROM alexcrichton/rust-slave-linux-cross:2016-11-11
USER root

ENV CARGO_TARGET_ARM_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-linux-gnueabihf-gcc

COPY ci/docker/scripts/sccache.sh /scripts/
RUN sh /scripts/sccache.sh
