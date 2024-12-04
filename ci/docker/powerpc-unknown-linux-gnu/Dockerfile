FROM rust-powerpc-unknown-linux-gnu

# Building `aws-lc-rs` for Linux depends on `gcc-multilib`, `libclang` and `bindgen`.
# See: https://aws.github.io/aws-lc-rs/requirements/linux
RUN apt-get update && apt-get install -qy gcc-multilib libclang-dev

ENV CC_powerpc_unknown_linux_gnu=powerpc-unknown-linux-gnu-gcc \
    CARGO_TARGET_POWERPC_UNKNOWN_LINUX_GNU_LINKER=powerpc-unknown-linux-gnu-gcc
