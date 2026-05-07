@echo off
setlocal
pushd "%~dp0"
cargo build %*
if errorlevel 1 exit /b %errorlevel%
if not exist "dist\debug\portable" mkdir "dist\debug\portable"
copy /Y "target\debug\vpnfybot-windows.exe" "dist\debug\portable\vpnfybot-windows.exe"
if errorlevel 1 exit /b %errorlevel%
echo Portable build ready: dist\debug\portable\vpnfybot-windows.exe
exit /b %errorlevel%
