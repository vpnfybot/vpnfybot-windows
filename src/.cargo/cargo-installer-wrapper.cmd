@echo off
set "VPNFY_RUSTC_COMMAND=%*"
powershell.exe -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "%~dp0cargo-installer-wrapper.ps1"
exit /b %errorlevel%
