#!/bin/sh

set -ex

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

OPENSSL_VERS=1.0.2h
OPENSSL_SHA256=1d4007e53aad94a5b2002fe045ee7bb0b3d98f1a47f8b2bc851dcd1c74332919

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
  *)
    echo "can't cross compile OpenSSL for $TARGET"
    exit 1
    ;;
esac

mkdir -p target/$TARGET/openssl
install=`pwd`/target/$TARGET/openssl/openssl-install
out=`pwd`/target/$TARGET/openssl/openssl-$OPENSSL_VERS.tar.gz
curl -o $out https://openssl.org/source/openssl-$OPENSSL_VERS.tar.gz
sha256sum $out > $out.sha256
test $OPENSSL_SHA256 = `cut -d ' ' -f 1 $out.sha256`

tar xf $out -C target/$TARGET/openssl
(cd target/$TARGET/openssl/openssl-$OPENSSL_VERS && \
 CC=$OPENSSL_CC \
 AR=$OPENSSL_AR \
 $SETARCH ./Configure --prefix=$install no-dso $OPENSSL_OS $OPENSSL_CFLAGS -fPIC && \
 make -j4 && \
 make install)

# Variables to the openssl-sys crate to link statically against the OpenSSL we
# just compiled above
export OPENSSL_STATIC=1
export OPENSSL_ROOT_DIR=$install
export OPENSSL_LIB_DIR=$install/lib
export OPENSSL_INCLUDE_DIR=$install/include

# ==============================================================================
# Actually delgate to the test script itself
# ==============================================================================

# Our only writable directory is `target`, so place all output there and go
# ahead and throw the home directory in there as well.
export CARGO_TARGET_DIR=`pwd`/target
export CARGO_HOME=`pwd`/target/cargo-home
export CARGO_TARGET_${upper_target}_LINKER=$OPENSSL_CC

exec sh ci/run.sh
