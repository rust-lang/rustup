FROM rust-aarch64-unknown-freebsd

# Building `aws-lc-rs` for FreeBSD on Linux depends on `gcc-multilib`, `libclang` and `bindgen`.
# See: https://aws.github.io/aws-lc-rs/requirements/linux
RUN apt-get update && apt-get install -qy gcc-multilib libclang-dev

ENV CC_aarch64_unknown_freebsd=aarch64-unknown-freebsd14-clang \
    CARGO_TARGET_AARCH64_UNKNOWN_FREEBSD_LINKER=aarch64-unknown-freebsd14-clang
