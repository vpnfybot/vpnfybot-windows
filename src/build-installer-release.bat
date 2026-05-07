@echo off
setlocal
pushd "%~dp0"

for /f "tokens=3" %%V in ('findstr /b "version =" Cargo.toml') do set "PRODUCT_VERSION=%%~V"
if not defined PRODUCT_VERSION set "PRODUCT_VERSION=4.0.7"

set "EXIT_CODE=0"

cargo build --release %*
if errorlevel 1 exit /b %errorlevel%

set "DIST_DIR=%CD%\dist"
set "OUTPUT_FILE=%DIST_DIR%\vpnfybot-windows.exe"
set "BUILD_OUTPUT_FILE=%TEMP%\vpnfybot-windows-installer-build-%RANDOM%%RANDOM%.exe"
set "PAYLOAD_DIR=%TEMP%\vpnfybot-windows-installer-payload-%RANDOM%%RANDOM%"
set "LEGACY_PAYLOAD_DIR=%DIST_DIR%\installer-payload"

if not exist "%DIST_DIR%" mkdir "%DIST_DIR%"
if exist "%BUILD_OUTPUT_FILE%" del /Q "%BUILD_OUTPUT_FILE%"
if exist "%LEGACY_PAYLOAD_DIR%" rmdir /S /Q "%LEGACY_PAYLOAD_DIR%"
if exist "%PAYLOAD_DIR%" rmdir /S /Q "%PAYLOAD_DIR%"
mkdir "%PAYLOAD_DIR%\deps\vpnfybot-windows-%PRODUCT_VERSION%"
if errorlevel 1 exit /b %errorlevel%

copy /Y "target\release\vpnfybot-windows.exe" "%PAYLOAD_DIR%\vpnfybot-windows.exe"
if errorlevel 1 exit /b %errorlevel%
copy /Y "vpnfy.ico" "%PAYLOAD_DIR%\vpnfy.ico"
if errorlevel 1 exit /b %errorlevel%

for %%F in (ProxyBridgeCore.dll ProxyBridge_CLI.exe WinDivert.dll WinDivert64.sys wireproxy.exe) do (
  copy /Y "embedded_deps\%%F" "%PAYLOAD_DIR%\deps\vpnfybot-windows-%PRODUCT_VERSION%\%%F"
  if errorlevel 1 exit /b %errorlevel%
)

set "MAKENSIS="
where makensis >nul 2>nul
if not errorlevel 1 set "MAKENSIS=makensis"
if not defined MAKENSIS if exist "%ProgramFiles(x86)%\NSIS\makensis.exe" set "MAKENSIS=%ProgramFiles(x86)%\NSIS\makensis.exe"
if not defined MAKENSIS if exist "%ProgramFiles%\NSIS\makensis.exe" set "MAKENSIS=%ProgramFiles%\NSIS\makensis.exe"

if not defined MAKENSIS (
  echo NSIS not found. Install NSIS or add makensis.exe to PATH.
  exit /b 1
)

"%MAKENSIS%" "/DPRODUCT_VERSION=%PRODUCT_VERSION%" "/DPAYLOAD_DIR=%PAYLOAD_DIR%" "/DOUTFILE=%BUILD_OUTPUT_FILE%" "installer\vpnfybot-windows.nsi"
if errorlevel 1 set "EXIT_CODE=%errorlevel%"

if exist "%PAYLOAD_DIR%" rmdir /S /Q "%PAYLOAD_DIR%"
if not "%EXIT_CODE%"=="0" (
  if exist "%BUILD_OUTPUT_FILE%" del /Q "%BUILD_OUTPUT_FILE%"
  exit /b %EXIT_CODE%
)

del /Q "%DIST_DIR%\vpnfybot-windows-*.exe" 2>nul
move /Y "%BUILD_OUTPUT_FILE%" "%OUTPUT_FILE%" >nul
if errorlevel 1 exit /b %errorlevel%

echo Installer ready: %OUTPUT_FILE%
exit /b 0