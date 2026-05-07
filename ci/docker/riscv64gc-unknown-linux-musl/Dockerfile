FROM rust-riscv64gc-unknown-linux-musl

# Building `aws-lc-rs` for Linux depends on `gcc-multilib`, `libclang` and `bindgen`.
# See: https://aws.github.io/aws-lc-rs/requirements/linux
RUN apt-get update && apt-get install -qy gcc-multilib libclang-dev

ENV CC_riscv64gc_unknown_linux_musl=riscv64-unknown-linux-musl-gcc \
    CARGO_TARGET_RISCV64GC_UNKNOWN_LINUX_MUSL_LINKER=riscv64-unknown-linux-musl-gcc
