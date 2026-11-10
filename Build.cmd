@echo off
setlocal
cd /d "%~dp0"
where cargo >nul 2>nul
if errorlevel 1 (
    echo Rust is required to build the source.
    echo Install it from https://rustup.rs/ and reopen this terminal.
    echo On Windows, also install the C++ build tools requested by Rust.
    exit /b 1
)
cargo build --release --offline --locked
if errorlevel 1 exit /b 1
copy /y "target\release\own-browser.exe" "ChezCat.exe" >nul
if errorlevel 1 exit /b 1
echo Built ChezCat.exe
endlocal

