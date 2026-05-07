!ifndef PRODUCT_VERSION
!define PRODUCT_VERSION "4.0.7"
!endif

!ifndef PAYLOAD_DIR
!define PAYLOAD_DIR "..\dist\installer-payload"
!endif

!ifndef OUTFILE
!define OUTFILE "..\dist\vpnfybot-windows.exe"
!endif

!define PRODUCT_NAME "vpnfybot-windows"
!define PRODUCT_DISPLAY_NAME "vpnfybot-windows"
!define PRODUCT_PUBLISHER "vpnfybot"
!define PRODUCT_WEB_SITE "https://github.com/vpnfybot/vpnfybot-windows"
!define PRODUCT_EXE "vpnfybot-windows.exe"
!define PRODUCT_DIR "vpnfybot-windows"
!define PRODUCT_DEPS_DIR "vpnfybot-windows-${PRODUCT_VERSION}"
!define PRODUCT_UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\vpnfybot-windows"
!define FIREWALL_RULE_WIREPROXY "vpnfybot-windows - wireproxy (incoming)"
!define FIREWALL_RULE_PROXYBRIDGE "vpnfybot-windows - ProxyBridge (incoming)"
!define FIREWALL_RULE_DESCRIPTION "vpnfybot-windows firewall rule"
!define DEPS_UNLOCK_RETRY_COUNT 20
!define DEPS_UNLOCK_RETRY_DELAY_MS 500

Unicode True
RequestExecutionLevel admin
SetCompressor /SOLID lzma
SetCompressorDictSize 64

Name "${PRODUCT_DISPLAY_NAME} ${PRODUCT_VERSION}"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Programs\${PRODUCT_DIR}"
InstallDirRegKey HKCU "${PRODUCT_UNINST_KEY}" "InstallLocation"
BrandingText "${PRODUCT_DISPLAY_NAME} installer"

VIProductVersion "4.0.7.0"
VIAddVersionKey "ProductName" "${PRODUCT_DISPLAY_NAME}"
VIAddVersionKey "ProductVersion" "${PRODUCT_VERSION}"
VIAddVersionKey "CompanyName" "${PRODUCT_PUBLISHER}"
VIAddVersionKey "LegalCopyright" "Copyright (c) 2026 ${PRODUCT_PUBLISHER}"
VIAddVersionKey "FileDescription" "${PRODUCT_DISPLAY_NAME} setup"
VIAddVersionKey "FileVersion" "${PRODUCT_VERSION}"

!include "MUI2.nsh"

!define MUI_ABORTWARNING
!define MUI_ICON "..\vpnfy.ico"
!define MUI_UNICON "..\vpnfy.ico"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "..\..\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "Russian"

!macro RunHidden COMMAND
  nsExec::Exec ${COMMAND}
  Pop $0
!macroend

Function StopTunnelProcesses
  DetailPrint "Stopping running tunnel components..."
  !insertmacro RunHidden '"$SYSDIR\taskkill.exe" /IM "vpnfybot-windows.exe" /F /T'
  !insertmacro RunHidden '"$SYSDIR\taskkill.exe" /IM "wireproxy.exe" /F /T'
  !insertmacro RunHidden '"$SYSDIR\taskkill.exe" /IM "ProxyBridge_CLI.exe" /F /T'
  !insertmacro RunHidden '"$SYSDIR\sc.exe" stop "WinDivert"'
  !insertmacro RunHidden '"$SYSDIR\sc.exe" delete "WinDivert"'
  Sleep 1000
FunctionEnd

Function un.StopTunnelProcesses
  DetailPrint "Stopping running tunnel components..."
  !insertmacro RunHidden '"$SYSDIR\taskkill.exe" /IM "vpnfybot-windows.exe" /F /T'
  !insertmacro RunHidden '"$SYSDIR\taskkill.exe" /IM "wireproxy.exe" /F /T'
  !insertmacro RunHidden '"$SYSDIR\taskkill.exe" /IM "ProxyBridge_CLI.exe" /F /T'
  !insertmacro RunHidden '"$SYSDIR\sc.exe" stop "WinDivert"'
  !insertmacro RunHidden '"$SYSDIR\sc.exe" delete "WinDivert"'
  Sleep 1000
FunctionEnd

Function un.RemoveInstalledDepsWithRetry
  StrCpy $1 0

un_remove_installed_deps_retry:
  Delete "$INSTDIR\deps\${PRODUCT_DEPS_DIR}\ProxyBridgeCore.dll"
  Delete "$INSTDIR\deps\${PRODUCT_DEPS_DIR}\ProxyBridge_CLI.exe"
  Delete "$INSTDIR\deps\${PRODUCT_DEPS_DIR}\WinDivert.dll"
  Delete "$INSTDIR\deps\${PRODUCT_DEPS_DIR}\WinDivert64.sys"
  Delete "$INSTDIR\deps\${PRODUCT_DEPS_DIR}\wireproxy.exe"
  RMDir /r "$INSTDIR\deps\${PRODUCT_DEPS_DIR}"

  IfFileExists "$INSTDIR\deps\${PRODUCT_DEPS_DIR}\WinDivert64.sys" un_remove_installed_deps_wait 0
  Return

un_remove_installed_deps_wait:
  IntOp $1 $1 + 1
  IntCmp $1 ${DEPS_UNLOCK_RETRY_COUNT} un_remove_installed_deps_failed un_remove_installed_deps_sleep un_remove_installed_deps_failed

un_remove_installed_deps_sleep:
  Sleep ${DEPS_UNLOCK_RETRY_DELAY_MS}
  Goto un_remove_installed_deps_retry

un_remove_installed_deps_failed:
  Delete /REBOOTOK "$INSTDIR\deps\${PRODUCT_DEPS_DIR}\ProxyBridgeCore.dll"
  Delete /REBOOTOK "$INSTDIR\deps\${PRODUCT_DEPS_DIR}\ProxyBridge_CLI.exe"
  Delete /REBOOTOK "$INSTDIR\deps\${PRODUCT_DEPS_DIR}\WinDivert.dll"
  Delete /REBOOTOK "$INSTDIR\deps\${PRODUCT_DEPS_DIR}\WinDivert64.sys"
  Delete /REBOOTOK "$INSTDIR\deps\${PRODUCT_DEPS_DIR}\wireproxy.exe"
  RMDir /r /REBOOTOK "$INSTDIR\deps\${PRODUCT_DEPS_DIR}"
  SetRebootFlag true
  DetailPrint "WinDivert64.sys is still loaded; dependency cleanup was scheduled for the next reboot."
  Return
FunctionEnd

Section "Install" SEC01
  SetShellVarContext current
  SetOverwrite on

  Call StopTunnelProcesses

  SetOutPath "$INSTDIR"

  File "${PAYLOAD_DIR}\${PRODUCT_EXE}"
  File "${PAYLOAD_DIR}\vpnfy.ico"

  IfFileExists "$INSTDIR\deps\${PRODUCT_DEPS_DIR}\wireproxy.exe" 0 copy_current_deps
  IfFileExists "$INSTDIR\deps\${PRODUCT_DEPS_DIR}\ProxyBridge_CLI.exe" 0 copy_current_deps
  IfFileExists "$INSTDIR\deps\${PRODUCT_DEPS_DIR}\WinDivert64.sys" install_deps_ready copy_current_deps

copy_current_deps:
  SetOutPath "$INSTDIR\deps\${PRODUCT_DEPS_DIR}"
  File "${PAYLOAD_DIR}\deps\${PRODUCT_DEPS_DIR}\ProxyBridgeCore.dll"
  File "${PAYLOAD_DIR}\deps\${PRODUCT_DEPS_DIR}\ProxyBridge_CLI.exe"
  File "${PAYLOAD_DIR}\deps\${PRODUCT_DEPS_DIR}\WinDivert.dll"
  File "${PAYLOAD_DIR}\deps\${PRODUCT_DEPS_DIR}\WinDivert64.sys"
  File "${PAYLOAD_DIR}\deps\${PRODUCT_DEPS_DIR}\wireproxy.exe"

install_deps_ready:

  SetOutPath "$INSTDIR"
  CreateDirectory "$INSTDIR\logs"
  CreateDirectory "$INSTDIR\permissions"
  CreateDirectory "$INSTDIR\configs"
  CreateDirectory "$INSTDIR\cache"
  CreateDirectory "$INSTDIR\cache\updates"

  CreateDirectory "$SMPROGRAMS\${PRODUCT_NAME}"
  CreateShortCut "$SMPROGRAMS\${PRODUCT_NAME}\${PRODUCT_NAME}.lnk" "$INSTDIR\${PRODUCT_EXE}" "" "$INSTDIR\vpnfy.ico"
  CreateShortCut "$DESKTOP\${PRODUCT_NAME}.lnk" "$INSTDIR\${PRODUCT_EXE}" "" "$INSTDIR\vpnfy.ico"

  !insertmacro RunHidden '"$SYSDIR\netsh.exe" advfirewall firewall delete rule name="${FIREWALL_RULE_WIREPROXY}"'
  !insertmacro RunHidden '"$SYSDIR\netsh.exe" advfirewall firewall add rule name="${FIREWALL_RULE_WIREPROXY}" dir=in action=allow program="$INSTDIR\deps\${PRODUCT_DEPS_DIR}\wireproxy.exe" enable=yes profile=any remoteip=any description="${FIREWALL_RULE_DESCRIPTION}"'
  !insertmacro RunHidden '"$SYSDIR\netsh.exe" advfirewall firewall delete rule name="${FIREWALL_RULE_PROXYBRIDGE}"'
  !insertmacro RunHidden '"$SYSDIR\netsh.exe" advfirewall firewall add rule name="${FIREWALL_RULE_PROXYBRIDGE}" dir=in action=allow program="$INSTDIR\deps\${PRODUCT_DEPS_DIR}\ProxyBridge_CLI.exe" enable=yes profile=any remoteip=any description="${FIREWALL_RULE_DESCRIPTION}"'
SectionEnd

Section -Post
  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteRegStr HKCU "${PRODUCT_UNINST_KEY}" "DisplayName" "${PRODUCT_DISPLAY_NAME}"
  WriteRegStr HKCU "${PRODUCT_UNINST_KEY}" "UninstallString" "$INSTDIR\uninstall.exe"
  WriteRegStr HKCU "${PRODUCT_UNINST_KEY}" "DisplayIcon" "$INSTDIR\vpnfy.ico"
  WriteRegStr HKCU "${PRODUCT_UNINST_KEY}" "DisplayVersion" "${PRODUCT_VERSION}"
  WriteRegStr HKCU "${PRODUCT_UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${PRODUCT_UNINST_KEY}" "URLInfoAbout" "${PRODUCT_WEB_SITE}"
  WriteRegStr HKCU "${PRODUCT_UNINST_KEY}" "Publisher" "${PRODUCT_PUBLISHER}"
  WriteRegDWORD HKCU "${PRODUCT_UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${PRODUCT_UNINST_KEY}" "NoRepair" 1
SectionEnd

Section "Uninstall"
  SetShellVarContext current
  Call un.StopTunnelProcesses
  Call un.RemoveInstalledDepsWithRetry
  !insertmacro RunHidden '"$SYSDIR\netsh.exe" advfirewall firewall delete rule name="${FIREWALL_RULE_WIREPROXY}"'
  !insertmacro RunHidden '"$SYSDIR\netsh.exe" advfirewall firewall delete rule name="${FIREWALL_RULE_PROXYBRIDGE}"'

  Delete "$SMPROGRAMS\${PRODUCT_NAME}\${PRODUCT_NAME}.lnk"
  Delete "$DESKTOP\${PRODUCT_NAME}.lnk"
  RMDir "$SMPROGRAMS\${PRODUCT_NAME}"

  Delete "$INSTDIR\${PRODUCT_EXE}"
  Delete "$INSTDIR\vpnfy.ico"
  Delete "$INSTDIR\uninstall.exe"
  Delete "$INSTDIR\app.info"
  Delete /REBOOTOK "$INSTDIR\app.info"
  RMDir /r /REBOOTOK "$INSTDIR\deps"
  RMDir /r "$INSTDIR\logs"
  RMDir /r "$INSTDIR\permissions"
  RMDir /r "$INSTDIR\configs"
  RMDir /r "$INSTDIR\cache"
  RMDir /REBOOTOK "$INSTDIR"

  DeleteRegKey HKCU "${PRODUCT_UNINST_KEY}"
  SetAutoClose true
SectionEnd