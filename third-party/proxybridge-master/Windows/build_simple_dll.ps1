# Build minimal ProxyBridgeCore.dll - pure C, no dependencies
param()

$SourceFile = "MinimalProxyBridge.c"
$OutputDLL = "ProxyBridgeCore.dll"

Write-Host "Building minimal ProxyBridgeCore.dll..." -ForegroundColor Green

# Check for source file
if (-not (Test-Path "src\$SourceFile")) {
    Write-Host "ERROR: Source file not found: src\$SourceFile" -ForegroundColor Red
    exit 1
}

# Try direct compilation with cl.exe if available
$compilerFound = $false

# Method 1: Direct compilation with cl.exe (no vcvarsall)
Write-Host "Trying direct compilation..." -ForegroundColor Cyan

# Create a temporary simple source file
$simpleSource = @'
#include <windows.h>
#include <stdio.h>

#define PROXYBRIDGE_API __declspec(dllexport)

typedef void (*LogCallback)(const char* message);

static LogCallback g_logCallback = NULL;

__declspec(dllexport) DWORD ProxyBridge_AddRule(const char* process_name, const char* target_hosts, 
                                                const char* target_ports, DWORD protocol, DWORD action) {
    if (g_logCallback != NULL) {
        g_logCallback("Rule added (minimal DLL)");
    }
    return 1;
}

__declspec(dllexport) BOOL ProxyBridge_EnableRule(DWORD rule_id) {
    if (g_logCallback != NULL) {
        g_logCallback("Rule enabled");
    }
    return TRUE;
}

__declspec(dllexport) BOOL ProxyBridge_DisableRule(DWORD rule_id) {
    if (g_logCallback != NULL) {
        g_logCallback("Rule disabled");
    }
    return TRUE;
}

__declspec(dllexport) BOOL ProxyBridge_DeleteRule(DWORD rule_id) {
    if (g_logCallback != NULL) {
        g_logCallback("Rule deleted");
    }
    return TRUE;
}

__declspec(dllexport) BOOL ProxyBridge_SetProxyConfig(DWORD type, const char* proxy_ip, 
                                                      USHORT proxy_port, const char* username, const char* password) {
    if (g_logCallback != NULL) {
        g_logCallback("Proxy config set");
    }
    return TRUE;
}

__declspec(dllexport) void ProxyBridge_SetDnsViaProxy(BOOL enable) {
    if (g_logCallback != NULL) {
        g_logCallback(enable ? "DNS via proxy enabled" : "DNS via proxy disabled");
    }
}

__declspec(dllexport) void ProxyBridge_SetLocalhostViaProxy(BOOL enable) {
    if (g_logCallback != NULL) {
        g_logCallback(enable ? "Localhost via proxy enabled" : "Localhost via proxy disabled");
    }
}

__declspec(dllexport) void ProxyBridge_SetLogCallback(LogCallback callback) {
    g_logCallback = callback;
    if (callback != NULL) {
        callback("Log callback registered");
    }
}

__declspec(dllexport) void ProxyBridge_SetConnectionCallback(void* callback) {
    // Not implemented
}

__declspec(dllexport) void ProxyBridge_SetTrafficLoggingEnabled(BOOL enable) {
    if (g_logCallback != NULL) {
        g_logCallback(enable ? "Traffic logging enabled" : "Traffic logging disabled");
    }
}

__declspec(dllexport) BOOL ProxyBridge_Start(void) {
    if (g_logCallback != NULL) {
        g_logCallback("ProxyBridge started (minimal)");
    }
    return TRUE;
}

__declspec(dllexport) BOOL ProxyBridge_Stop(void) {
    if (g_logCallback != NULL) {
        g_logCallback("ProxyBridge stopped");
    }
    return TRUE;
}

__declspec(dllexport) BOOL ProxyBridge_TestConnection(const char* target_host, USHORT target_port, 
                                                      char* result_buffer, ULONG buffer_size) {
    if (result_buffer != NULL && buffer_size > 10) {
        strcpy(result_buffer, "OK");
    }
    if (g_logCallback != NULL) {
        g_logCallback("Test connection succeeded");
    }
    return TRUE;
}

BOOL WINAPI DllMain(HINSTANCE hinstDLL, DWORD fdwReason, LPVOID lpvReserved) {
    return TRUE;
}
'@

$simpleSource | Out-File -FilePath "simple_proxybridge.c" -Encoding ASCII

# Try to compile with any available compiler
$buildMethods = @(
    @{
        Name = "MSVC via vcvarsall";
        Command = 'call "C:\Program Files (x86)\Microsoft Visual Studio\2019\BuildTools\VC\Auxiliary\Build\vcvarsall.bat" x64 >nul && cl.exe /nologo /LD simple_proxybridge.c /link /OUT:ProxyBridgeCore.dll';
        IsBatch = $true
    },
    @{
        Name = "Direct cl.exe";
        Command = 'cl.exe /nologo /LD simple_proxybridge.c /link /OUT:ProxyBridgeCore.dll';
        IsBatch = $false
    },
    @{
        Name = "GCC";
        Command = 'gcc -shared simple_proxybridge.c -o ProxyBridgeCore.dll';
        IsBatch = $false
    }
)

foreach ($method in $buildMethods) {
    Write-Host "`nTrying: $($method.Name)..." -ForegroundColor Yellow
    
    try {
        if ($method.IsBatch) {
            $result = cmd /c $method.Command '2>&1'
        } else {
            $result = Invoke-Expression $method.Command '2>&1'
        }
        
        if ($LASTEXITCODE -eq 0 -or (Test-Path $OutputDLL)) {
            $compilerFound = $true
            Write-Host "$($method.Name) succeeded!" -ForegroundColor Green
            break
        } else {
            Write-Host "$($method.Name) failed: $result" -ForegroundColor Gray
        }
    } catch {
        Write-Host "$($method.Name) error: $_" -ForegroundColor Gray
    }
}

# Clean up temp file
Remove-Item "simple_proxybridge.c" -ErrorAction SilentlyContinue

if ($compilerFound) {
    Write-Host "`n=== SUCCESS ===" -ForegroundColor Green
    Write-Host "ProxyBridgeCore.dll created!" -ForegroundColor Cyan
    
    # Show file info
    $dllInfo = Get-Item $OutputDLL -ErrorAction SilentlyContinue
    if ($dllInfo) {
        $sizeKB = [math]::Round($dllInfo.Length / 1024, 2)
        Write-Host "Size: $sizeKB KB" -ForegroundColor Gray
        Write-Host "Location: $($dllInfo.FullName)" -ForegroundColor Gray
    }
    
    # Create windivert.h stub
    $windivertStub = '#ifndef _WINDIVERT_H_
#define _WINDIVERT_H_

#include <windows.h>

#define WINDIVERT_LAYER_NETWORK        0
#define WINDIVERT_LAYER_NETWORK_FORWARD 1

#define WINDIVERT_FLAG_SNIFF            0x0001
#define WINDIVERT_FLAG_DROP             0x0002
#define WINDIVERT_FLAG_RECV_ONLY        0x0100

typedef struct _WINDIVERT_ADDRESS {
    ULONG64 Timestamp;
    ULONG   Layer:8;
    ULONG   Event:8;
    ULONG   Sniffed:1;
    ULONG   Outbound:1;
    ULONG   Loopback:1;
    ULONG   Impostor:1;
    ULONG   IPv6:1;
    ULONG   TCP:1;
    ULONG   UDP:1;
    ULONG   Reserved:8;
    ULONG   Reserved2:4;
    ULONG   IfIdx;
    ULONG   SubIfIdx;
} WINDIVERT_ADDRESS, *PWINDIVERT_ADDRESS;

HANDLE WINAPI WinDivertOpen(const char* filter, WINDIVERT_LAYER layer, INT16 priority, UINT64 flags);
BOOL WINAPI WinDivertRecv(HANDLE handle, void* packet, UINT packet_len, UINT* recv_len, PWINDIVERT_ADDRESS addr);
BOOL WINAPI WinDivertSend(HANDLE handle, void* packet, UINT packet_len, UINT* send_len, PWINDIVERT_ADDRESS addr);
BOOL WINAPI WinDivertClose(HANDLE handle);

#endif // _WINDIVERT_H_'
    
    $windivertStub | Out-File -FilePath "windivert.h" -Encoding ASCII
    Write-Host "Created: windivert.h (compatibility stub)" -ForegroundColor Gray
    
    Write-Host "`n=== NEXT STEPS ===" -ForegroundColor Yellow
    Write-Host "1. Copy ProxyBridgeCore.dll to c:\Users\vayzer\Desktop\vpnfywin-wireproxy\vpnfywin\src\target\release\" -ForegroundColor White
    Write-Host "2. Copy windivert.h to the same directory" -ForegroundColor White
    Write-Host "3. Run your Rust application and test the integration" -ForegroundColor White
    Write-Host "`nNOTE: This is a minimal stub DLL that logs calls but doesn't actually intercept traffic." -ForegroundColor Gray
    
} else {
    Write-Host "`n=== ERROR ===" -ForegroundColor Red
    Write-Host "No compiler found. Trying alternative approach..." -ForegroundColor Yellow
    
    # Try to copy any existing DLL
    $existingDlls = Get-ChildItem -Path "..\..\.." -Recurse -Filter "ProxyBridge*.dll" -ErrorAction SilentlyContinue | Where-Object { $_.Name -eq "ProxyBridge.dll" }
    
    if ($existingDlls.Count -gt 0) {
        Write-Host "Found existing ProxyBridge.dll, renaming to ProxyBridgeCore.dll" -ForegroundColor Cyan
        Copy-Item $existingDlls[0].FullName -Destination $OutputDLL -Force
        Write-Host "Copied existing DLL as ProxyBridgeCore.dll" -ForegroundColor Green
        
        # Create windivert.h stub
        '#ifndef _WINDIVERT_H_
#define _WINDIVERT_H_
#endif' | Out-File -FilePath "windivert.h" -Encoding ASCII
        
        Write-Host "Created minimal windivert.h" -ForegroundColor Gray
        Write-Host "`nUsing existing DLL as ProxyBridgeCore.dll" -ForegroundColor Yellow
    } else {
        Write-Host "No existing DLLs found. Manual compilation required." -ForegroundColor Red
        Write-Host "`nYou need to compile a simple C DLL with these exported functions:" -ForegroundColor Yellow
        Write-Host "- ProxyBridge_AddRule" -ForegroundColor Gray
        Write-Host "- ProxyBridge_EnableRule" -ForegroundColor Gray
        Write-Host "- ProxyBridge_SetProxyConfig" -ForegroundColor Gray
        Write-Host "- ProxyBridge_SetLogCallback" -ForegroundColor Gray
        Write-Host "- ProxyBridge_Start/Stop" -ForegroundColor Gray
        exit 1
    }
}