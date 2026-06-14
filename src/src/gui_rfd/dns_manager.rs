use super::*;

const DNS_RESTORE_STATE_FILE: &str = "vpnfy_dns_restore_state.txt";
const DNS_APPLY_SCRIPT_FILE: &str = "vpnfy_apply_dns.ps1";
const DNS_RESTORE_SCRIPT_FILE: &str = "vpnfy_restore_dns.ps1";
const FALLBACK_TUNNEL_DNS: [&str; 2] = ["1.1.1.1", "8.8.8.8"];

pub(super) fn tunnel_dns_servers_for_config(conf_path: &str) -> Vec<String> {
    fs::read_to_string(conf_path)
        .map(|content| tunnel_dns_servers_from_content(&content))
        .unwrap_or_else(|_| fallback_tunnel_dns_servers())
}

pub(super) fn apply_tunnel_dns(conf_path: &str) -> Result<Vec<String>, String> {
    let dns_servers = tunnel_dns_servers_for_config(conf_path);
    let state_path = super::managed_cache_dir().join(DNS_RESTORE_STATE_FILE);
    let script_path = super::managed_cache_dir().join(DNS_APPLY_SCRIPT_FILE);
    let script = build_apply_dns_script(&state_path, &dns_servers);

    run_dns_script(&script_path, &script)?;
    Ok(dns_servers)
}

pub(super) fn ensure_tunnel_dns(conf_path: &str) -> Result<Vec<String>, String> {
    let dns_servers = tunnel_dns_servers_for_config(conf_path);
    let state_path = super::managed_cache_dir().join(DNS_RESTORE_STATE_FILE);
    if state_path.exists() {
        return Ok(dns_servers);
    }

    apply_tunnel_dns(conf_path)
}

pub(super) fn restore_tunnel_dns() -> Result<(), String> {
    let state_path = super::managed_cache_dir().join(DNS_RESTORE_STATE_FILE);
    if !state_path.exists() {
        return Ok(());
    }

    let script_path = super::managed_cache_dir().join(DNS_RESTORE_SCRIPT_FILE);
    let script = build_restore_dns_script(&state_path);
    run_dns_script(&script_path, &script)
}

fn tunnel_dns_servers_from_content(config: &str) -> Vec<String> {
    let mut current_section = "";
    let mut dns_servers = Vec::new();

    for raw_line in config.lines() {
        let line = raw_line
            .split(|ch| matches!(ch, '#' | ';'))
            .next()
            .unwrap_or_default()
            .trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            current_section = line.trim_matches(['[', ']']).trim();
            continue;
        }

        if !current_section.eq_ignore_ascii_case("Interface") {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if !key.trim().eq_ignore_ascii_case("DNS") {
            continue;
        }

        for entry in value.split(',').map(str::trim) {
            if is_usable_tunnel_dns_server(entry)
                && !dns_servers.iter().any(|existing| existing == entry)
            {
                dns_servers.push(entry.to_string());
            }
        }
    }

    if dns_servers.is_empty() {
        fallback_tunnel_dns_servers()
    } else {
        dns_servers
    }
}

fn fallback_tunnel_dns_servers() -> Vec<String> {
    FALLBACK_TUNNEL_DNS
        .iter()
        .map(|server| (*server).to_string())
        .collect()
}

fn is_usable_tunnel_dns_server(value: &str) -> bool {
    let Ok(ip) = value.parse::<std::net::Ipv4Addr>() else {
        return false;
    };

    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_multicast()
        || ip.octets()[0] == 198 && matches!(ip.octets()[1], 18 | 19))
}

fn build_apply_dns_script(state_path: &Path, dns_servers: &[String]) -> String {
    let state_path = powershell_single_quoted_path(state_path);
    let servers = powershell_string_array(dns_servers);

    format!(
        r#"$ErrorActionPreference = 'Stop'
$statePath = {state_path}
$servers = @({servers})
$stateDir = Split-Path -Parent $statePath
if (![string]::IsNullOrWhiteSpace($stateDir)) {{
    New-Item -ItemType Directory -Force -Path $stateDir | Out-Null
}}

$interfaces = @(Get-NetIPConfiguration |
    Where-Object {{ $_.IPv4DefaultGateway -ne $null -and $_.NetAdapter.Status -eq 'Up' }} |
    Select-Object -ExpandProperty InterfaceIndex -Unique)

if ($interfaces.Count -eq 0) {{
    $interfaces = @(Get-NetAdapter |
        Where-Object {{ $_.Status -eq 'Up' -and $_.HardwareInterface }} |
        Select-Object -ExpandProperty InterfaceIndex -Unique)
}}

if ($interfaces.Count -eq 0) {{
    throw 'No active network interfaces found for DNS override'
}}

if (!(Test-Path -LiteralPath $statePath)) {{
    $lines = New-Object 'System.Collections.Generic.List[string]'
    $lines.Add('vpnfybot-dns-v4')

    foreach ($index in $interfaces) {{
        $savedServers = @(
            Get-DnsClientServerAddress -InterfaceIndex $index -ErrorAction SilentlyContinue |
                Select-Object -ExpandProperty ServerAddresses
        ) | Where-Object {{ ![string]::IsNullOrWhiteSpace($_) }}

        $lines.Add(('{{0}}|servers|{{1}}' -f $index, ($savedServers -join ',')))

        $adapter = Get-NetAdapter -InterfaceIndex $index -ErrorAction SilentlyContinue
        if ($adapter) {{
            $binding = Get-NetAdapterBinding -Name $adapter.Name -ComponentID ms_tcpip6 -ErrorAction SilentlyContinue
            if ($binding) {{
                $lines.Add(('binding|{{0}}|{{1}}' -f $index, $binding.Enabled))
            }}
        }}
    }}

    Set-Content -LiteralPath $statePath -Encoding UTF8 -Value $lines
}}

foreach ($index in $interfaces) {{
    Set-DnsClientServerAddress -InterfaceIndex $index -ServerAddresses $servers -ErrorAction Stop
}}

foreach ($index in $interfaces) {{
    $adapter = Get-NetAdapter -InterfaceIndex $index -ErrorAction SilentlyContinue
    if ($adapter) {{
        Disable-NetAdapterBinding -Name $adapter.Name -ComponentID ms_tcpip6 -ErrorAction SilentlyContinue
    }}
}}

Clear-DnsClientCache
"#
    )
}

fn build_restore_dns_script(state_path: &Path) -> String {
    let state_path = powershell_single_quoted_path(state_path);

    format!(
        r#"$ErrorActionPreference = 'Stop'
$statePath = {state_path}

if (!(Test-Path -LiteralPath $statePath)) {{
    exit 0
}}

$lines = @(Get-Content -LiteralPath $statePath)
$version = if ($lines.Count -gt 0) {{ $lines[0] }} else {{ '' }}

if ($version -eq 'vpnfybot-dns-v2' -or $version -eq 'vpnfybot-dns-v3' -or $version -eq 'vpnfybot-dns-v4') {{
    if ($version -eq 'vpnfybot-dns-v3') {{
        $prefixLine = $lines | Where-Object {{ $_ -like 'prefixpolicy|*' }} | Select-Object -First 1
        if (![string]::IsNullOrWhiteSpace($prefixLine)) {{
            $prefixParts = $prefixLine -split '\|', 3
            if ($prefixParts.Count -ge 3) {{
                netsh interface ipv6 set prefixpolicy ::ffff:0:0/96 $prefixParts[1] $prefixParts[2] | Out-Null
            }}
        }}
    }}

    if ($version -eq 'vpnfybot-dns-v4') {{
        foreach ($line in $lines | Where-Object {{ $_ -like 'binding|*|True' }}) {{
            $parts = $line -split '\|', 3
            if ($parts.Count -ge 3 -and $parts[1] -match '^\d+$') {{
                $adapter = Get-NetAdapter -InterfaceIndex ([int]$parts[1]) -ErrorAction SilentlyContinue
                if ($adapter) {{
                    Enable-NetAdapterBinding -Name $adapter.Name -ComponentID ms_tcpip6 -ErrorAction SilentlyContinue
                }}
            }}
        }}
    }}

    foreach ($line in $lines | Select-Object -Skip 1) {{
        if ([string]::IsNullOrWhiteSpace($line)) {{
            continue
        }}

        $parts = $line -split '\|', 3
        if ($parts.Count -lt 2) {{
            continue
        }}
        if ($parts[0] -notmatch '^\d+$') {{
            continue
        }}

        $index = [int]$parts[0]
        $savedServers = if ($parts.Count -ge 3) {{ $parts[2] }} else {{ '' }}
        $servers = @(($savedServers -split ',') | Where-Object {{ ![string]::IsNullOrWhiteSpace($_) }})
        if ($servers.Count -gt 0) {{
            Set-DnsClientServerAddress -InterfaceIndex $index -ServerAddresses $servers -ErrorAction Stop
        }} else {{
            Set-DnsClientServerAddress -InterfaceIndex $index -ResetServerAddresses -ErrorAction Stop
        }}
    }}

    if ($version -eq 'vpnfybot-dns-v4') {{
        foreach ($line in $lines | Where-Object {{ $_ -like 'binding|*|False' }}) {{
            $parts = $line -split '\|', 3
            if ($parts.Count -ge 3 -and $parts[1] -match '^\d+$') {{
                $adapter = Get-NetAdapter -InterfaceIndex ([int]$parts[1]) -ErrorAction SilentlyContinue
                if ($adapter) {{
                    Disable-NetAdapterBinding -Name $adapter.Name -ComponentID ms_tcpip6 -ErrorAction SilentlyContinue
                }}
            }}
        }}
    }}
}} else {{
    foreach ($line in $lines | Select-Object -Skip 1) {{
        if ([string]::IsNullOrWhiteSpace($line)) {{
            continue
        }}

        $parts = $line -split '\|', 3
        if ($parts.Count -lt 2) {{
            continue
        }}

        $index = [int]$parts[0]
        $mode = $parts[1]
        $savedServers = if ($parts.Count -ge 3) {{ $parts[2] }} else {{ '' }}

        if ($mode -eq 'static' -and ![string]::IsNullOrWhiteSpace($savedServers)) {{
            $servers = @(($savedServers -split ',') | Where-Object {{ ![string]::IsNullOrWhiteSpace($_) }})
            if ($servers.Count -gt 0) {{
                Set-DnsClientServerAddress -InterfaceIndex $index -ServerAddresses $servers -ErrorAction Stop
            }} else {{
                Set-DnsClientServerAddress -InterfaceIndex $index -ResetServerAddresses -ErrorAction Stop
            }}
        }} else {{
            Set-DnsClientServerAddress -InterfaceIndex $index -ResetServerAddresses -ErrorAction Stop
        }}
    }}
}}

Clear-DnsClientCache
Remove-Item -LiteralPath $statePath -Force -ErrorAction SilentlyContinue
"#
    )
}

fn run_dns_script(script_path: &Path, script: &str) -> Result<(), String> {
    if let Some(parent) = script_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create DNS script directory: {}", error))?;
    }
    fs::write(script_path, script)
        .map_err(|error| format!("Failed to write DNS script: {}", error))?;

    let output = if super::is_elevated() {
        let mut command = std::process::Command::new("powershell");
        command
            .arg("-NoProfile")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-File")
            .arg(script_path);
        hide_command_window(&mut command);
        command
            .output()
            .map_err(|error| format!("Failed to run DNS script: {}", error))?
    } else {
        let command_text = format!(
            "$process = Start-Process -FilePath 'powershell.exe' -ArgumentList @('-NoProfile','-ExecutionPolicy','Bypass','-File',{}) -Verb RunAs -WindowStyle Hidden -PassThru -Wait; exit $process.ExitCode",
            powershell_single_quoted_path(script_path)
        );
        let mut command = std::process::Command::new("powershell");
        command
            .arg("-NoProfile")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-Command")
            .arg(command_text);
        hide_command_window(&mut command);
        command
            .output()
            .map_err(|error| format!("Failed to run elevated DNS script: {}", error))?
    };

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let details = if stderr.is_empty() { stdout } else { stderr };
        Err(if details.is_empty() {
            format!(
                "DNS script failed with code {}",
                output.status.code().unwrap_or(-1)
            )
        } else {
            format!("DNS script failed: {}", details)
        })
    }
}

fn powershell_string_array(values: &[String]) -> String {
    values
        .iter()
        .map(|value| powershell_single_quoted(value))
        .collect::<Vec<_>>()
        .join(", ")
}

fn powershell_single_quoted_path(path: &Path) -> String {
    powershell_single_quoted(&path.to_string_lossy())
}

fn powershell_single_quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn hide_command_window(command: &mut std::process::Command) {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
}
