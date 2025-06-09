FROM rust-sparcv9-sun-solaris

# Building `aws-lc-rs` for Solaris on Linux depends on `gcc-multilib`, `libclang` and `bindgen`.
# See: https://aws.github.io/aws-lc-rs/requirements/linux
RUN apt-get update && apt-get install -qy gcc-multilib libclang-dev

ENV \
  AR_sparcv9_sun_solaris=sparcv9-solaris-ar \
  RANLIB_sparcv9_sun_solaris=sparcv9-solaris-ranlib \
  CC_sparcv9_sun_solaris=sparcv9-solaris-gcc \
  CXX_sparcv9_sun_solaris=sparcv9-solaris-g++ \
  CARGO_TARGET_SPARCV9_SUN_SOLARIS_LINKER=sparcv9-solaris-gcc
