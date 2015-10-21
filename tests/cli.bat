@echo off

echo ^> Running CLI tests...

set MR="%~dp0\..\target\release\multirust-rs.exe"

echo ^> Testing --help
%MR% --help || (echo FAILED && exit /b 1)

echo ^> Testing install
%MR% install -a || (echo FAILED && exit /b 1)

echo ^> Updating PATH
set PATH=%LOCALAPPDATA%\.multirust\bin;%PATH%

echo ^> Testing default
multirust default nightly || (echo FAILED && exit /b 1)

echo ^> Testing rustc
call rustc --multirust || (echo FAILED && exit /b 1)

echo ^> Testing cargo
call cargo --multirust || (echo FAILED && exit /b 1)

echo ^> Testing override
multirust override i686-msvc-stable || (echo FAILED && exit /b 1)

echo ^> Testing update
multirust update || (echo FAILED && exit /b 1)

echo ^> Testing uninstall
multirust uninstall -y || (echo FAILED && exit /b 1)

echo ^> Finished
