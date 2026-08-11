#!/bin/sh

configure=$1
shift
# `openssl-src` has no API for extra Configure arguments.
exec perl "$configure" no-dtls no-sm2 no-sm3 no-sm4 no-srtp "$@"
