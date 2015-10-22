#!/bin/sh
set -e

echo "> Running CLI tests..."

MR="`dirname $0`/multirust-rs(2)"

echo "> Renaming to multirust(2)"
cp "`dirname $0`/../target/release/multirust-rs" $MR

echo "> Testing --help"
$MR --help

echo "> Testing install"
$MR install -a

echo "> Updating PATH"
. ~/.profile

echo "> Testing default"
multirust default nightly

echo "> Testing rustc"
rustc --multirust

echo "> Testing cargo"
cargo --multirust

echo "> Testing override"
multirust override i686-pc-windows-msvc-stable

echo "> Testing update"
multirust update

echo "> Testing uninstall"
multirust uninstall -y

echo "> Finished"
