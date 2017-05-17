FROM ubuntu:16.04

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    cmake \
    curl \
    gcc \
    libc6-dev \
    make \
    pkg-config

RUN apt-get install -y --no-install-recommends \
    unzip \
    python && \
    curl -O https://dl.google.com/android/repository/android-ndk-r13b-linux-x86_64.zip && \
    unzip -q android-ndk-r13b-linux-x86_64.zip && \
    ./android-ndk-r13b/build/tools/make_standalone_toolchain.py \
        --install-dir /android-ndk/arm \
        --arch arm \
        --api 21 && \
    ./android-ndk-r13b/build/tools/make_standalone_toolchain.py \
        --install-dir /android-ndk/arm64 \
        --arch arm64 \
        --api 21 && \
    ./android-ndk-r13b/build/tools/make_standalone_toolchain.py \
        --install-dir /android-ndk/x86 \
        --arch x86 \
        --api 21 && \
    rm -rf ./android-ndk-r13b-linux-x86_64.zip ./android-ndk-r13b && \
    apt-get purge --auto-remove -y unzip python

ENV PATH=$PATH:/android-ndk/arm/bin:/android-ndk/arm64/bin:/android-ndk/x86/bin

WORKDIR /buildslave
