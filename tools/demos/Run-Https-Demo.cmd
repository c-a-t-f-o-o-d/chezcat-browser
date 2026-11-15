@echo off
setlocal DisableDelayedExpansion
for %%I in ("%~dp0..\..") do set "chezcat_root=%%~fI"
cd /d "%chezcat_root%"
if errorlevel 1 exit /b 1
where py >nul 2>nul
if errorlevel 1 goto python
py -3 "%chezcat_root%\tools\https_fixture_server.py" --open-browser %*
exit /b %errorlevel%
:python
where python >nul 2>nul
if errorlevel 1 (
    echo Python 3 is needed only to run this optional local test server.
    echo ChezCat itself does not use Python.
    echo Install Python 3 or use an existing Python installation.
    pause
    exit /b 1
)
python "%chezcat_root%\tools\https_fixture_server.py" --open-browser %*
exit /b %errorlevel%
