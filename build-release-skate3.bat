@echo off
rem Builds the optimized launcher with the skate3 feature and its two viewers.
setlocal
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
pushd "%~dp0" || exit /b 1
powershell -NoProfile -ExecutionPolicy Bypass -File tools\Build-Launcher.ps1 -Skate3 %*
set "BUILD_EXIT_CODE=%ERRORLEVEL%"
popd
echo.
echo Build finished with exit code %BUILD_EXIT_CODE%.
pause
exit /b %BUILD_EXIT_CODE%
