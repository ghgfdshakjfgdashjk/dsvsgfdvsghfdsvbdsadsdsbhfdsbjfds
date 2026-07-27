@echo off
REM boots autoclicker - dev launcher
REM Double-click this to build and run. Close the window to stop.

cd /d "%~dp0"

if not exist "node_modules" (
    echo Installing npm packages, one moment...
    call npm install
    if errorlevel 1 (
        echo.
        echo npm install failed. Is Node.js installed?
        pause
        exit /b 1
    )
)

echo Starting boots autoclicker...
echo.
call npm run tauri dev

REM Keep the window open if something went wrong, so the error stays readable.
if errorlevel 1 (
    echo.
    echo --- Build failed. Copy the errors above. ---
    pause
)
