# Simple compile script for ProxyBridgeCore.dll
$WinDivertPath = "c:\Users\vayzer\Desktop\vpnfywin-wireproxy\vpnfywin\WinDivert-master"
$OutputDLL = "ProxyBridgeCore.dll"
$OutputDir = "output"

Write-Host "=== ProxyBridge Core Compilation ===" -ForegroundColor Cyan

# Create output directory
if (Test-Path $OutputDir) {
    Remove-Item $OutputDir -Recurse -Force
}
New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null

# Check if WinDivert exists
if (-not (Test-Path $WinDivertPath)) {
    Write-Host "ERROR: WinDivert not found at: $WinDivertPath" -ForegroundColor Red
    exit 1
}

Write-Host "WinDivert found at: $WinDivertPath" -ForegroundColor Green

# First, try to compile a stub without WinDivert
Write-Host "`nCreating stub ProxyBridgeCore.dll..." -ForegroundColor Yellow

# Create a simple stub C source file
$stubSource = '#include <windows.h>
#include <stdio.h>
#include <string.h>

#ifdef PROXYBRIDGE_EXPORTS
#define PROXYBRIDGE_API __declspec(dllexport)
#else
#define PROXYBRIDGE_API __declspec(dllimport)
#endif

typedef void (*LogCallback)(const char* message);
typedef void (*ConnectionCallback)(const char* process_name, DWORD pid, const char* dest_ip, USHORT dest_port, const char* proxy_info);

static LogCallback g_logCallback = NULL;
static BOOL g_isRunning = FALSE;
static DWORD g_nextRuleId = 1;

PROXYBRIDGE_API DWORD ProxyBridge_AddRule(const char* process_name, const char* target_hosts, const char* target_ports, DWORD protocol, DWORD action)
{
    if (g_logCallback != NULL)
    {
        char buffer[256];
        snprintf(buffer, sizeof(buffer), "Rule added: %s -> %s:%s:%lu -> %lu", 
                 process_name, target_hosts, target_ports, protocol, action);
        g_logCallback(buffer);
    }
    return g_nextRuleId++;
}

PROXYBRIDGE_API BOOL ProxyBridge_EnableRule(DWORD rule_id)
{
    if (g_logCallback != NULL)
    {
        char buffer[128];
        snprintf(buffer, sizeof(buffer), "Rule %lu enabled", rule_id);
        g_logCallback(buffer);
    }
    return TRUE;
}

PROXYBRIDGE_API BOOL ProxyBridge_DisableRule(DWORD rule_id)
{
    if (g_logCallback != NULL)
    {
        char buffer[128];
        snprintf(buffer, sizeof(buffer), "Rule %lu disabled", rule_id);
        g_logCallback(buffer);
    }
    return TRUE;
}

PROXYBRIDGE_API BOOL ProxyBridge_DeleteRule(DWORD rule_id)
{
    if (g_logCallback != NULL)
    {
        char buffer[128];
        snprintf(buffer, sizeof(buffer), "Rule %lu deleted", rule_id);
        g_logCallback(buffer);
    }
    return TRUE;
}

PROXYBRIDGE_API BOOL ProxyBridge_SetProxyConfig(DWORD type, const char* proxy_ip, USHORT proxy_port, const char* username, const char* password)
{
    if (g_logCallback != NULL)
    {
        char buffer[256];
        const char* typeStr = (type == 0) ? "HTTP" : "SOCKS5";
        snprintf(buffer, sizeof(buffer), "Proxy set: %s://%s:%u", typeStr, proxy_ip, proxy_port);
        if (username != NULL && username[0] != '\0')
        {
            strncat(buffer, " (auth: ", sizeof(buffer) - strlen(buffer) - 1);
            strncat(buffer, username, sizeof(buffer) - strlen(buffer) - 1);
            strncat(buffer, ")", sizeof(buffer) - strlen(buffer) - 1);
        }
        g_logCallback(buffer);
    }
    return TRUE;
}

PROXYBRIDGE_API void ProxyBridge_SetDnsViaProxy(BOOL enable)
{
    if (g_logCallback != NULL)
    {
        char buffer[128];
        snprintf(buffer, sizeof(buffer), "DNS via proxy: %s", enable ? "enabled" : "disabled");
        g_logCallback(buffer);
    }
}

PROXYBRIDGE_API void ProxyBridge_SetLocalhostViaProxy(BOOL enable)
{
    if (g_logCallback != NULL)
    {
        char buffer[128];
        snprintf(buffer, sizeof(buffer), "Localhost via proxy: %s", enable ? "enabled" : "disabled");
        g_logCallback(buffer);
    }
}

PROXYBRIDGE_API void ProxyBridge_SetLogCallback(LogCallback callback)
{
    g_logCallback = callback;
}

PROXYBRIDGE_API void ProxyBridge_SetConnectionCallback(ConnectionCallback callback)
{
    // Stub - not implemented
}

PROXYBRIDGE_API void ProxyBridge_SetTrafficLoggingEnabled(BOOL enable)
{
    // Stub - not implemented
}

PROXYBRIDGE_API BOOL ProxyBridge_Start(void)
{
    if (g_isRunning)
    {
        return FALSE;
    }
    
    if (g_logCallback != NULL)
    {
        g_logCallback("ProxyBridge started (stub mode)");
    }
    
    g_isRunning = TRUE;
    return TRUE;
}

PROXYBRIDGE_API BOOL ProxyBridge_Stop(void)
{
    if (!g_isRunning)
    {
        return FALSE;
    }
    
    if (g_logCallback != NULL)
    {
        g_logCallback("ProxyBridge stopped");
    }
    
    g_isRunning = FALSE;
    return TRUE;
}

BOOL APIENTRY DllMain(HMODULE hModule, DWORD  ul_reason_for_call, LPVOID lpReserved)
{
    return TRUE;
}'

$stubSource | Out-File -FilePath "stub.c" -Encoding ASCII

# Try to find a C compiler
$compilerFound = $false

# Check for MSBuild
$msbuildPath = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
if (Test-Path $msbuildPath) {
    $vsPath = & $msbuildPath -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if ($vsPath) {
        Write-Host "Found Visual Studio at: $vsPath" -ForegroundColor Cyan
        $vcvarsPath = Join-Path $vsPath "VC\Auxiliary\Build\vcvarsall.bat"
        if (Test-Path $vcvarsPath) {
            # Try to compile with cl.exe
            $cmd = "`"$vcvarsPath`" x64 >nul && cl.exe /nologo /O2 /D_CRT_SECURE_NO_WARNINGS /D_WINSOCK_DEPRECATED_NO_WARNINGS /DPROXYBRIDGE_EXPORTS /DNDEBUG stub.c /LD /link ws2_32.lib /OUT:$OutputDLL"
            Write-Host "Compiling with MSVC..." -ForegroundColor Green
            $result = cmd /c $cmd '2>&1'
            if ($LASTEXITCODE -eq 0) {
                $compilerFound = $true
                Write-Host "MSVC compilation successful!" -ForegroundColor Green
            } else {
                Write-Host "MSVC compilation failed: $result" -ForegroundColor Yellow
            }
        }
    }
}

# If MSVC failed, check for GCC
if (-not $compilerFound) {
    $gccCheck = cmd /c "gcc --version 2>&1"
    if ($LASTEXITCODE -eq 0) {
        Write-Host "Found GCC compiler" -ForegroundColor Cyan
        $cmd = "gcc -shared -O2 -s -Wall -D_WIN32_WINNT=0x0601 -DPROXYBRIDGE_EXPORTS stub.c -lws2_32 -lkernel32 -o $OutputDLL"
        Write-Host "Compiling with GCC..." -ForegroundColor Green
        $result = cmd /c $cmd '2>&1'
        if ($LASTEXITCODE -eq 0) {
            $compilerFound = $true
            Write-Host "GCC compilation successful!" -ForegroundColor Green
        } else {
            Write-Host "GCC compilation failed: $result" -ForegroundColor Yellow
        }
    }
}

# Clean up stub file
Remove-Item "stub.c" -Force -ErrorAction SilentlyContinue

if ($compilerFound) {
    # Move the DLL to output directory
    if (Test-Path $OutputDLL) {
        Move-Item $OutputDLL -Destination $OutputDir -Force
        Write-Host "`nProxyBridgeCore.dll created successfully!" -ForegroundColor Green
        Write-Host "Location: $PWD\$OutputDir\$OutputDLL" -ForegroundColor Cyan
        
        # Copy necessary files
        # Copy windivert.h if exists
        $windivertH = Join-Path $WinDivertPath "include\windivert.h"
        if (Test-Path $windivertH) {
            Copy-Item $windivertH -Destination $OutputDir -Force
            Write-Host "Copied: windivert.h" -ForegroundColor Gray
        }
        
        # Copy WinDivert DLL if exists
        $windivertDll = Get-ChildItem -Path $WinDivertPath -Recurse -Filter "WinDivert*.dll" -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($windivertDll -ne $null) {
            Copy-Item $windivertDll.FullName -Destination $OutputDir -Force
            Write-Host "Copied: $($windivertDll.Name)" -ForegroundColor Gray
        } else {
            Write-Host "Warning: WinDivert DLL not found. ProxyBridge will run in stub mode." -ForegroundColor Yellow
        }
        
        # Show summary
        Write-Host "`n=== Summary ===" -ForegroundColor Cyan
        Get-ChildItem $OutputDir | ForEach-Object {
            $sizeKB = [math]::Round($_.Length / 1024, 2)
            Write-Host "  $($_.Name) ($sizeKB KB)" -ForegroundColor Gray
        }
    } else {
        Write-Host "ERROR: DLL was not created" -ForegroundColor Red
        exit 1
    }
} else {
    Write-Host "`nERROR: No C compiler found (MSVC or GCC required)" -ForegroundColor Red
    Write-Host "`nPlease install one of:" -ForegroundColor Yellow
    Write-Host "1. Visual Studio Build Tools (with C++ workload)" -ForegroundColor Yellow
    Write-Host "2. MinGW-w64 (GCC for Windows)" -ForegroundColor Yellow
    Write-Host "3. Microsoft C++ Build Tools" -ForegroundColor Yellow
    exit 1
}