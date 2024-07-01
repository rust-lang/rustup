FROM rust-armv7-unknown-linux-gnueabihf

RUN apt-get update && apt-get install -qy gcc-multilib libclang-dev

ENV CC_armv7_unknown_linux_gnueabihf=armv7-unknown-linux-gnueabihf-gcc \
  CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=armv7-unknown-linux-gnueabihf-gcc
