@echo off

echo ^> Running CLI tests...

set MR="%~dp0\..\target\release\multirust-rs.exe"

echo ^> Testing --help
%MR% --help || exit /b 1

echo ^> Testing install
%MR% install -a || exit /b 1

echo ^> Updating PATH
set PATH=%USERPROFILE%\.multirust\bin;%PATH%

echo ^> Testing default
multirust default nightly || exit /b 1

echo ^> Testing rustc
rustc --multirust || exit /b 1

echo ^> Testing cargo
cargo --multirust || exit /b 1

echo ^> Testing override
multirust override i686-msvc-stable || exit /b 1

echo ^> Testing update
multirust update || exit /b 1

echo ^> Testing uninstall
multirust uninstall -y || exit /b 1

echo ^> Finished
