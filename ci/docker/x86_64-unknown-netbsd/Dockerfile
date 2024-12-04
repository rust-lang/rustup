FROM rust-x86_64-unknown-netbsd

# Building `aws-lc-rs` for NetBSD on Linux depends on `gcc-multilib`, `libclang` and `bindgen`.
# See: https://aws.github.io/aws-lc-rs/requirements/linux
RUN apt-get update && apt-get install -qy gcc-multilib libclang-dev

ENV CARGO_TARGET_X86_64_UNKNOWN_NETBSD_LINKER=x86_64--netbsd-gcc-sysroot
