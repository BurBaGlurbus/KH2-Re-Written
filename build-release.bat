@echo off
rem Builds the optimized launcher and its two viewers.
setlocal
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
pushd "%~dp0" || exit /b 1
powershell -NoProfile -ExecutionPolicy Bypass -File tools\Build-Launcher.ps1 %*
set "BUILD_EXIT_CODE=%ERRORLEVEL%"
popd
echo.
echo Build finished with exit code %BUILD_EXIT_CODE%.
pause
exit /b %BUILD_EXIT_CODE%
