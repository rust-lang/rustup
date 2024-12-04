FROM rust-loongarch64-unknown-linux-musl

# Building `aws-lc-rs` for Linux depends on `gcc-multilib`, `libclang` and `bindgen`.
# See: https://aws.github.io/aws-lc-rs/requirements/linux
RUN apt-get update && apt-get install -qy gcc-multilib libclang-dev

ENV CC_loongarch64_unknown_linux_musl=loongarch64-unknown-linux-musl-gcc \
    CARGO_TARGET_LOONGARCH64_UNKNOWN_LINUX_MUSL_LINKER=loongarch64-unknown-linux-musl-gcc
