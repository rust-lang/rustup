FROM rust-x86_64-unknown-freebsd

# Building `aws-lc-rs` for FreeBSD on Linux depends on `gcc-multilib`, `libclang` and `bindgen`.
# See: https://aws.github.io/aws-lc-rs/requirements/linux
RUN apt-get update && apt-get install -qy gcc-multilib libclang-dev

ENV CC_x86_64_unknown_freebsd=x86_64-unknown-freebsd12-clang \
    CARGO_TARGET_X86_64_UNKNOWN_FREEBSD_LINKER=x86_64-unknown-freebsd12-clang
