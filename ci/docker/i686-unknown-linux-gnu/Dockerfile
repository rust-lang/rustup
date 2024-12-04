FROM rust-i686-unknown-linux-gnu

# Install `perl-IPC-Cmd` to make OpenSSL v3 happy.
# See: <https://github.com/sfackler/rust-openssl/issues/1550>
RUN yum upgrade -y && \
    yum install -y perl-IPC-Cmd \
    # Building `aws-lc-rs` for Linux depends on `gcc-multilib`, `libclang` and `bindgen`.
    # See: https://aws.github.io/aws-lc-rs/requirements/linux
    glibc-devel.i686 clang-libs
