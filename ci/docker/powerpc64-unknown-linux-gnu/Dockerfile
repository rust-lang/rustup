FROM rust-powerpc64-unknown-linux-gnu

# Building `aws-lc-rs` for Linux depends on `gcc-multilib`, `libclang` and `bindgen`.
# See: https://aws.github.io/aws-lc-rs/requirements/linux
RUN apt-get update && apt-get install -qy gcc-multilib libclang-dev \
  && curl --proto '=https' --tlsv1.2 -LsSf https://github.com/rust-lang/rust-bindgen/releases/latest/download/bindgen-cli-installer.sh | sh \
  && mv $HOME/.cargo/bin/bindgen /usr/bin

ENV CC_powerpc64_unknown_linux_gnu=powerpc64-unknown-linux-gnu-gcc \
    CARGO_TARGET_POWERPC64_UNKNOWN_LINUX_GNU_LINKER=powerpc64-unknown-linux-gnu-gcc
