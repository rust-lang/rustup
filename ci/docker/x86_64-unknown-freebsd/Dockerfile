FROM alexcrichton/rust-slave-linux-cross:2016-11-11
USER root

ENV CARGO_TARGET_X86_64_UNKNOWN_FREEBSD_LINKER=x86_64-unknown-freebsd10-gcc

COPY ci/docker/scripts/sccache.sh /scripts/
RUN sh /scripts/sccache.sh
