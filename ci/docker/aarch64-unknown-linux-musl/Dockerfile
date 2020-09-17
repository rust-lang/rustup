FROM rust-aarch64-unknown-linux-musl

ENV CC_aarch64_unknown_linux_musl=aarch64-linux-musl-gcc \
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=aarch64-linux-musl-gcc \
    RUSTFLAGS="-C target-feature=+crt-static -C link-arg=-lgcc"
