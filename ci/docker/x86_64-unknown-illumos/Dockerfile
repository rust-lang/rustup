FROM rust-x86_64-unknown-illumos

# Building `aws-lc-rs` for Illumos on Linux depends on `gcc-multilib`, `libclang` and `bindgen`.
# See: https://aws.github.io/aws-lc-rs/requirements/linux
RUN apt-get update && apt-get install -qy gcc-multilib libclang-dev

ENV \
  AR_x86_64_unknown_illumos="x86_64-illumos-ar" \
  RANLIB_x86_64_unknown_illumos="x86_64-illumos-ranlib" \
  CC_x86_64_unknown_illumos=x86_64-illumos-gcc \
  CXX_x86_64_unknown_illumos=x86_64-illumos-g++ \
  CARGO_TARGET_X86_64_UNKNOWN_ILLUMOS_LINKER=x86_64-illumos-gcc
