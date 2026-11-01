@echo off
setlocal
pushd "%~dp0.." || exit /b 1
set "CARGO_EXE=cargo"
set "TOOLCHAIN=+stable"
if not exist ".tools\cargo\bin\cargo.exe" goto choose_action
set "CARGO_HOME=%CD%\.tools\cargo"
set "RUSTUP_HOME=%CD%\.tools\rustup"
set "CARGO_EXE=%CARGO_HOME%\bin\cargo.exe"
set "TOOLCHAIN=+stable-x86_64-pc-windows-gnu"
set "LINKER_BIN=%RUSTUP_HOME%\toolchains\stable-x86_64-pc-windows-gnu\lib\rustlib\x86_64-pc-windows-gnu\bin"
set "LLVM_BIN=%CD%\.tools\llvm-mingw\llvm-mingw-20261020-ucrt-x86_64\bin"
if not exist "%LLVM_BIN%\ld.lld.exe" goto missing_tools
if not exist "%LLVM_BIN%\llvm-dlltool.exe" goto missing_tools
set "PATH=%LLVM_BIN%;%LINKER_BIN%\self-contained;%LINKER_BIN%;%CARGO_HOME%\bin;%PATH%"

:choose_action
set "CARGO_ARGS="
if "%~1"=="" set "CARGO_ARGS=run --bin tesserun"
if /I "%~1"=="run" set "CARGO_ARGS=run --bin tesserun"
if /I "%~1"=="server" set "CARGO_ARGS=run --no-default-features --bin tesserun-server"
if /I "%~1"=="check" set "CARGO_ARGS=check --all-targets"
if /I "%~1"=="test" set "CARGO_ARGS=test --all-targets"
if /I "%~1"=="build" set "CARGO_ARGS=build --release"
if /I "%~1"=="lint" set "CARGO_ARGS=clippy --all-targets -- -D warnings"
if /I "%~1"=="format" set "CARGO_ARGS=fmt --all"
if not defined CARGO_ARGS goto usage
"%CARGO_EXE%" %TOOLCHAIN% %CARGO_ARGS%
set "RESULT=%ERRORLEVEL%"
popd
exit /b %RESULT%

:missing_tools
echo Missing project-local LLVM-MinGW tools. Restore .tools\llvm-mingw before building. 1>&2
popd
exit /b 1

:usage
echo Usage: rust.cmd [run^|server^|check^|test^|build^|lint^|format] 1>&2
popd
exit /b 2
