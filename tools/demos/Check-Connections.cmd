@echo off
setlocal DisableDelayedExpansion
for %%I in ("%~dp0..\..") do set "chezcat_root=%%~fI"
cd /d "%chezcat_root%"
if errorlevel 1 exit /b 1
if not exist "%chezcat_root%\ChezCat.exe" (
    echo ChezCat.exe is missing. Extract the complete archive first.
    pause
    exit /b 1
)
if not exist "results" mkdir "results"
if not exist "results" exit /b 1
echo Checking document connections with ChezCat.
echo This does not submit forms or start fingerprint or port scans.
echo Results will replace results\connection-report.txt.
start "ChezCat connection checks" /wait "%chezcat_root%\ChezCat.exe" --diagnose https://www.google.com/ --diagnose https://coveryourtracks.eff.org/fingerprint-nojs --diagnose https://www.grc.com/shieldsup --report "%chezcat_root%\results\connection-report.txt"
if errorlevel 1 (
    echo The connection checker could not complete. Check that the folder is writable.
    pause
    exit /b 1
)
echo Report written. Open results\connection-report.txt to see which checks succeeded or failed.
start "" notepad.exe "%chezcat_root%\results\connection-report.txt"
