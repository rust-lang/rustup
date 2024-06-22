FROM rust-i686-unknown-linux-gnu

# Building `aws-lc-rs` for Linux depends on `gcc-multilib`, `libclang` and `bindgen`.
# See: https://aws.github.io/aws-lc-rs/requirements/linux
RUN apt-get update && apt-get install -qy gcc-multilib libclang-dev \
  && curl --proto '=https' --tlsv1.2 -LsSf https://github.com/rust-lang/rust-bindgen/releases/latest/download/bindgen-cli-installer.sh | sh \
  && mv $HOME/.cargo/bin/bindgen /usr/bin

 # Install `perl-IPC-Cmd` to make OpenSSL v3 happy.
 # See: <https://github.com/sfackler/rust-openssl/issues/1550>
 RUN yum upgrade -y && \
     yum install -y perl-IPC-Cmd
