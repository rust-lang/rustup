@echo off

echo ^> Running CLI tests...

set MR="%~dp0\multirust-rs(2).exe"

echo ^> Renaming to multirust(2).exe
copy /B /Y "%~dp0\..\target\release\multirust-rs.exe" %MR% || (echo FAILED && exit /b 1)

echo ^> Testing self install
%MR% self install -a || (echo FAILED && exit /b 1)

echo ^> Updating PATH
set PATH=%LOCALAPPDATA%\.multirust\bin;%PATH%

echo ^> Testing default
multirust default nightly || (echo FAILED && exit /b 1)

echo ^> Testing rustc
rustc --multirust || (echo FAILED && exit /b 1)

echo ^> Testing cargo
cargo --multirust || (echo FAILED && exit /b 1)

echo ^> Testing override
multirust override i686-msvc-stable || (echo FAILED && exit /b 1)

echo ^> Testing update
multirust update || (echo FAILED && exit /b 1)

echo ^> Testing proxying global commands
multirust proxy cmd /c echo "Hello from global command" || (echo FAILED && exit /b 1)

echo ^> Testing running global commands
multirust run stable cmd /c echo "Hello from global command" || (echo FAILED && exit /b 1)

echo ^> Testing doc
multirust doc || (echo FAILED && exit /b 1)

echo ^> Testing doc --all
multirust doc --all || (echo FAILED && exit /b 1)

echo ^> Testing self uninstall
multirust self uninstall -y || (echo FAILED && exit /b 1)

echo ^> Finished
