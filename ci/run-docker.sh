#!/bin/sh

set -ex

# For some unknown reason libz is not found in the android docker image, so we
# use this workaround
case $TARGET in
  arm-linux-androideabi | armv7-linux-androideabi )
    export DEP_Z_ROOT=/android-ndk/arm/sysroot/usr/;;

  aarch64-linux-android )
    export DEP_Z_ROOT=/android-ndk/arm64/sysroot/usr/;;

  i686-linux-android )
    export DEP_Z_ROOT=/android-ndk/x86/sysroot/usr/;;
esac

upper_target=$(echo $TARGET | tr '[a-z]' '[A-Z]' | tr '-' '_')
export PATH=/travis-rust/bin:$PATH
export LD_LIBRARY_PATH=/travis-rust/lib:$LD_LIBRARY_PATH

# ==============================================================================
# First up, let's compile OpenSSL
#
# The artifacts that we distribute must all statically be linked to OpenSSL
# because we have no idea what system we're going to be running on eventually.
# The target system may or may not have OpenSSL installed and it also may have
# any one of a number of ABI-incompatible OpenSSL versions installed.
#
# To get around all this we just compile it statically for the rustup *we*
# distribute (this can be changed by others of course).
# ==============================================================================

OPENSSL_VERS=1.0.2k
OPENSSL_SHA256=6b3977c61f2aedf0f96367dcfb5c6e578cf37e7b8d913b4ecb6643c3cb88d8c0

case $TARGET in
  x86_64-*-linux-*)
    OPENSSL_OS=linux-x86_64
    OPENSSL_CC=gcc
    OPENSSL_AR=ar
    ;;
  i686-*-linux-*)
    OPENSSL_OS=linux-elf
    OPENSSL_CC=gcc
    OPENSSL_AR=ar
    OPENSSL_SETARCH='setarch i386'
    OPENSSL_CFLAGS=-m32
    ;;
  arm-linux-androideabi)
    OPENSSL_OS=android
    OPENSSL_CC=arm-linux-androideabi-gcc
    OPENSSL_AR=arm-linux-androideabi-ar
    ;;
  armv7-linux-androideabi)
    OPENSSL_OS=android-armv7
    OPENSSL_CC=arm-linux-androideabi-gcc
    OPENSSL_AR=arm-linux-androideabi-ar
    ;;
  aarch64-linux-android)
    OPENSSL_OS=linux-generic64
    OPENSSL_CC=aarch64-linux-android-gcc
    OPENSSL_AR=aarch64-linux-android-ar
    OPENSSL_CFLAGS="-mandroid -fomit-frame-pointer"
    ;;
  i686-linux-android)
    OPENSSL_OS=android-x86
    OPENSSL_CC=i686-linux-android-gcc
    OPENSSL_AR=i686-linux-android-ar
    ;;
  arm-*-linux-gnueabi)
    OPENSSL_OS=linux-armv4
    OPENSSL_CC=arm-linux-gnueabi-gcc
    OPENSSL_AR=arm-linux-gnueabi-ar
    ;;
  arm-*-linux-gnueabihf)
    OPENSSL_OS=linux-armv4
    OPENSSL_CC=arm-linux-gnueabihf-gcc
    OPENSSL_AR=arm-linux-gnueabihf-ar
    ;;
  armv7-*-linux-gnueabihf)
    OPENSSL_OS=linux-armv4
    OPENSSL_CC=armv7-linux-gnueabihf-gcc
    OPENSSL_AR=armv7-linux-gnueabihf-ar
    ;;
  aarch64-*-linux-gnu)
    OPENSSL_OS=linux-aarch64
    OPENSSL_CC=aarch64-linux-gnu-gcc
    OPENSSL_AR=aarch64-linux-gnu-ar
    ;;
  x86_64-*-freebsd)
    OPENSSL_OS=BSD-x86_64
    OPENSSL_CC=x86_64-unknown-freebsd10-gcc
    OPENSSL_AR=x86_64-unknown-freebsd10-ar
    ;;
  x86_64-*-netbsd)
    OPENSSL_OS=BSD-x86_64
    OPENSSL_CC=x86_64-unknown-netbsd-gcc
    OPENSSL_AR=x86_64-unknown-netbsd-ar
    ;;
  powerpc-*-linux-*)
    OPENSSL_OS=linux-ppc
    OPENSSL_CC=powerpc-linux-gnu-gcc
    OPENSSL_AR=powerpc-linux-gnu-ar
    ;;
  powerpc64-*-linux-*)
    OPENSSL_OS=linux-ppc64
    OPENSSL_CC=powerpc64-linux-gnu-gcc-5
    OPENSSL_AR=powerpc64-linux-gnu-ar
    OPENSSL_CFLAGS=-m64
    ;;
  powerpc64le-*-linux-*)
    OPENSSL_OS=linux-ppc64le
    OPENSSL_CC=powerpc64le-linux-gnu-gcc
    OPENSSL_AR=powerpc64le-linux-gnu-ar
    ;;
  mips-*-linux-*)
    OPENSSL_OS=linux-mips32
    OPENSSL_CC=mips-linux-gnu-gcc
    OPENSSL_AR=mips-linux-gnu-ar
    ;;
  mipsel-*-linux-*)
    OPENSSL_OS=linux-mips32
    OPENSSL_CC=mipsel-linux-gnu-gcc
    OPENSSL_AR=mipsel-linux-gnu-ar
    ;;
  mips64-*-linux-*)
    OPENSSL_OS=linux64-mips64
    OPENSSL_CC=mips64-linux-gnuabi64-gcc
    OPENSSL_AR=mips64-linux-gnuabi64-ar
    ;;
  mips64el-*-linux-*)
    OPENSSL_OS=linux64-mips64
    OPENSSL_CC=mips64el-linux-gnuabi64-gcc
    OPENSSL_AR=mips64el-linux-gnuabi64-ar
    ;;
  s390x-*-linux-*)
    OPENSSL_OS=linux64-s390x
    OPENSSL_CC=s390x-linux-gnu-gcc
    OPENSSL_AR=s390x-linux-gnu-ar
    ;;
  *)
    echo "can't cross compile OpenSSL for $TARGET"
    exit 1
    ;;
esac

install=`pwd`/target/$TARGET/openssl/openssl-install/$OPENSSL_VERS

if [ -e $install ]; then
  echo 'Using cached OpenSSL static libs'
else
  # Clean up any builds of previous versions from the cache
  rm -rf $(dirname $install)/*

  # If the build fails half way through it will be difficult to distinguish when the next run sees
  # the cached version, so finalize the build atomically. We're linking statically so don't need to
  # worry about using a different prefix at install time.
  final_install_path=$install
  install=$install-partial

  mkdir -p target/$TARGET/openssl
  out=`pwd`/target/$TARGET/openssl/openssl-$OPENSSL_VERS.tar.gz
  curl -o $out https://www.openssl.org/source/openssl-$OPENSSL_VERS.tar.gz
  sha256sum $out > $out.sha256
  test $OPENSSL_SHA256 = `cut -d ' ' -f 1 $out.sha256`

  tar xf $out -C target/$TARGET/openssl
  (cd target/$TARGET/openssl/openssl-$OPENSSL_VERS && \
   CC=$OPENSSL_CC \
   AR=$OPENSSL_AR \
   $SETARCH ./Configure --prefix=$install no-dso $OPENSSL_OS $OPENSSL_CFLAGS -fPIC && \
   make -j4 && \
   make install)

   mv $install $final_install_path
   install=$final_install_path
fi

# Variables to the openssl-sys crate to link statically against the OpenSSL we
# just compiled above
export OPENSSL_STATIC=1
export OPENSSL_DIR=$install

# ==============================================================================
# Actually delgate to the test script itself
# ==============================================================================

# Our only writable directory is `target`, so place all output there and go
# ahead and throw the home directory in there as well.
export CARGO_TARGET_DIR=`pwd`/target
export CARGO_HOME=`pwd`/target/cargo-home
export CARGO_TARGET_${upper_target}_LINKER=$OPENSSL_CC

exec sh ci/run.sh
