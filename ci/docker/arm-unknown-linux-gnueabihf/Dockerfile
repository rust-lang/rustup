FROM rust-arm-unknown-linux-gnueabihf

# Building `aws-lc-rs` for Linux depends on `gcc-multilib`, `libclang` and `bindgen`.
# See: https://aws.github.io/aws-lc-rs/requirements/linux
RUN apt-get update && apt-get install -qy gcc-multilib libclang-dev \
  && curl --proto '=https' --tlsv1.2 -LsSf https://github.com/rust-lang/rust-bindgen/releases/latest/download/bindgen-cli-installer.sh | sh \
  && mv $HOME/.cargo/bin/bindgen /usr/bin

ENV CC_arm_unknown_linux_gnueabihf=arm-unknown-linux-gnueabihf-gcc \
    CARGO_TARGET_ARM_UNKNOWN_LINUX_GNUEABIHF_LINKER=arm-unknown-linux-gnueabihf-gcc
