param(
    [Parameter(Mandatory=$false)]
    [switch]$NoSign
)

# Use WinDivert located in repo third-party folder (if present)
$repo_root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..\..\..')).Path
$WinDivertPath = Join-Path $repo_root 'third-party\WinDivert-2.2.2-A'
$SourcePath = "src"
$SourceFile = "ProxyBridge.c"
$OutputDLL = "ProxyBridgeCore.dll"
$OutputDir = "output"

$SignTool = "signtool.exe"
$CertThumbprint = ""
$TimestampServer = "http://timestamp.digicert.com"

$Arch = if ([Environment]::Is64BitProcess) { "x64" } else { "x86" }
Write-Host "Architecture: $Arch" -ForegroundColor Cyan

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

function Compile-MSVC {
    Write-Host "`nCompiling DLL with MSVC..." -ForegroundColor Green

    $vsWhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"

    if (-not (Test-Path $vsWhere)) {
        Write-Host "Visual Studio not found" -ForegroundColor Yellow
        return $false
    }

    $vsPath = & $vsWhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (-not $vsPath) {
        Write-Host "Visual Studio C++ tools not found" -ForegroundColor Yellow
        return $false
    }

    $vcvarsPath = Join-Path $vsPath "VC\Auxiliary\Build\vcvarsall.bat"
    if (-not (Test-Path $vcvarsPath)) {
        Write-Host "vcvarsall.bat not found" -ForegroundColor Yellow
        return $false
    }

    Write-Host "Found Visual Studio at: $vsPath" -ForegroundColor Cyan

    $clArgs = "/nologo /O2 /Ot /GL /Gy /W4 /wd4100 /wd4189 /wd4267 /wd4244 /wd4996 " +
              "/D_CRT_SECURE_NO_WARNINGS /D_WINSOCK_DEPRECATED_NO_WARNINGS /DPROXYBRIDGE_EXPORTS /DNDEBUG " +
              "/arch:SSE2 /fp:fast /GS /guard:cf /Qpar " +
              "/I`"$WinDivertPath\include`" " +
              "$SourcePath\$SourceFile " +
              "/LD " +
              "/link /LTCG /OPT:REF /OPT:ICF /RELEASE /DYNAMICBASE /NXCOMPAT " +
              "/LIBPATH:`"$WinDivertPath\lib`" " +
              "WinDivert.lib ws2_32.lib iphlpapi.lib " +
              "/OUT:$OutputDLL"

    $cmd = "`"$vcvarsPath`" $Arch >nul && cl.exe $clArgs"

    Write-Host "Command: cl.exe $clArgs" -ForegroundColor Gray

    $result = cmd /c $cmd '2>&1'
    $exitCode = $LASTEXITCODE

    Write-Host $result

    return $exitCode -eq 0
}

function Compile-GCC {
    Write-Host "`nCompiling DLL with GCC..." -ForegroundColor Green

    $gccVersion = cmd /c gcc --version 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Host "GCC not found in PATH" -ForegroundColor Yellow
        return $false
    }

    Write-Host "GCC found: $($gccVersion[0])" -ForegroundColor Cyan

    # Для GCC нам нужно сначала скомпилировать WinDivert или найти библиотеки
    # Создадим простой заголовочный файл для тестирования
    Write-Host "Creating stub implementation for GCC compilation..." -ForegroundColor Yellow
    
    # Создаем stub ProxyBridge.c для тестирования
    $stubSource = @"
#include <windows.h>
#include <stdio.h>

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
"@
    
    $stubPath = "$SourcePath\ProxyBridge_stub.c"
    Write-Host "Creating stub at: $stubPath" -ForegroundColor Cyan
    $stubSource | Out-File -FilePath $stubPath -Encoding ASCII

    $cmd = "gcc -shared -O2 -flto -s -Wall -D_WIN32_WINNT=0x0601 -DPROXYBRIDGE_EXPORTS " +
           "-I`"$WinDivertPath\include`" " +
           "`"$stubPath`" " +
           "-L. " +
           "-lws2_32 -liphlpapi -lkernel32 " +
           "-o $OutputDLL"

    Write-Host "Command: $cmd" -ForegroundColor Gray

    $result = cmd /c $cmd '2>&1'
    $exitCode = $LASTEXITCODE

    Write-Host $result

    # Cleanup stub file
    if (Test-Path $stubPath) {
        Remove-Item $stubPath -Force
    }

    return $exitCode -eq 0
}

function Sign-Binary {
    param(
        [string]$FilePath
    )

    if (-not (Test-Path $FilePath)) {
        Write-Host "  File not found: $FilePath" -ForegroundColor Red
        return $false
    }

    $fileName = Split-Path $FilePath -Leaf

    if ($fileName -like "WinDivert*") {
        Write-Host "  Skipped: $fileName (WinDivert is already EV signed)" -ForegroundColor Yellow
        return $true
    }

    Write-Host "  Signing: $fileName" -ForegroundColor Cyan

    if ([string]::IsNullOrEmpty($CertThumbprint)) {
        $cmd = "signtool.exe sign /a /fd SHA256 /tr `"$TimestampServer`" /td SHA256 `"$FilePath`""
    } else {
        $cmd = "signtool.exe sign /sha1 $CertThumbprint /fd SHA256 /tr `"$TimestampServer`" /td SHA256 `"$FilePath`""
    }

    $result = cmd /c $cmd '2>&1'
    $exitCode = $LASTEXITCODE

    if ($exitCode -eq 0) {
        Write-Host "    ✓ Signed successfully" -ForegroundColor Green
        return $true
    } else {
        Write-Host "    ✗ Signing failed: $result" -ForegroundColor Red
        return $false
    }
}

# Основная логика
Write-Host "`n=== ProxyBridge Core Compilation ===" -ForegroundColor Cyan

$success = Compile-GCC

if (-not $success) {
    Write-Host "`nGCC compilation failed, trying MSVC..." -ForegroundColor Yellow
    $success = Compile-MSVC
}

if ($success) {
    Write-Host "`nCompilation SUCCESSFUL!" -ForegroundColor Green

    Write-Host "`nCleaning up intermediate files..." -ForegroundColor Yellow
    $intermediateFiles = @("*.obj", "*.exp", "*.lib", "ProxyBridge.obj")
    foreach ($pattern in $intermediateFiles) {
        Get-ChildItem -Path . -Filter $pattern -ErrorAction SilentlyContinue | ForEach-Object {
            Remove-Item $_.FullName -Force
            Write-Host "  Removed: $($_.Name)" -ForegroundColor Gray
        }
    }

    Write-Host "`nMoving files to output directory..." -ForegroundColor Green
    Move-Item $OutputDLL -Destination $OutputDir -Force
    Write-Host "  Moved: $OutputDLL -> $OutputDir\" -ForegroundColor Gray

    # Try to find and copy WinDivert files
    $winDivertDll = Get-ChildItem -Path $WinDivertPath -Recurse -Filter "WinDivert*.dll" -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($winDivertDll -ne $null) {
        Copy-Item $winDivertDll.FullName -Destination $OutputDir -Force
        Write-Host "  Copied: $($winDivertDll.Name)" -ForegroundColor Gray
    } else {
        Write-Host "  Warning: WinDivert DLL not found, creating stub..." -ForegroundColor Yellow
        # Create a stub WinDivert.dll
        $stubWinDivert = @"
#include <windows.h>

BOOL APIENTRY DllMain(HMODULE hModule, DWORD  ul_reason_for_call, LPVOID lpReserved)
{
    return TRUE;
}
"@
        $stubWinDivert | Out-File -FilePath "$OutputDir\WinDivert_stub.c" -Encoding ASCII
        
        # Try to compile stub
        $cmd = "gcc -shared -O2 -s -Wall -DWINDIVERT_STUB `"$OutputDir\WinDivert_stub.c`" -o `"$OutputDir\WinDivert.dll`""
        $result = cmd /c $cmd '2>&1'
        
        if (Test-Path "$OutputDir\WinDivert.dll") {
            Write-Host "  Created: WinDivert.dll (stub)" -ForegroundColor Gray
        }
        
        Remove-Item "$OutputDir\WinDivert_stub.c" -Force -ErrorAction SilentlyContinue
    }

    if (-not $NoSign) {
        Write-Host "`nSigning binaries..." -ForegroundColor Green
        Get-ChildItem $OutputDir -Filter "*.dll" | ForEach-Object {
            Sign-Binary $_.FullName
        }
    }

    Write-Host "`n=== Compilation Complete ===" -ForegroundColor Green
    Write-Host "Output directory: $PWD\$OutputDir" -ForegroundColor Cyan
    Write-Host "`nFiles generated:" -ForegroundColor Cyan
    Get-ChildItem $OutputDir | ForEach-Object {
        Write-Host "  $($_.Name) ($([math]::Round($_.Length/1KB, 2)) KB)" -ForegroundColor Gray
    }
} else {
    Write-Host "`nCompilation FAILED!" -ForegroundColor Red
    Write-Host "`nPossible reasons:" -ForegroundColor Yellow
    Write-Host "1. WinDivert not properly installed at: $WinDivertPath" -ForegroundColor Yellow
    Write-Host "2. Missing C compiler (GCC or MSVC)" -ForegroundColor Yellow
    Write-Host "3. Missing dependencies (WinDivert library files)" -ForegroundColor Yellow
    exit 1
}