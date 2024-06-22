FROM rust-riscv64gc-unknown-linux-gnu

# Building `aws-lc-rs` for Linux depends on `gcc-multilib`, `libclang` and `bindgen`.
# See: https://aws.github.io/aws-lc-rs/requirements/linux
RUN apt-get update && apt-get install -qy gcc-multilib libclang-dev \
  && curl --proto '=https' --tlsv1.2 -LsSf https://github.com/rust-lang/rust-bindgen/releases/latest/download/bindgen-cli-installer.sh | sh \
  && mv $HOME/.cargo/bin/bindgen /usr/bin

ENV CC_riscv64gc_unknown_linux_gnu=riscv64-unknown-linux-gnu-gcc \
    CARGO_TARGET_RISCV64GC_UNKNOWN_LINUX_GNU_LINKER=riscv64-unknown-linux-gnu-gcc
