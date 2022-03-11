#!/bin/bash
#
# This file is intended to be sourced, so it is not being marked as
# an executable file in git.

# This is modified from
# https://github.com/travis-ci/travis-build/blob/master/lib/travis/build/bash/travis_setup_env.bash
export ANSI_RED='\033[31;1m'
export ANSI_GREEN='\033[32;1m'
export ANSI_YELLOW='\033[33;1m'
export ANSI_RESET='\033[0m'
export ANSI_CLEAR='\033[0K'

# This is modified loop version of
# https://github.com/travis-ci/travis-build/blob/master/lib/travis/build/bash/travis_retry.bash
command_retry() {
  local result=0
  local count=1
  local max=5
  while [ "${count}" -le "${max}" ]; do
    [ "${result}" -ne 0 ] && {
      printf "${ANSI_RED}"'The command "%s" failed. Retrying, %s of %s.'"${ANSI_RESET}"'\n' "${*}" "${count}" "${max}" >&2
    }
    "${@}" && { result=0 && break; } || result="${?}"
    : $((count=count+1))
    sleep 1
  done

  [ "${count}" -gt "${max}" ] && {
    printf "${ANSI_RED}"'The command "%s" failed %s times.'"${ANSI_RESET}"'\n' "${*}" "${max}" >&2
  }

  return "${result}"
}

# Set the features flag differently from the target.
set_features_by_target() {
  FEATURES=('--no-default-features' '--features' 'curl-backend,reqwest-backend,reqwest-default-tls')
  case "$(uname -s)" in
    *NT* ) ;; # Windows NT
    * ) FEATURES+=('--features' 'vendored-openssl') ;;
  esac

  case "$1" in
    # these platforms aren't supported by ring:
    powerpc* ) ;;
    mips* ) ;;
    riscv* ) ;;
    s390x* ) ;;
    aarch64-pc-windows-msvc ) ;;
    # default case, build with rustls enabled
    * ) FEATURES+=('--features' 'reqwest-rustls-tls') ;;
  esac
}

# Follow the target to determine if CFLAGS is exported.
try_export_CFLAGS_by_target() {
  # rustc only supports armv7: https://doc.rust-lang.org/nightly/rustc/platform-support.html
  if [ "$1" = arm-linux-androideabi ]; then
    export CFLAGS='-march=armv7'
  fi
}
