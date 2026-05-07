@echo off
setlocal
pushd "%~dp0"
cargo build --release %*
if errorlevel 1 exit /b %errorlevel%
if not exist "dist\portable" mkdir "dist\portable"
copy /Y "target\release\vpnfybot-windows.exe" "dist\portable\vpnfybot-windows.exe"
if errorlevel 1 exit /b %errorlevel%
for %%F in ("dist\portable\vpnfybot-windows.exe") do set BUILD_SIZE=%%~zF
echo Portable build size: %BUILD_SIZE% bytes
echo Portable build ready: dist\portable\vpnfybot-windows.exe
exit /b %errorlevel%
