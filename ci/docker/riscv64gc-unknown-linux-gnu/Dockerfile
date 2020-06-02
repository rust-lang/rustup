FROM rust-riscv64gc-unknown-linux-gnu


# We install a stable toolchain using rustup anyway, this should stop us getting
# confused and still having this toolchain in $PATH
RUN /usr/local/lib/rustlib/uninstall.sh
# Weirdly that doesn't remove these:
RUN rm /usr/local/bin/cargo /usr/local/bin/rust*

ENV CC_riscv64gc_unknown_linux_gnu=riscv64-unknown-linux-gnu-gcc \
    CARGO_TARGET_RISCV64GC_UNKNOWN_LINUX_GNU_LINKER=riscv64-unknown-linux-gnu-gcc
