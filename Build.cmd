@echo off
setlocal DisableDelayedExpansion
cd /d "%~dp0"
where rustup >nul 2>nul
if errorlevel 1 (
    echo Install the pinned Rust toolchain and Windows C++ build tools.
    exit /b 1
)
python tools\accept_license.py --check
if errorlevel 1 exit /b 1
set "CHEZCAT_EFFECTIVE_TARGET="
for /f "delims=" %%I in ('python -B tools\capture_tests.py --print-target-dir') do set "CHEZCAT_EFFECTIVE_TARGET=%%I"
if not defined CHEZCAT_EFFECTIVE_TARGET exit /b 1
rustup run 1.99.1 cargo build --release --offline --locked --target x86_64-pc-windows-msvc --target-dir "%CHEZCAT_EFFECTIVE_TARGET%"
if errorlevel 1 exit /b 1
copy /y "%CHEZCAT_EFFECTIVE_TARGET%\x86_64-pc-windows-msvc\release\chezcat.exe" "ChezCat.exe" >nul
if errorlevel 1 exit /b 1
echo Built ChezCat.exe
endlocal
