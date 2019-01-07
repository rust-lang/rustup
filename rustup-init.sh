#!/bin/bash
# Copyright 2016 The Rust Project Developers. See the COPYRIGHT
# file at the top-level directory of this distribution and at
# http://rust-lang.org/COPYRIGHT.
#
# Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
# http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
# <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
# option. This file may not be copied, modified, or distributed
# except according to those terms.

# This is just a little script that can be downloaded from the internet to
# install rustup. It just does platform detection, downloads the installer
# and runs it.

set -u

if [ -z "$RUSTUP_UPDATE_ROOT" ]; then
    RUSTUP_UPDATE_ROOT="https://static.rust-lang.org/rustup"
fi

#XXX: If you change anything here, please make the same changes in setup_mode.rs
usage() {
    cat 1>&2 <<EOF
rustup-init 1.0.0 (408ed84 2017-02-11)
The installer for rustup

USAGE:
    rustup-init [FLAGS] [OPTIONS]

FLAGS:
    -v, --verbose           Enable verbose output
    -y                      Disable confirmation prompt.
        --no-modify-path    Don't configure the PATH environment variable
    -h, --help              Prints help information
    -V, --version           Prints version information

OPTIONS:
        --default-host <default-host>              Choose a default host triple
        --default-toolchain <default-toolchain>    Choose a default toolchain to install
        --default-toolchain none                   Do not install any toolchains
EOF
}

main() {
    downloader --check
    need_cmd uname
    need_cmd mktemp
    need_cmd chmod
    need_cmd mkdir
    need_cmd rm
    need_cmd rmdir

    get_architecture || return 1
    local _arch="$RETVAL"
    assert_nz "$_arch" "arch"

    local _ext=""
    case "$_arch" in
        *windows*)
            _ext=".exe"
            ;;
    esac

    local _url="${RUSTUP_UPDATE_ROOT}/dist/${_arch}/rustup-init${_ext}"

    local _dir
    _dir="$(mktemp -d 2>/dev/null || ensure mktemp -d -t rustup)"
    local _file="${_dir}/rustup-init${_ext}"

    local _ansi_escapes_are_valid=false
    if [ -t 2 ]; then
        if [ "${TERM+set}" = 'set' ]; then
            case "$TERM" in
                xterm*|rxvt*|urxvt*|linux*|vt*)
                    _ansi_escapes_are_valid=true
                ;;
            esac
        fi
    fi

    # check if we have to use /dev/tty to prompt the user
    local need_tty=yes
    for arg in "$@"; do
        case "$arg" in
            -h|--help)
                usage
                exit 0
                ;;
            -y)
                # user wants to skip the prompt -- we don't need /dev/tty
                need_tty=no
                ;;
            *)
                ;;
        esac
    done

    if $_ansi_escapes_are_valid; then
        printf "\33[1minfo:\33[0m downloading installer\n" 1>&2
    else
        printf '%s\n' 'info: downloading installer' 1>&2
    fi

    ensure mkdir -p "$_dir"
    ensure downloader "$_url" "$_file"
    ensure chmod u+x "$_file"
    if [ ! -x "$_file" ]; then
        printf '%s\n' "Cannot execute $_file (likely because of mounting /tmp as noexec)." 1>&2
        printf '%s\n' "Please copy the file to a location where you can execute binaries and run ./rustup-init${_ext}." 1>&2
        exit 1
    fi



    if [ "$need_tty" = "yes" ]; then
        # The installer is going to want to ask for confirmation by
        # reading stdin.  This script was piped into `sh` though and
        # doesn't have stdin to pass to its children. Instead we're going
        # to explicitly connect /dev/tty to the installer's stdin.
        if [ ! -t 1 ]; then
            err "Unable to run interactively. Run with -y to accept defaults, --help for additional options"
        fi

        ignore "$_file" "$@" < /dev/tty
    else
        ignore "$_file" "$@"
    fi

    local _retval=$?

    ignore rm "$_file"
    ignore rmdir "$_dir"

    return "$_retval"
}

get_bitness() {
    need_cmd head
    # Architecture detection without dependencies beyond coreutils.
    # ELF files start out "\x7fELF", and the following byte is
    #   0x01 for 32-bit and
    #   0x02 for 64-bit.
    # The printf builtin on some shells like dash only supports octal
    # escape sequences, so we use those.
    local _current_exe_head
    _current_exe_head=$(head -c 5 /proc/self/exe )
    if [ "$_current_exe_head" = "$(printf '\177ELF\001')" ]; then
        echo 32
    elif [ "$_current_exe_head" = "$(printf '\177ELF\002')" ]; then
        echo 64
    else
        err "unknown platform bitness"
    fi
}

get_endianness() {
    local cputype=$1
    local suffix_eb=$2
    local suffix_el=$3

    # detect endianness without od/hexdump, like get_bitness() does.
    need_cmd head
    need_cmd tail

    local _current_exe_endianness
    _current_exe_endianness="$(head -c 6 /proc/self/exe | tail -c 1)"
    if [ "$_current_exe_endianness" = "$(printf '\001')" ]; then
        echo "${cputype}${suffix_el}"
    elif [ "$_current_exe_endianness" = "$(printf '\002')" ]; then
        echo "${cputype}${suffix_eb}"
    else
        err "unknown platform endianness"
    fi
}

get_architecture() {
    local _ostype _cputype
    _ostype="$(uname -s)"
    _cputype="$(uname -m)"

    if [ "$_ostype" = Linux ]; then
        if [ "$(uname -o)" = Android ]; then
            _ostype=Android
        fi
    fi

    if [ "$_ostype" = Darwin ] && [ "$_cputype" = i386 ]; then
        # Darwin `uname -s` lies
        if sysctl hw.optional.x86_64 | grep -q ': 1'; then
            _cputype=x86_64
        fi
    fi

    case "$_ostype" in

        Android)
            _ostype=linux-android
            ;;

        Linux)
            _ostype=unknown-linux-gnu
            ;;

        FreeBSD)
            _ostype=unknown-freebsd
            ;;

        NetBSD)
            _ostype=unknown-netbsd
            ;;

        DragonFly)
            _ostype=unknown-dragonfly
            ;;

        Darwin)
            _ostype=apple-darwin
            ;;

        MINGW* | MSYS* | CYGWIN*)
            _ostype=pc-windows-gnu
            ;;

        *)
            err "unrecognized OS type: $_ostype"
            ;;

    esac

    case "$_cputype" in

        i386 | i486 | i686 | i786 | x86)
            _cputype=i686
            ;;

        xscale | arm)
            _cputype=arm
            if [ "$_ostype" = "linux-android" ]; then
                _ostype=linux-androideabi
            fi
            ;;

        armv6l)
            _cputype=arm
            if [ "$_ostype" = "linux-android" ]; then
                _ostype=linux-androideabi
            else
                _ostype="${_ostype}eabihf"
            fi
            ;;

        armv7l | armv8l)
            _cputype=armv7
            if [ "$_ostype" = "linux-android" ]; then
                _ostype=linux-androideabi
            else
                _ostype="${_ostype}eabihf"
            fi
            ;;

        aarch64)
            _cputype=aarch64
            ;;

        x86_64 | x86-64 | x64 | amd64)
            _cputype=x86_64
            ;;

        mips)
            _cputype="$(get_endianness $_cputype "" 'el')"
            ;;

        mips64)
            _bitness="$(get_bitness)"
            if [ "$_bitness" -eq 32 ]; then
                if [ "$_ostype" = "unknown-linux-gnu" ]; then
                    # 64-bit kernel with 32-bit userland
                    # endianness suffix is appended later
                    _cputype=mips
                fi
            else
                # only n64 ABI is supported for now
                _ostype="${_ostype}abi64"
            fi

            _cputype="$(get_endianness "$_cputype" "" 'el')"
            ;;

        ppc)
            _cputype=powerpc
            ;;

        ppc64)
            _cputype=powerpc64
            ;;

        ppc64le)
            _cputype=powerpc64le
            ;;

        *)
            err "unknown CPU type: $_cputype"

    esac

    # Detect 64-bit linux with 32-bit userland for x86
    if [ "$_ostype" = unknown-linux-gnu ] && [ "$_cputype" = x86_64 ]; then
        if [ "$(get_bitness)" = "32" ]; then
            _cputype=i686
        fi
    fi

    # Detect 64-bit linux with 32-bit userland for powerpc
    if [ $_ostype = unknown-linux-gnu ] && [ $_cputype = powerpc64 ]; then
        if [ "$(get_bitness)" = "32" ]; then
            local _cputype=powerpc
        fi
    fi

    # Detect armv7 but without the CPU features Rust needs in that build,
    # and fall back to arm.
    # See https://github.com/rust-lang/rustup.rs/issues/587.
    if [ "$_ostype" = "unknown-linux-gnueabihf" ] && [ "$_cputype" = armv7 ]; then
        if ensure grep '^Features' /proc/cpuinfo | grep -q -v neon; then
            # At least one processor does not have NEON.
            _cputype=arm
        fi
    fi

    _arch="${_cputype}-${_ostype}"

    RETVAL="$_arch"
}

say() {
    printf 'rustup: %s\n' "$1"
}

err() {
    say "$1" >&2
    exit 1
}

need_cmd() {
    if ! check_cmd "$1"; then
        err "need '$1' (command not found)"
    fi
}

check_cmd() {
    command -v "$1" > /dev/null 2>&1
}

need_ok() {
    if [ $? -ne 0 ]; then err "$1"; fi
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
    "$@"
}

# This wraps curl or wget. Try curl first, if not installed,
# use wget instead.
downloader() {
    if check_cmd curl; then
        _dld=curl
    elif check_cmd wget; then
        _dld=wget
    else
        _dld='curl or wget' # to be used in error message of need_cmd
    fi

    if [ "$1" = --check ]; then
        need_cmd "$_dld"
    elif [ "$_dld" = curl ]; then
        curl -sSfL "$1" -o "$2"
    elif [ "$_dld" = wget ]; then
        wget "$1" -O "$2"
    else
        err "Unknown downloader"   # should not reach here
    fi
}

main "$@" || exit 1
