FROM rust-android

ENV PATH=$PATH:/android/ndk/toolchains/llvm/prebuilt/linux-x86_64/bin \
    CC_arm_linux_androideabi=armv7a-linux-androideabi19-clang \
    CC_armv7_linux_androideabi=armv7a-linux-androideabi19-clang \
    CC_aarch64_linux_android=aarch64-linux-android21-clang \
    CC_i686_linux_android=i686-linux-android19-clang \
    CC_x86_64_linux_android=x86_64-linux-android21-clang \
    CXX_arm_linux_androideabi=armv7a-linux-androideabi19-clang++ \
    CXX_armv7_linux_androideabi=armv7a-linux-androideabi19-clang++ \
    CXX_aarch64_linux_android=aarch64-linux-android21-clang++ \
    CXX_i686_linux_android=i686-linux-android19-clang++ \
    CXX_x86_64_linux_android=x86_64-linux-android21-clang++ \
    CARGO_TARGET_ARM_LINUX_ANDROIDEABI_LINKER=armv7a-linux-androideabi19-clang \
    CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER=armv7a-linux-androideabi19-clang \
    CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=aarch64-linux-android21-clang \
    CARGO_TARGET_I686_LINUX_ANDROID_LINKER=i686-linux-android19-clang \
    CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER=x86_64-linux-android21-clang
