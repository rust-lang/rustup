# Based on https://github.com/rust-lang/rust/blob/master/src/ci/docker/dist-android/Dockerfile
FROM ubuntu:18.04
USER root

COPY ci/docker/scripts/sccache.bash /scripts/

RUN apt-get update -qq && \
    apt-get install -qq --no-install-recommends \
        ca-certificates \
        cmake \
        curl \
        g++ \
        libc6-dev \
        make \
        pkg-config \
        python2.7 \
        unzip \
        xz-utils && \
    bash /scripts/sccache.bash && \
    ANDROID_EMU_URL=https://dl.google.com/android/repository && \
    ANDROID_NDK_FILE=android-ndk-r15c-linux-x86_64.zip && \
    curl -fO "${ANDROID_EMU_URL}/${ANDROID_NDK_FILE}" && \
    unzip -q "./${ANDROID_NDK_FILE}" && \
    rm "${ANDROID_NDK_FILE}" && \
    mv android-ndk-* ndk && \
    python2.7 ./ndk/build/tools/make_standalone_toolchain.py \
        --install-dir /android-ndk/arm \
        --arch arm \
        --api 21 && \
    python2.7 ./ndk/build/tools/make_standalone_toolchain.py \
        --install-dir /android-ndk/arm64 \
        --arch arm64 \
        --api 21 && \
    python2.7 ./ndk/build/tools/make_standalone_toolchain.py \
        --install-dir /android-ndk/x86 \
        --arch x86 \
        --api 21 && \
    python2.7 ./ndk/build/tools/make_standalone_toolchain.py \
        --install-dir /android-ndk/x86_64 \
        --arch x86_64 \
        --api 21 && \
    rm -rf ./ndk

ENV PATH=$PATH:/android-ndk/arm/bin:/android-ndk/arm64/bin:/android-ndk/x86/bin:/android-ndk/x86_64/bin \
    CC_arm_linux_androideabi=arm-linux-androideabi-clang \
    CC_armv7_linux_androideabi=arm-linux-androideabi-clang \
    CC_aarch64_linux_android=aarch64-linux-android-clang \
    CC_i686_linux_android=i686-linux-android-clang \
    CC_x86_64_linux_android=x86_64-linux-android-clang \
    CXX_arm_linux_androideabi=arm-linux-androideabi-clang++ \
    CXX_armv7_linux_androideabi=arm-linux-androideabi-clang++ \
    CXX_aarch64_linux_android=aarch64-linux-android-clang++ \
    CXX_i686_linux_android=i686-linux-android-clang++ \
    CXX_x86_64_linux_android=x86_64-linux-android-clang++ \
    CARGO_TARGET_ARM_LINUX_ANDROIDEABI_LINKER=arm-linux-androideabi-clang \
    CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER=arm-linux-androideabi-clang \
    CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=aarch64-linux-android-clang \
    CARGO_TARGET_I686_LINUX_ANDROID_LINKER=i686-linux-android-clang \
    CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER=x86_64-linux-android-clang

WORKDIR /buildslave
