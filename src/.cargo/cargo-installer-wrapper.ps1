Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public static class VpnfyCommandLineParser
{
    [DllImport("shell32.dll", SetLastError = true)]
    public static extern IntPtr CommandLineToArgvW(
        [MarshalAs(UnmanagedType.LPWStr)] string commandLine,
        out int argumentCount);

    [DllImport("kernel32.dll")]
    public static extern IntPtr LocalFree(IntPtr memory);
}
"@

$commandLine = $env:VPNFY_RUSTC_COMMAND
if (-not $commandLine) {
    Write-Error "rustc wrapper did not receive a command line"
    exit 1
}

$argumentCount = 0
$argumentPointer = [VpnfyCommandLineParser]::CommandLineToArgvW(
    $commandLine,
    [ref]$argumentCount
)
if ($argumentPointer -eq [IntPtr]::Zero) {
    Write-Error "failed to parse the rustc command line"
    exit 1
}

$WrapperArguments = @()
try {
    for ($index = 0; $index -lt $argumentCount; $index++) {
        $valuePointer = [Runtime.InteropServices.Marshal]::ReadIntPtr(
            $argumentPointer,
            $index * [IntPtr]::Size
        )
        $WrapperArguments += [Runtime.InteropServices.Marshal]::PtrToStringUni($valuePointer)
    }
} finally {
    [void][VpnfyCommandLineParser]::LocalFree($argumentPointer)
}

if ($WrapperArguments.Count -lt 1) {
    Write-Error "rustc wrapper did not receive the rustc executable"
    exit 1
}

$rustc = $WrapperArguments[0]
$rustcArguments = @(
    if ($WrapperArguments.Count -gt 1) {
        $WrapperArguments[1..($WrapperArguments.Count - 1)]
    }
)

& $rustc @rustcArguments
$rustcExitCode = $LASTEXITCODE
if ($rustcExitCode -ne 0) {
    exit $rustcExitCode
}

if ($env:VPNFY_SKIP_AUTO_INSTALLER -eq "1") {
    exit 0
}

$crateName = $null
$crateType = $null
$emitKinds = @()
$outDir = $null
for ($index = 0; $index -lt $rustcArguments.Count; $index++) {
    if ($rustcArguments[$index] -like "--emit=*") {
        $emitKinds = $rustcArguments[$index].Substring(7).Split(",")
        continue
    }
    if ($index -ge $rustcArguments.Count - 1) {
        continue
    }
    switch ($rustcArguments[$index]) {
        "--crate-name" { $crateName = $rustcArguments[$index + 1] }
        "--crate-type" { $crateType = $rustcArguments[$index + 1] }
        "--out-dir" { $outDir = $rustcArguments[$index + 1] }
    }
}

$isReleaseOutput = $outDir -and
    (Split-Path -Leaf $outDir) -eq "deps" -and
    (Split-Path -Leaf (Split-Path -Parent $outDir)) -eq "release"
$isTargetBinary = $crateName -eq "vpnfybot_windows" -and
    $crateType -eq "bin" -and
    $emitKinds -contains "link" -and
    -not ($rustcArguments -contains "--test")

if (-not ($isReleaseOutput -and $isTargetBinary)) {
    exit 0
}

$binaryPath = Join-Path $outDir "vpnfybot_windows.exe"
if (-not (Test-Path -LiteralPath $binaryPath)) {
    $binaryPath = Get-ChildItem -LiteralPath $outDir -Filter "vpnfybot_windows*.exe" |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1 -ExpandProperty FullName
}
if (-not $binaryPath -or -not (Test-Path -LiteralPath $binaryPath)) {
    Write-Error "release binary was linked, but its output file was not found"
    exit 1
}

$packageScript = Join-Path (Split-Path -Parent $PSScriptRoot) "build-installer-release.bat"
& $packageScript --package-only $binaryPath
exit $LASTEXITCODE
