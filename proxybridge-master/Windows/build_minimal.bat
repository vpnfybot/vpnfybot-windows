@echo off
echo Building MinimalProxyBridge.dll...

REM Set Visual Studio environment
call "C:\Program Files (x86)\Microsoft Visual Studio\2019\BuildTools\VC\Auxiliary\Build\vcvarsall.bat" x64

REM Compile minimal DLL
cl.exe /nologo /O2 /D_CRT_SECURE_NO_WARNINGS /D_WINSOCK_DEPRECATED_NO_WARNINGS /DPROXYBRIDGE_EXPORTS /DNDEBUG src\MinimalProxyBridge.c /LD /link ws2_32.lib /OUT:ProxyBridgeCore.dll

if errorlevel 1 (
    echo Compilation failed!
    pause
    exit /b 1
)

echo.
echo MinimalProxyBridge.dll compiled successfully!
echo Files created:
dir ProxyBridgeCore.dll
echo.
echo Copy this DLL to your Rust project's target/release/ folder
echo.

REM Create a simple windivert.h stub
echo #ifndef _WINDIVERT_H_ > windivert.h
echo #define _WINDIVERT_H_ >> windivert.h
echo #include <windows.h> >> windivert.h
echo. >> windivert.h
echo #define WINDIVERT_LAYER_NETWORK        0 >> windivert.h
echo #define WINDIVERT_LAYER_NETWORK_FORWARD 1 >> windivert.h
echo. >> windivert.h
echo #define WINDIVERT_FLAG_SNIFF            0x0001 >> windivert.h
echo #define WINDIVERT_FLAG_DROP             0x0002 >> windivert.h
echo #define WINDIVERT_FLAG_RECV_ONLY        0x0100 >> windivert.h
echo. >> windivert.h
echo typedef struct _WINDIVERT_ADDRESS { >> windivert.h
echo     ULONG64 Timestamp; >> windivert.h
echo     ULONG   Layer:8; >> windivert.h
echo     ULONG   Event:8; >> windivert.h
echo     ULONG   Sniffed:1; >> windivert.h
echo     ULONG   Outbound:1; >> windivert.h
echo     ULONG   Loopback:1; >> windivert.h
echo     ULONG   Impostor:1; >> windivert.h
echo     ULONG   IPv6:1; >> windivert.h
echo     ULONG   TCP:1; >> windivert.h
echo     ULONG   UDP:1; >> windivert.h
echo     ULONG   Reserved:8; >> windivert.h
echo     ULONG   Reserved2:4; >> windivert.h
echo     ULONG   IfIdx; >> windivert.h
echo     ULONG   SubIfIdx; >> windivert.h
echo } WINDIVERT_ADDRESS, *PWINDIVERT_ADDRESS; >> windivert.h
echo. >> windivert.h
echo HANDLE WINAPI WinDivertOpen(const char* filter, WINDIVERT_LAYER layer, INT16 priority, UINT64 flags); >> windivert.h
echo BOOL WINAPI WinDivertRecv(HANDLE handle, void* packet, UINT packet_len, UINT* recv_len, PWINDIVERT_ADDRESS addr); >> windivert.h
echo BOOL WINAPI WinDivertSend(HANDLE handle, void* packet, UINT packet_len, UINT* send_len, PWINDIVERT_ADDRESS addr); >> windivert.h
echo BOOL WINAPI WinDivertClose(HANDLE handle); >> windivert.h
echo. >> windivert.h
echo #endif // _WINDIVERT_H_ >> windivert.h

echo Created windivert.h stub for compatibility
echo.
echo Copy both ProxyBridgeCore.dll and windivert.h to target/release/
pause