@echo off
setlocal
pushd "%~dp0"
cargo build %*
if errorlevel 1 exit /b %errorlevel%
if not exist "dist\debug" mkdir "dist\debug"
copy /Y "target\debug\vpnfybot-windows.exe" "dist\debug\@vpnfybot-windows.exe"
if errorlevel 1 exit /b %errorlevel%
echo Portable build ready: dist\debug\@vpnfybot-windows.exe
exit /b %errorlevel%
