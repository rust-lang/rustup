FROM rust-armv7-unknown-linux-gnueabihf

# Building `aws-lc-rs` for Linux depends on `gcc-multilib`, `libclang` and `bindgen`.
# See: https://aws.github.io/aws-lc-rs/requirements/linux
RUN apt-get update && apt-get install -qy gcc-multilib libclang-dev \
  && curl --proto '=https' --tlsv1.2 -LsSf https://github.com/rust-lang/rust-bindgen/releases/latest/download/bindgen-cli-installer.sh | sh \
  && mv $HOME/.cargo/bin/bindgen /usr/bin

ENV CC_armv7_unknown_linux_gnueabihf=armv7-unknown-linux-gnueabihf-gcc \
    CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=armv7-unknown-linux-gnueabihf-gcc
