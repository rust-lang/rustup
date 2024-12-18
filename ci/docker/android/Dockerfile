FROM rust-android

# Building `aws-lc-rs` for Linux depends on `gcc-multilib`, `libclang` and `bindgen`.
# See: https://aws.github.io/aws-lc-rs/requirements/linux
RUN apt-get update && apt-get install -qy gcc-multilib libclang-dev

ENV PATH=$PATH:/android/ndk/toolchains/llvm/prebuilt/linux-x86_64/bin \
    ANDROID_NDK=/android/ndk/ \
    AR_arm_linux_androideabi=llvm-ar \
    AR_armv7_linux_androideabi=llvm-ar \
    AR_aarch64_linux_android=llvm-ar \
    AR_i686_linux_android=llvm-ar \
    AR_x86_64_linux_android=llvm-ar \
    RANLIB_arm_linux_androideabi=llvm-ranlib \
    RANLIB_armv7_linux_androideabi=llvm-ranlib \
    RANLIB_aarch64_linux_android=llvm-ranlib \
    RANLIB_i686_linux_android=llvm-ranlib \
    RANLIB_x86_64_linux_android=llvm-ranlib \
    CC_arm_linux_androideabi=armv7a-linux-androideabi23-clang \
    CC_armv7_linux_androideabi=armv7a-linux-androideabi23-clang \
    CC_aarch64_linux_android=aarch64-linux-android23-clang \
    CC_i686_linux_android=i686-linux-android23-clang \
    CC_x86_64_linux_android=x86_64-linux-android23-clang \
    CXX_arm_linux_androideabi=armv7a-linux-androideabi23-clang++ \
    CXX_armv7_linux_androideabi=armv7a-linux-androideabi23-clang++ \
    CXX_aarch64_linux_android=aarch64-linux-android23-clang++ \
    CXX_i686_linux_android=i686-linux-android23-clang++ \
    CXX_x86_64_linux_android=x86_64-linux-android23-clang++ \
    CARGO_TARGET_ARM_LINUX_ANDROIDEABI_LINKER=armv7a-linux-androideabi23-clang \
    CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER=armv7a-linux-androideabi23-clang \
    CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=aarch64-linux-android23-clang \
    CARGO_TARGET_I686_LINUX_ANDROID_LINKER=i686-linux-android23-clang \
    CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER=x86_64-linux-android23-clang
