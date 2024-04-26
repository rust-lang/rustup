FROM rust-x86_64-unknown-linux-gnu

 # Install `perl-IPC-Cmd` to make OpenSSL v3 happy.
 # See: <https://github.com/sfackler/rust-openssl/issues/1550>
 RUN yum upgrade -y && \
     yum install -y perl-IPC-Cmd
