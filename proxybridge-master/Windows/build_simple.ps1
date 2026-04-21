# Build SimpleProxyBridge.dll (user-space version without WinDivert)
param(
    [switch]$NoSign
)

$SourceFile = "SimpleProxyBridge.c"
$OutputDLL = "ProxyBridgeCore.dll"
$OutputDir = "output_simple"

$Arch = if ([Environment]::Is64BitProcess) { "x64" } else { "x86" }
Write-Host "Architecture: $Arch" -ForegroundColor Cyan

# Check if we're in the right directory
$currentDir = Get-Location
Write-Host "Current directory: $currentDir" -ForegroundColor Yellow

# Check for source file
if (-not (Test-Path "src\$SourceFile")) {
    Write-Host "ERROR: Source file not found: src\$SourceFile" -ForegroundColor Red
    exit 1
}

if (Test-Path $OutputDir) {
    Write-Host "Removing existing output directory..." -ForegroundColor Yellow
    Remove-Item $OutputDir -Recurse -Force
}
Write-Host "Creating output directory: $OutputDir" -ForegroundColor Cyan
New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null

Write-Host "Building SimpleProxyBridge.dll (user-space version)..." -ForegroundColor Green

# Try to find Visual Studio
$compilerFound = $false
$vsWhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"

if (Test-Path $vsWhere) {
    Write-Host "Looking for Visual Studio..." -ForegroundColor Cyan
    $vsPath = & $vsWhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    
    if ($vsPath) {
        Write-Host "Found Visual Studio at: $vsPath" -ForegroundColor Green
        $vcvarsPath = Join-Path $vsPath "VC\Auxiliary\Build\vcvarsall.bat"
        
        if (Test-Path $vcvarsPath) {
            # Compile with MSVC
            $clArgs = "/nologo /O2 /D_CRT_SECURE_NO_WARNINGS /D_WINSOCK_DEPRECATED_NO_WARNINGS /DPROXYBRIDGE_EXPORTS /DNDEBUG " +
                      "src\$SourceFile " +
                      "/LD " +
                      "/link ws2_32.lib iphlpapi.lib psapi.lib " +
                      "/OUT:$OutputDLL"

            $cmd = "`"$vcvarsPath`" $Arch >nul && cl.exe $clArgs"

            Write-Host "Compiling with MSVC: cl.exe $clArgs" -ForegroundColor Gray

            $result = cmd /c $cmd '2>&1'
            $exitCode = $LASTEXITCODE

            if ($exitCode -eq 0) {
                $compilerFound = $true
                Write-Host "MSVC compilation successful!" -ForegroundColor Green
            } else {
                Write-Host "MSVC compilation failed: $result" -ForegroundColor Yellow
            }
        }
    }
}

# Try GCC if MSVC failed
if (-not $compilerFound) {
    $gccCheck = cmd /c "gcc --version 2>&1"
    if ($LASTEXITCODE -eq 0) {
        Write-Host "Found GCC compiler" -ForegroundColor Cyan
        $cmd = "gcc -shared -O2 -s -Wall -D_WIN32_WINNT=0x0601 -DPROXYBRIDGE_EXPORTS src\$SourceFile -lws2_32 -liphlpapi -lpsapi -lkernel32 -o $OutputDLL"
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

if ($compilerFound) {
    # Move DLL to output directory
    if (Test-Path $OutputDLL) {
        Move-Item $OutputDLL -Destination $OutputDir -Force
        Write-Host "`nSimpleProxyBridge.dll created successfully!" -ForegroundColor Green
        Write-Host "Location: $PWD\$OutputDir\$OutputDLL" -ForegroundColor Cyan
        
        # Create a simple windivert.h stub for compatibility
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
        
        $windivertStub | Out-File -FilePath "$OutputDir\windivert.h" -Encoding ASCII
        Write-Host "Created: windivert.h (stub for compatibility)" -ForegroundColor Gray
        
        # Create a dummy WinDivert.dll for compatibility
        $dummyDllSource = '#include <windows.h>
BOOL WINAPI DllMain(HINSTANCE hinstDLL, DWORD fdwReason, LPVOID lpvReserved) {
    return TRUE;
}'
        
        $dummyDllSource | Out-File -FilePath "WinDivert_stub.c" -Encoding ASCII
        $compileDummy = "cl.exe /nologo /LD WinDivert_stub.c /link /OUT:$OutputDir\WinDivert.dll"
        $result = cmd /c $compileDummy '2>&1' 2>$null
        
        if (Test-Path "$OutputDir\WinDivert.dll") {
            Write-Host "Created: WinDivert.dll (dummy stub)" -ForegroundColor Gray
        }
        
        Remove-Item "WinDivert_stub.c" -ErrorAction SilentlyContinue
        Remove-Item "WinDivert_stub.obj" -ErrorAction SilentlyContinue
        Remove-Item "WinDivert_stub.exp" -ErrorAction SilentlyContinue
        Remove-Item "WinDivert_stub.lib" -ErrorAction SilentlyContinue
        
        # Show summary
        Write-Host "`n=== Files created ===" -ForegroundColor Cyan
        Get-ChildItem $OutputDir | ForEach-Object {
            $sizeKB = [math]::Round($_.Length / 1024, 2)
            Write-Host "  $($_.Name) ($sizeKB KB)" -ForegroundColor Gray
        }
        
        Write-Host "`n=== HOW IT WORKS ===" -ForegroundColor Yellow
        Write-Host "1. SimpleProxyBridge.dll uses HTTP_PROXY/SOCKS5_PROXY environment variables" -ForegroundColor White
        Write-Host "2. No kernel-level packet interception (no WinDivert needed)" -ForegroundColor White
        Write-Host "3. Works with applications that respect proxy environment variables" -ForegroundColor White
        Write-Host "4. Runs without administrator privileges" -ForegroundColor White
        
        Write-Host "`n=== HOW TO USE ===" -ForegroundColor Green
        Write-Host "1. Copy files from $OutputDir to your Rust project's target/release/" -ForegroundColor White
        Write-Host "2. Start your Rust application normally" -ForegroundColor White
        Write-Host "3. Use the GUI to select processes and configure proxy" -ForegroundColor White
        Write-Host "4. Selected processes will use proxy via environment variables" -ForegroundColor White
        
        Write-Host "`n=== LIMITATIONS ===" -ForegroundColor Yellow
        Write-Host "- Only works with apps that check HTTP_PROXY/SOCKS5_PROXY" -ForegroundColor Gray
        Write-Host "- No packet-level interception (can't force all traffic)" -ForegroundColor Gray
        Write-Host "- Won't work with apps that ignore proxy settings" -ForegroundColor Gray
        
    } else {
        Write-Host "ERROR: DLL was not created despite successful compilation" -ForegroundColor Red
    }
} else {
    Write-Host "`nERROR: No C compiler found (MSVC or GCC required)" -ForegroundColor Red
    Write-Host "`nPlease install one of:" -ForegroundColor Yellow
    Write-Host "1. Visual Studio Build Tools (with C++ workload)" -ForegroundColor Yellow
    Write-Host "2. MinGW-w64 (GCC for Windows)" -ForegroundColor Yellow
    Write-Host "3. Microsoft C++ Build Tools" -ForegroundColor Yellow
    exit 1
}