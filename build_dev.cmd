@echo off
setlocal DisableDelayedExpansion
pushd "%~dp0"
if errorlevel 1 exit /b 1

rem Keep development artifacts separate from the normal release cache.
set "CHEZCAT_DEV_BASE_TARGET="
for /f "delims=" %%I in ('python -B tools\capture_tests.py --print-target-dir') do set "CHEZCAT_DEV_BASE_TARGET=%%I"
if not defined CHEZCAT_DEV_BASE_TARGET (
    popd
    exit /b 1
)
set "CARGO_TARGET_DIR=%CHEZCAT_DEV_BASE_TARGET%\build-dev"
set "CARGO_PROFILE_RELEASE_LTO=off"
set "CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16"
set "CARGO_INCREMENTAL=1"

call Build.cmd
set "CHEZCAT_DEV_EXIT_CODE=%ERRORLEVEL%"
popd
endlocal & exit /b %CHEZCAT_DEV_EXIT_CODE%
