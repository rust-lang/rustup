FROM rust-aarch64-unknown-freebsd

RUN apt-get update && apt-get install -qy gcc libclang-dev

# Workaround for a bug in xz2-rs, see PRs alexcrichton/xz2-rs#123 and alexcrichton/xz2-rs#134
ENV CFLAGS_aarch64_unknown_freebsd="-DHAVE_CLOCK_GETTIME=1"
ENV CC_aarch64_unknown_freebsd=aarch64-unknown-freebsd14-clang \
    CARGO_TARGET_AARCH64_UNKNOWN_FREEBSD_LINKER=aarch64-unknown-freebsd14-clang
