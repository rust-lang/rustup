#!/bin/sh
# Copyright 2016 The Rust Project Developers. See the COPYRIGHT
# file at the top-level directory of this distribution and at
# http://rust-lang.org/COPYRIGHT.
#
# Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
# http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
# <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
# option. This file may not be copied, modified, or distributed
# except according to those terms.

# This is just a little script that can be curled from the internet to
# install rustup. It just does platform detection, curls the installer
# and runs it.

set -u

RUSTUP_UPDATE_ROOT="https://static.rust-lang.org/rustup/dist"

main() {
    need_cmd curl
    need_cmd mktemp
    need_cmd chmod
    need_cmd mkdir
    need_cmd rm
    need_cmd rmdir
    need_cmd printf

    get_architecture || return 1
    local _arch="$RETVAL"
    assert_nz "$_arch" "arch"

    local _ext=""
    case "$_arch" in
        *windows*)
            _ext=".exe"
            ;;
    esac

    local _url="$RUSTUP_UPDATE_ROOT/$_arch/rustup-init$_ext"

    local _dir="$(mktemp -d 2>/dev/null || ensure mktemp -d -t rustup)"
    local _file="$_dir/rustup-init$_ext"

    printf "\33[1minfo:\33[0m downloading installer\n"

    ensure mkdir -p "$_dir"
    ensure curl -sSfL "$_url" -o "$_file"
    ensure chmod u+x "$_file"
    if [ ! -x "$_file" ]; then
        echo "Cannot execute $_file (likely because of mounting /tmp as noexec)."
        echo "Please copy the file to a location where you can execute binaries and run ./rustup-init$_ext."
        exit 1
    fi

    # check if we have to use /dev/tty to prompt the user
    local need_tty=yes
    for arg in "$@"; do
        case "$arg" in
            -y)
                # user wants to skip the prompt -- we don't need /dev/tty
                need_tty=no
                ;;
            *)
                ;;
        esac
    done


    if [ "$need_tty" = "yes" ]; then
        # The installer is going to want to ask for confirmation by
        # reading stdin.  This script was piped into `sh` though and
        # doesn't have stdin to pass to its children. Instead we're going
        # to explicitly connect /dev/tty to the installer's stdin.
        if [ ! -e "/dev/tty" ]; then
            err "/dev/tty does not exist"
        fi

        run "$_file" "$@" < /dev/tty
    else
        run "$_file" "$@"
    fi

    local _retval=$?

    ignore rm "$_file"
    ignore rmdir "$_dir"

    return "$_retval"
}

get_architecture() {

    local _ostype="$(uname -s)"
    local _cputype="$(uname -m)"

    if [ "$_ostype" = Darwin -a "$_cputype" = i386 ]; then
        # Darwin `uname -s` lies
        if sysctl hw.optional.x86_64 | grep -q ': 1'; then
            local _cputype=x86_64
        fi
    fi

    case "$_ostype" in

        Linux)
            local _ostype=unknown-linux-gnu
            ;;

        FreeBSD)
            local _ostype=unknown-freebsd
            ;;

        DragonFly)
            local _ostype=unknown-dragonfly
            ;;

        Darwin)
            local _ostype=apple-darwin
            ;;

        MINGW* | MSYS* | CYGWIN*)
            local _ostype=pc-windows-gnu
            ;;

        *)
            err "unrecognized OS type: $_ostype"
            ;;

    esac

    case "$_cputype" in

        i386 | i486 | i686 | i786 | x86)
            local _cputype=i686
            ;;

        xscale | arm)
            local _cputype=arm
            ;;

        armv6l)
            local _cputype=arm
            local _ostype="${_ostype}eabihf"
            ;;

        armv7l)
            local _cputype=armv7
            local _ostype="${_ostype}eabihf"
            ;;

        aarch64)
            local _cputype=aarch64
            ;;

        x86_64 | x86-64 | x64 | amd64)
            local _cputype=x86_64
            ;;

        ppc)
            local _cputype=powerpc
            ;;

        ppc64)
            local _cputype=powerpc64
            ;;

        ppc64le)
            local _cputype=powerpc64le
            ;;

        *)
            err "unknown CPU type: $_cputype"

    esac

    # Detect 64-bit linux with 32-bit userland
    if [ $_ostype = unknown-linux-gnu -a $_cputype = x86_64 ]; then
        need_cmd head
        # Architecture detection without dependencies beyond coreutils.
        # ELF files start out "\x7fELF", and the following byte is
        #   0x01 for 32-bit and
        #   0x02 for 64-bit.
        # The printf builtin on some shells like dash only supports octal
        # escape sequences, so we use those.
        local _current_exe_head=$(head -c 5 /proc/self/exe )
        if [ "$_current_exe_head" = "$(printf '\177ELF\001')" ]; then
            local _cputype=i686
        fi
    fi

    # Detect armv7 but without the CPU features Rust needs in that build,
    # and fall back to arm.
    # See https://github.com/rust-lang-nursery/rustup.rs/issues/587.
    if [ $_ostype = "unknown-linux-gnueabihf" -a $_cputype = armv7 ]; then
        if ensure grep '^Features' /proc/cpuinfo | grep -q -v neon; then
            # At least one processor does not have NEON.
            local _cputype=arm
        fi
    fi

    local _arch="$_cputype-$_ostype"

    RETVAL="$_arch"
}

say() {
    echo "rustup: $1"
}

say_err() {
    say "$1" >&2
}

err() {
    say "$1" >&2
    exit 1
}

need_cmd() {
    if ! command -v "$1" > /dev/null 2>&1
    then err "need '$1' (command not found)"
    fi
}

need_ok() {
    if [ $? != 0 ]; then err "$1"; fi
}

assert_nz() {
    if [ -z "$1" ]; then err "assert_nz $2"; fi
}

# Run a command that should never fail. If the command fails execution
# will immediately terminate with an error showing the failing
# command.
ensure() {
    "$@"
    need_ok "command failed: $*"
}

# This is just for indicating that commands' results are being
# intentionally ignored. Usually, because it's being executed
# as part of error handling.
ignore() {
    run "$@"
}

# Runs a command and prints it to stderr if it fails.
run() {
    "$@"
    local _retval=$?
    if [ $_retval != 0 ]; then
        say_err "command failed: $*"
    fi
    return $_retval
}

main "$@" || exit 1
