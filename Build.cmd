@echo off
setlocal
cd /d "%~dp0"
where rustup >nul 2>nul
if errorlevel 1 (
    echo Install the pinned Rust toolchain and Windows C++ build tools.
    exit /b 1
)
python tools\accept_license.py --check
if errorlevel 1 exit /b 1
rustup run 1.99.1 cargo build --release --offline --locked --target x86_64-pc-windows-msvc --target-dir target
if errorlevel 1 exit /b 1
copy /y "target\x86_64-pc-windows-msvc\release\chezcat.exe" "ChezCat.exe" >nul
if errorlevel 1 exit /b 1
echo Built ChezCat.exe
endlocal
