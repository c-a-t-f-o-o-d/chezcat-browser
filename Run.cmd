@echo off
setlocal
cd /d "%~dp0"
if not exist "ChezCat.exe" (
    call Build.cmd
    if errorlevel 1 (
        pause
        exit /b 1
    )
)
start "" "%~dp0ChezCat.exe" "%~dp0pages\start.html"
endlocal

