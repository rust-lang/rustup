FROM rust-x86_64-pc-solaris

# Building `aws-lc-rs` for Solaris on Linux depends on `gcc-multilib`, `libclang` and `bindgen`.
# See: https://aws.github.io/aws-lc-rs/requirements/linux
RUN apt-get update && apt-get install -qy gcc-multilib libclang-dev

ENV \
  AR_x86_64_pc_solaris=x86_64-solaris-ar \
  RANLIB_x86_64_pc_solaris=x86_64-solaris-ranlib \
  CC_x86_64_pc_solaris=x86_64-solaris-gcc \
  CXX_x86_64_pc_solaris=x86_64-solaris-g++ \
  CARGO_TARGET_X86_64_PC_SOLARIS_LINKER=x86_64-solaris-gcc
