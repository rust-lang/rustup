#!/bin/sh
set -e

echo "> Running CLI tests..."

MR="`dirname $0`/multirust-rs(2)"

echo "> Renaming to multirust(2)"
cp "`dirname $0`/../target/release/multirust-rs" $MR

echo "> Testing self install"
$MR self install -a

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

echo "> Testing proxying global commands"
multirust proxy echo "Hello from global command"

echo "> Testing running global commands"
multirust run stable echo "Hello from global command"

echo "> Testing doc"
multirust doc

echo "> Testing doc --all"
multirust doc --all

echo "> Testing self uninstall"
multirust self uninstall -y

echo "> Finished"
