# PowerShell script to copy dependencies to src directory before building

$ErrorActionPreference = "Stop"

$workspaceRoot = Split-Path $PSScriptRoot -Parent
$srcDir = $PSScriptRoot

# Create list of files to copy
$filesToCopy = @(
    @{Source = Join-Path $workspaceRoot "third-party\proxybridge-master\Windows\ProxyBridgeCore.dll"; Dest = "embedded_deps\ProxyBridgeCore.dll"},
    @{Source = Join-Path $workspaceRoot "third-party\proxybridge-master\Windows\cli\bin\Release\net10.0-windows\win-x64\native\ProxyBridge_CLI.exe"; Dest = "embedded_deps\ProxyBridge_CLI.exe"},
    @{Source = Join-Path $workspaceRoot "third-party\wireproxy-master\wireproxy.exe"; Dest = "embedded_deps\wireproxy.exe"},
    @{Source = Join-Path $workspaceRoot "third-party\WinDivert-2.2.2-A\x64\WinDivert.dll"; Dest = "embedded_deps\WinDivert.dll"},
    @{Source = Join-Path $workspaceRoot "third-party\WinDivert-2.2.2-A\x64\WinDivert64.sys"; Dest = "embedded_deps\WinDivert64.sys"}
)

# Create directory for embedded dependencies
$embeddedDepsDir = Join-Path $srcDir "embedded_deps"
if (-not (Test-Path $embeddedDepsDir)) {
    New-Item -ItemType Directory -Path $embeddedDepsDir -Force | Out-Null
}

Write-Host "Copying dependency files to $embeddedDepsDir..." -ForegroundColor Green

foreach ($file in $filesToCopy) {
    $source = $file.Source
    $dest = Join-Path $srcDir $file.Dest
    
    if (Test-Path $source) {
        try {
            Copy-Item -Path $source -Destination $dest -Force
            Write-Host "  Copied: $(Split-Path $source -Leaf) -> $($file.Dest)" -ForegroundColor Cyan
        } catch {
            Write-Host "  Error copying $source : $_" -ForegroundColor Red
        }
    } else {
        Write-Host "  File not found: $source" -ForegroundColor Yellow
    }
}

Write-Host "Copying completed." -ForegroundColor Green