FROM rust-s390x-unknown-linux-gnu

# Building `aws-lc-rs` for Linux depends on `gcc-multilib`, `libclang` and `bindgen`.
# See: https://aws.github.io/aws-lc-rs/requirements/linux
RUN apt-get update && apt-get install -qy gcc-multilib libclang-dev \
  && curl --proto '=https' --tlsv1.2 -LsSf https://github.com/rust-lang/rust-bindgen/releases/latest/download/bindgen-cli-installer.sh | sh \
  && mv $HOME/.cargo/bin/bindgen /usr/bin

ENV CC_s390x_unknown_linux_gnu=s390x-ibm-linux-gnu-gcc \
    CARGO_TARGET_S390X_UNKNOWN_LINUX_GNU_LINKER=s390x-ibm-linux-gnu-gcc
