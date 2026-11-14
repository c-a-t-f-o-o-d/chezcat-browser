@echo off
setlocal DisableDelayedExpansion
for %%I in ("%~dp0..\..") do set "chezcat_root=%%~fI"
cd /d "%chezcat_root%"
if errorlevel 1 exit /b 1
if not "%~2"=="" goto usage
set "demo_port=%~1"
if not defined demo_port set "demo_port=8766"
if not "%demo_port:~5,1%"=="" goto usage
for /f "delims=0123456789" %%A in ("%demo_port%") do goto usage
set "demo_port_digits=00000%demo_port%"
set "demo_port_digits=%demo_port_digits:~-5%"
set /a demo_port_number=1%demo_port_digits%-100000 >nul 2>nul
if errorlevel 1 goto usage
if %demo_port_number% lss 1 goto usage
if %demo_port_number% gtr 65535 goto usage
if not exist "%chezcat_root%\ChezCat.exe" (
    echo ChezCat.exe was not found. Build it first using Build.cmd.
    exit /b 1
)
if not exist "%chezcat_root%\tools\https-demo\demo-roots.pem" (
    echo The separate demo trust file is missing. Extract the complete ChezCat folder.
    exit /b 1
)
start "ChezCat HTTPS demo" "%chezcat_root%\ChezCat.exe" --trust-file "%chezcat_root%\tools\https-demo\demo-roots.pem" https://localhost:%demo_port_number%/
exit /b %errorlevel%
:usage
echo Usage: Run-Https-Browser.cmd [port]
echo The port must be a number from 1 to 65535. The default is 8766.
exit /b 2
