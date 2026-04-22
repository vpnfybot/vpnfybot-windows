@echo off
echo Building minimal ProxyBridgeCore.dll...

REM Set Visual Studio environment
call "C:\Program Files (x86)\Microsoft Visual Studio\2019\BuildTools\VC\Auxiliary\Build\vcvarsall.bat" x64

if errorlevel 1 (
    echo Failed to set Visual Studio environment!
    pause
    exit /b 1
)

REM Create minimal source file
echo Creating minimal C source...
(
echo // MinimalProxyBridgeCore.c
echo #define WIN32_LEAN_AND_MEAN
echo #include <windows.h>
echo #include <stdio.h>
echo.
echo #ifdef PROXYBRIDGE_EXPORTS
echo #define PROXYBRIDGE_API __declspec(dllexport)
echo #else
echo #define PROXYBRIDGE_API __declspec(dllimport)
echo #endif
echo.
echo typedef void (*LogCallback)(const char* message);
echo.
echo static LogCallback g_logCallback = NULL;
echo static FILE* g_logFile = NULL;
echo.
echo static void write_log(const char* msg) {
echo     if (g_logFile) {
echo         fprintf(g_logFile, "%%s\n", msg);
echo         fflush(g_logFile);
echo     }
echo }
echo.
echo PROXYBRIDGE_API DWORD ProxyBridge_AddRule(const char* process_name, const char* target_hosts, 
echo                                           const char* target_ports, DWORD protocol, DWORD action) {
echo     write_log("ProxyBridge_AddRule called");
echo     if (g_logCallback) {
echo         g_logCallback("Rule added (minimal)");
echo     }
echo     return 1;
echo }
echo.
echo PROXYBRIDGE_API BOOL ProxyBridge_EnableRule(DWORD rule_id) {
echo     write_log("ProxyBridge_EnableRule called");
echo     return TRUE;
echo }
echo.
echo PROXYBRIDGE_API BOOL ProxyBridge_DisableRule(DWORD rule_id) {
echo     write_log("ProxyBridge_DisableRule called");
echo     return TRUE;
echo }
echo.
echo PROXYBRIDGE_API BOOL ProxyBridge_DeleteRule(DWORD rule_id) {
echo     write_log("ProxyBridge_DeleteRule called");
echo     return TRUE;
echo }
echo.
echo PROXYBRIDGE_API BOOL ProxyBridge_SetProxyConfig(DWORD type, const char* proxy_ip, 
echo                                                 USHORT proxy_port, const char* username, const char* password) {
echo     write_log("ProxyBridge_SetProxyConfig called");
echo     if (g_logCallback) {
echo         char buffer[256];
echo         sprintf(buffer, "Proxy set: %%s://%%s:%%d", type == 0 ? "HTTP" : "SOCKS5", proxy_ip, proxy_port);
echo         g_logCallback(buffer);
echo     }
echo     return TRUE;
echo }
echo.
echo PROXYBRIDGE_API void ProxyBridge_SetDnsViaProxy(BOOL enable) {
echo     write_log("ProxyBridge_SetDnsViaProxy called");
echo }
echo.
echo PROXYBRIDGE_API void ProxyBridge_SetLocalhostViaProxy(BOOL enable) {
echo     write_log("ProxyBridge_SetLocalhostViaProxy called");
echo }
echo.
echo PROXYBRIDGE_API void ProxyBridge_SetLogCallback(LogCallback callback) {
echo     write_log("ProxyBridge_SetLogCallback called");
echo     g_logCallback = callback;
echo     if (callback) {
echo         callback("Log callback registered");
echo     }
echo }
echo.
echo PROXYBRIDGE_API void ProxyBridge_SetConnectionCallback(void* callback) {
echo     write_log("ProxyBridge_SetConnectionCallback called");
echo }
echo.
echo PROXYBRIDGE_API void ProxyBridge_SetTrafficLoggingEnabled(BOOL enable) {
echo     write_log("ProxyBridge_SetTrafficLoggingEnabled called");
echo }
echo.
echo PROXYBRIDGE_API BOOL ProxyBridge_Start(void) {
echo     write_log("ProxyBridge_Start called");
echo     g_logFile = fopen("ProxyBridgeCore.log", "a");
echo     if (g_logCallback) {
echo         g_logCallback("ProxyBridge started (minimal)");
echo     }
echo     return TRUE;
echo }
echo.
echo PROXYBRIDGE_API BOOL ProxyBridge_Stop(void) {
echo     write_log("ProxyBridge_Stop called");
echo     if (g_logFile) {
echo         fclose(g_logFile);
echo         g_logFile = NULL;
echo     }
echo     if (g_logCallback) {
echo         g_logCallback("ProxyBridge stopped");
echo     }
echo     return TRUE;
echo }
echo.
echo PROXYBRIDGE_API BOOL ProxyBridge_TestConnection(const char* target_host, USHORT target_port, 
echo                                                 char* result_buffer, ULONG buffer_size) {
echo     write_log("ProxyBridge_TestConnection called");
echo     if (result_buffer && buffer_size > 10) {
echo         strcpy(result_buffer, "OK");
echo     }
echo     return TRUE;
echo }
echo.
echo BOOL WINAPI DllMain(HINSTANCE hinstDLL, DWORD fdwReason, LPVOID lpvReserved) {
echo     return TRUE;
echo }
) > minimal.c

echo Compiling...
cl.exe /nologo /O2 /D_CRT_SECURE_NO_WARNINGS /DPROXYBRIDGE_EXPORTS minimal.c /LD /link /OUT:ProxyBridgeCore.dll

if errorlevel 1 (
    echo Compilation failed!
    del minimal.c 2>nul
    pause
    exit /b 1
)

echo.
echo Successfully compiled ProxyBridgeCore.dll!
echo.
echo Copying files to target/release...
copy ProxyBridgeCore.dll "..\..\..\src\target\release\" >nul
if errorlevel 1 (
    echo Failed to copy DLL!
) else (
    echo Copied ProxyBridgeCore.dll to target/release
)

echo.
echo Creating windivert.h stub...
(
echo #ifndef _WINDIVERT_H_
echo #define _WINDIVERT_H_
echo #include <windows.h>
echo.
echo #define WINDIVERT_LAYER_NETWORK        0
echo #define WINDIVERT_LAYER_NETWORK_FORWARD 1
echo.
echo #define WINDIVERT_FLAG_SNIFF            0x0001
echo #define WINDIVERT_FLAG_DROP             0x0002
echo #define WINDIVERT_FLAG_RECV_ONLY        0x0100
echo.
echo typedef struct _WINDIVERT_ADDRESS {
echo     ULONG64 Timestamp;
echo     ULONG   Layer:8;
echo     ULONG   Event:8;
echo     ULONG   Sniffed:1;
echo     ULONG   Outbound:1;
echo     ULONG   Loopback:1;
echo     ULONG   Impostor:1;
echo     ULONG   IPv6:1;
echo     ULONG   TCP:1;
echo     ULONG   UDP:1;
echo     ULONG   Reserved:8;
echo     ULONG   Reserved2:4;
echo     ULONG   IfIdx;
echo     ULONG   SubIfIdx;
echo } WINDIVERT_ADDRESS, *PWINDIVERT_ADDRESS;
echo.
echo HANDLE WINAPI WinDivertOpen(const char* filter, WINDIVERT_LAYER layer, INT16 priority, UINT64 flags);
echo BOOL WINAPI WinDivertRecv(HANDLE handle, void* packet, UINT packet_len, UINT* recv_len, PWINDIVERT_ADDRESS addr);
echo BOOL WINAPI WinDivertSend(HANDLE handle, void* packet, UINT packet_len, UINT* send_len, PWINDIVERT_ADDRESS addr);
echo BOOL WINAPI WinDivertClose(HANDLE handle);
echo.
echo #endif // _WINDIVERT_H_
) > windivert.h

copy windivert.h "..\..\..\src\target\release\" >nul
if errorlevel 1 (
    echo Failed to copy windivert.h!
) else (
    echo Copied windivert.h to target/release
)

echo.
echo Cleaning up...
del minimal.c 2>nul
del windivert.h 2>nul
del minimal.obj 2>nul
del minimal.exp 2>nul
del minimal.lib 2>nul

echo.
echo === COMPLETED ===
echo 1. ProxyBridgeCore.dll created (minimal stub version)
echo 2. windivert.h created (compatibility stub)
echo 3. Both copied to src\target\release\
echo.
echo The DLL will log calls to ProxyBridgeCore.log file.
echo For real traffic interception, you need to compile WinDivert.
echo.
pause