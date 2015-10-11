@echo off

echo "Running CLI tests..."

set MR="%~dp0\..\target\release\multirust-rs.exe"
set HMR="%USERPROFILE%\.multirust\bin\multirust.exe"

echo "Testing --help"
%MR% --help || exit /b 1

echo "Testing install"
%MR% install -a || exit /b 1

echo "Testing default"
%HMR% default nightly || exit /b 1

echo "Testing override"
%HMR% override i686-msvc-stable || exit /b 1

echo "Testing update"
%HMR% update || exit /b 1

echo "Testing uninstall"
%HMR% uninstall -y || exit /b 1

echo "Finished"
