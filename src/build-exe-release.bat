@echo off
setlocal
pushd "%~dp0"
cargo build --release %*
if errorlevel 1 exit /b %errorlevel%
if not exist "dist" mkdir "dist"
copy /Y "target\release\vpnfybot-windows.exe" "dist\@vpnfybot-windows.exe"
if errorlevel 1 exit /b %errorlevel%
for %%F in ("dist\@vpnfybot-windows.exe") do set BUILD_SIZE=%%~zF
echo Portable build size: %BUILD_SIZE% bytes
echo Portable build ready: dist\@vpnfybot-windows.exe
exit /b %errorlevel%
