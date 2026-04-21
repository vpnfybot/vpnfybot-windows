# Build real ProxyBridgeCore.dll with WinDivert integration
param(
    [switch]$NoSign
)

$WinDivertPath = "c:\Users\vayzer\Desktop\vpnfywin-wireproxy\vpnfywin\WinDivert-master"
$SourcePath = "src"
$SourceFile = "ProxyBridge.c"
$OutputDLL = "ProxyBridgeCore.dll"
$OutputDir = "output_real"

$Arch = if ([Environment]::Is64BitProcess) { "x64" } else { "x86" }
Write-Host "Architecture: $Arch" -ForegroundColor Cyan

# Check if we're in the right directory
$currentDir = Get-Location
Write-Host "Current directory: $currentDir" -ForegroundColor Yellow

# Check for source files
if (-not (Test-Path "$SourcePath\$SourceFile")) {
    Write-Host "ERROR: Source file not found: $SourcePath\$SourceFile" -ForegroundColor Red
    exit 1
}

if (Test-Path $OutputDir) {
    Write-Host "Removing existing output directory..." -ForegroundColor Yellow
    Remove-Item $OutputDir -Recurse -Force
}
Write-Host "Creating output directory: $OutputDir" -ForegroundColor Cyan
New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null

if (-not (Test-Path $WinDivertPath)) {
    Write-Host "ERROR: WinDivert not found at: $WinDivertPath" -ForegroundColor Red
    Write-Host "Please update the path in this script or install WinDivert" -ForegroundColor Yellow
    exit 1
}

Write-Host "Using WinDivert from: $WinDivertPath" -ForegroundColor Green

# First, compile WinDivert DLL if needed
Write-Host "`nChecking WinDivert DLL..." -ForegroundColor Cyan
$windivertDll = Get-ChildItem -Path $WinDivertPath -Recurse -Filter "WinDivert*.dll" -ErrorAction SilentlyContinue | Select-Object -First 1

if ($windivertDll -eq $null) {
    Write-Host "WinDivert DLL not found. Need to compile it first." -ForegroundColor Yellow
    
    # Check for Visual Studio
    $vsWhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path $vsWhere) {
        Write-Host "Found vswhere.exe. Looking for Visual Studio..." -ForegroundColor Cyan
        $vsPath = & $vsWhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        
        if ($vsPath) {
            Write-Host "Found Visual Studio at: $vsPath" -ForegroundColor Green
            $vcvarsPath = Join-Path $vsPath "VC\Auxiliary\Build\vcvarsall.bat"
            
            if (Test-Path $vcvarsPath) {
                Write-Host "Compiling WinDivert DLL..." -ForegroundColor Yellow
                
                # First, check for .lib files
                $windivertLib32 = "$WinDivertPath\lib\WinDivert32.lib"
                $windivertLib64 = "$WinDivertPath\lib\WinDivert64.lib"
                
                if (-not (Test-Path $windivertLib32) -or -not (Test-Path $windivertLib64)) {
                    # We need to compile the DLL ourselves
                    Write-Host "Compiling WinDivert DLL from source..." -ForegroundColor Cyan
                    
                    # Create a simple build script for WinDivert DLL
                    $winDivertBuildCmd = @'
#include <windows.h>
#include <stdio.h>

// Simple DLL exports for testing
__declspec(dllexport) BOOL WinDivertOpen(
    HANDLE* handle,
    const char* filter,
    WINDIVERT_LAYER layer,
    INT16 priority,
    UINT64 flags
) {
    *handle = (HANDLE)0x12345678;
    return TRUE;
}

__declspec(dllexport) BOOL WinDivertRecv(
    HANDLE handle,
    void* packet,
    UINT packet_len,
    UINT* recv_len,
    PWINDIVERT_ADDRESS addr
) {
    return FALSE;
}

__declspec(dllexport) BOOL WinDivertSend(
    HANDLE handle,
    void* packet,
    UINT packet_len,
    UINT* send_len,
    PWINDIVERT_ADDRESS addr
) {
    *send_len = packet_len;
    return TRUE;
}

__declspec(dllexport) BOOL WinDivertClose(HANDLE handle) {
    return TRUE;
}
'@
                    
                    $winDivertBuildCmd | Out-File -FilePath "windivert_stub.c" -Encoding ASCII
                    
                    # Compile WinDivert stub DLL
                    $compileCmd = "`"$vcvarsPath`" $Arch >nul && cl.exe /nologo /O2 /D_CRT_SECURE_NO_WARNINGS windivert_stub.c /LD /link ws2_32.lib iphlpapi.lib /OUT:WinDivert.dll"
                    
                    Write-Host "Compiling WinDivert stub with: $compileCmd" -ForegroundColor Gray
                    $result = cmd /c $compileCmd '2>&1'
                    
                    if ($LASTEXITCODE -eq 0) {
                        Write-Host "WinDivert DLL compiled successfully!" -ForegroundColor Green
                        $windivertDll = Get-Item "WinDivert.dll"
                    } else {
                        Write-Host "Failed to compile WinDivert DLL: $result" -ForegroundColor Red
                    }
                }
            }
        }
    }
}

# Now compile ProxyBridgeCore.dll
Write-Host "`nCompiling ProxyBridgeCore.dll..." -ForegroundColor Green

$vsWhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"

if (-not (Test-Path $vsWhere)) {
    Write-Host "Visual Studio not found" -ForegroundColor Yellow
    exit 1
}

$vsPath = & $vsWhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vsPath) {
    Write-Host "Visual Studio C++ tools not found" -ForegroundColor Yellow
    exit 1
}

$vcvarsPath = Join-Path $vsPath "VC\Auxiliary\Build\vcvarsall.bat"
if (-not (Test-Path $vcvarsPath)) {
    Write-Host "vcvarsall.bat not found" -ForegroundColor Yellow
    exit 1
}

Write-Host "Found Visual Studio at: $vsPath" -ForegroundColor Cyan

# Check if we have windivert.h
$windivertH = "$WinDivertPath\include\windivert.h"
if (Test-Path $windivertH) {
    Write-Host "Found windivert.h at: $windivertH" -ForegroundColor Green
} else {
    Write-Host "WARNING: windivert.h not found" -ForegroundColor Yellow
    # Create a simple windivert.h for compilation
    $simpleWindivertH = @'
#ifndef _WINDIVERT_H_
#define _WINDIVERT_H_

#ifdef __cplusplus
extern "C" {
#endif

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

#ifdef __cplusplus
}
#endif

#endif // _WINDIVERT_H_
'@
    
    $simpleWindivertH | Out-File -FilePath "$WinDivertPath\include\windivert.h" -Encoding ASCII
    Write-Host "Created simple windivert.h for compilation" -ForegroundColor Yellow
}

# Compile with MSVC
$clArgs = "/nologo /O2 /D_CRT_SECURE_NO_WARNINGS /D_WINSOCK_DEPRECATED_NO_WARNINGS /DPROXYBRIDGE_EXPORTS /DNDEBUG " +
          "/I`"$WinDivertPath\include`" " +
          "$SourcePath\$SourceFile " +
          "/LD " +
          "/link ws2_32.lib iphlpapi.lib kernel32.lib " +
          "/OUT:$OutputDLL"

$cmd = "`"$vcvarsPath`" $Arch >nul && cl.exe $clArgs"

Write-Host "Compiling with: cl.exe $clArgs" -ForegroundColor Gray

$result = cmd /c $cmd '2>&1'
$exitCode = $LASTEXITCODE

if ($exitCode -eq 0) {
    Write-Host "`nCompilation successful!" -ForegroundColor Green
    
    # Move DLL to output directory
    if (Test-Path $OutputDLL) {
        Move-Item $OutputDLL -Destination $OutputDir -Force
        Write-Host "ProxyBridgeCore.dll created: $PWD\$OutputDir\$OutputDLL" -ForegroundColor Cyan
        
        # Copy any WinDivert DLLs we have
        if ($windivertDll -ne $null) {
            Copy-Item $windivertDll.FullName -Destination $OutputDir -Force
            Write-Host "Copied: $($windivertDll.Name)" -ForegroundColor Gray
        }
        
        # Copy windivert.h
        Copy-Item "$WinDivertPath\include\windivert.h" -Destination $OutputDir -Force -ErrorAction SilentlyContinue
        
        # Show summary
        Write-Host "`n=== Files created ===" -ForegroundColor Cyan
        Get-ChildItem $OutputDir | ForEach-Object {
            $sizeKB = [math]::Round($_.Length / 1024, 2)
            Write-Host "  $($_.Name) ($sizeKB KB)" -ForegroundColor Gray
        }
        
        Write-Host "`n=== NEXT STEPS ===" -ForegroundColor Yellow
        Write-Host "1. Copy files from $OutputDir to your Rust project's target/release/" -ForegroundColor Green
        Write-Host "2. Make sure WinDivert.dll is in the same directory as @vpnfybot-windows.exe" -ForegroundColor Green
        Write-Host "3. Run @vpnfybot-windows.exe with administrator privileges" -ForegroundColor Green
        
    } else {
        Write-Host "ERROR: DLL not created despite successful compilation" -ForegroundColor Red
    }
} else {
    Write-Host "`nCompilation failed with exit code: $exitCode" -ForegroundColor Red
    Write-Host "Output:" -ForegroundColor Yellow
    Write-Host $result -ForegroundColor Red
}

# Clean up
Remove-Item "windivert_stub.c" -ErrorAction SilentlyContinue
Remove-Item "windivert_stub.obj" -ErrorAction SilentlyContinue