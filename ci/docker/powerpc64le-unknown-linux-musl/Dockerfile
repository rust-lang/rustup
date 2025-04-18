FROM rust-powerpc64le-unknown-linux-musl

# Building `aws-lc-rs` for Linux depends on `gcc-multilib`, `libclang` and `bindgen`.
# See: https://aws.github.io/aws-lc-rs/requirements/linux
RUN apt-get update && apt-get install -qy gcc-multilib libclang-dev

ENV CC_powerpc64le_unknown_linux_musl=powerpc64le-unknown-linux-musl-gcc \
    CARGO_TARGET_POWERPC64LE_UNKNOWN_LINUX_MUSL_LINKER=powerpc64le-unknown-linux-musl-gcc \
    RUSTFLAGS="-C target-feature=+crt-static -C link-arg=-lgcc"