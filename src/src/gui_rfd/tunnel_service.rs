use super::*;

fn save_config_to_cache(conf_path: &str) {
    let cache_dir = super::managed_cache_dir();

    if let Ok(config_content) = fs::read_to_string(conf_path) {
        let original_name = Path::new(conf_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("wireproxy_config");

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let temp_config_name = format!("{}_wireproxy_{}.conf", original_name, timestamp);
        let temp_config_path = cache_dir.join(&temp_config_name);

        let mut final_config = config_content.clone();
        if !final_config.contains("[Socks5]") {
            if !final_config.ends_with('\n') {
                final_config.push('\n');
            }
            final_config.push('\n');
            final_config.push_str("[Socks5]\n");
            final_config.push_str("BindAddress = 0.0.0.0:1080\n");
        }

        let _ = fs::write(&temp_config_path, final_config);
    }
}

pub(super) fn allocate_wireproxy_info_addr() -> Result<String, String> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("Не удалось выделить локальный порт для метрик wireproxy: {}", e))?;
    let addr = listener
        .local_addr()
        .map_err(|e| format!("Не удалось определить адрес метрик wireproxy: {}", e))?;
    drop(listener);
    Ok(addr.to_string())
}

pub(super) fn fetch_wireproxy_metrics(info_addr: &str) -> Option<String> {
    let socket_addr: SocketAddr = info_addr.parse().ok()?;
    let mut stream = std::net::TcpStream::connect_timeout(&socket_addr, Duration::from_millis(250)).ok()?;
    let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(250)));

    let request = format!(
        "GET /metrics HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        info_addr
    );
    stream.write_all(request.as_bytes()).ok()?;

    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    let (_, body) = response.split_once("\r\n\r\n")?;
    Some(body.to_string())
}

#[allow(dead_code)]
pub(super) fn parse_wireproxy_metrics_total_bytes(metrics: &str) -> Option<u64> {
    let mut total_bytes = 0u64;
    let mut found_counter = false;

    for line in metrics.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };

        if !matches!(key.trim(), "tx_bytes" | "rx_bytes" | "transfer_tx" | "transfer_rx") {
            continue;
        }

        let Ok(bytes) = value.trim().parse::<u64>() else {
            continue;
        };

        total_bytes = total_bytes.saturating_add(bytes);
        found_counter = true;
    }

    found_counter.then_some(total_bytes)
}

pub(super) fn parse_wireproxy_metrics_rx_tx(metrics: &str) -> Option<(u64, u64)> {
    let mut tx_total = 0u64;
    let mut rx_total = 0u64;
    let mut found_counter = false;

    for line in metrics.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };

        let key = key.trim();
        let Ok(bytes) = value.trim().parse::<u64>() else {
            continue;
        };

        match key {
            "tx_bytes" | "transfer_tx" => {
                tx_total = tx_total.saturating_add(bytes);
                found_counter = true;
            }
            "rx_bytes" | "transfer_rx" => {
                rx_total = rx_total.saturating_add(bytes);
                found_counter = true;
            }
            _ => {}
        }
    }

    found_counter.then_some((tx_total, rx_total))
}

pub(super) fn create_and_start_service(conf: &str) -> ServiceResult {
    let config_content = match fs::read_to_string(conf) {
        Ok(content) => content,
        Err(e) => {
            return ServiceResult {
                message: format!("Не удалось прочитать конфиг: {}", e),
                active: false,
                error_log: Some(format!("Ошибка чтения конфига: {}", e)),
                wireproxy_info_addr: None,
            }
        }
    };

    let mut final_config = String::new();

    for line in config_content.lines() {
        let processed_line = if line.starts_with("Address =") {
            if let Some(ipv4_part) = line.split(',').next() {
                ipv4_part.replace("/24", "/32").replace("/25", "/32").replace("/23", "/32").replace("/22", "/32")
            } else {
                line.to_string()
            }
        } else if line.contains("PersistentKeepalive = 0") {
            "PersistentKeepalive = 25".to_string()
        } else {
            line.to_string()
        };

        final_config.push_str(&processed_line);
        final_config.push('\n');
    }

    if !final_config.contains("[Socks5]") {
        final_config.push('\n');
        final_config.push_str("[Socks5]\n");
        final_config.push_str("BindAddress = 0.0.0.0:1080\n");
    }

    let runtime_config_path = super::managed_cache_dir().join("vpnfy_wireproxy_temp.conf");
    if let Err(e) = fs::write(&runtime_config_path, &final_config) {
        return ServiceResult {
            message: format!("Не удалось сохранить конфиг: {}", e),
            active: false,
            error_log: Some(format!("Ошибка сохранения конфига: {}", e)),
            wireproxy_info_addr: None,
        };
    }

    let deps = match embedded_deps_bytes::ExtractedDeps::get() {
        Ok(paths) => paths,
        Err(e) => {
            return ServiceResult {
                message: format!("Не удалось получить зависимости: {}", e),
                active: false,
                error_log: Some(format!("Ошибка получения зависимостей: {}", e)),
                wireproxy_info_addr: None,
            }
        }
    };

    let wireproxy_exe = deps.wireproxy;
    let wireproxy_info_addr = match allocate_wireproxy_info_addr() {
        Ok(addr) => addr,
        Err(e) => {
            return ServiceResult {
                message: e.clone(),
                active: false,
                error_log: Some(e),
                wireproxy_info_addr: None,
            }
        }
    };

    let mut wire_cmd = std::process::Command::new(&wireproxy_exe);
    wire_cmd.arg("-c")
        .arg(runtime_config_path.to_str().unwrap())
        .arg("--info")
        .arg(&wireproxy_info_addr)
        .stdin(std::process::Stdio::null());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        wire_cmd.creation_flags(CREATE_NO_WINDOW);
    }

    match wire_cmd.spawn() {
        Ok(_child) => {
            save_config_to_cache(conf);

            ServiceResult {
                message: format!("Wireproxy запущен для конфига {}", Path::new(conf)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("tunnel")),
                active: true,
                error_log: None,
                wireproxy_info_addr: Some(wireproxy_info_addr),
            }
        }
        Err(e) => {
            ServiceResult {
                message: format!("Не удалось запустить wireproxy: {}", e),
                active: false,
                error_log: Some(format!("Ошибка запуска wireproxy: {}", e)),
                wireproxy_info_addr: None,
            }
        }
    }
}

pub(super) fn stop_and_delete_service(conf: &str) -> ServiceResult {
    let config_path = Path::new(conf).canonicalize().ok().map(|p| p.to_string_lossy().to_string());
    let temp_config_path = super::managed_cache_dir().join("vpnfy_wireproxy_temp.conf").to_string_lossy().to_string();

    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate};
    let mut system = sysinfo::System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::new(),
    );

    let mut killed = false;
    for (pid, process) in system.processes().iter() {
        let proc_name = process.name().to_string_lossy().to_lowercase();
        if proc_name.contains("wireproxy") {
            let has_matching_config = process.cmd().iter().any(|arg| {
                let arg_str = arg.to_string_lossy();
                config_path.as_ref().map_or(false, |cp| arg_str.contains(cp))
                    || arg_str.contains(&temp_config_path)
            });

            if has_matching_config || config_path.is_none() {
                let mut tk = std::process::Command::new("taskkill");
                tk.arg("/PID").arg(pid.to_string()).arg("/F")
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null());
                #[cfg(target_os = "windows")]
                {
                    use std::os::windows::process::CommandExt;
                    const CREATE_NO_WINDOW: u32 = 0x08000000;
                    tk.creation_flags(CREATE_NO_WINDOW);
                }

                if let Ok(output) = tk.output() {
                    if output.status.success() {
                        killed = true;
                    }
                }
            }
        }
    }

    if !killed {
        let mut tk = std::process::Command::new("taskkill");
        tk.arg("/IM").arg("wireproxy.exe").arg("/F").arg("/T")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            tk.creation_flags(CREATE_NO_WINDOW);
        }
        if let Ok(output) = tk.output() {
            if output.status.success() {
                killed = true;
            }
        }
    }

    if killed {
        ServiceResult {
            message: "Wireproxy остановлен".to_string(),
            active: false,
            error_log: None,
            wireproxy_info_addr: None,
        }
    } else {
        ServiceResult {
            message: "Wireproxy не запущен".to_string(),
            active: false,
            error_log: None,
            wireproxy_info_addr: None,
        }
    }
}

pub(super) fn get_running_processes() -> Vec<String> {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::everything()
            .with_exe(UpdateKind::OnlyIfNotSet)
            .with_cmd(UpdateKind::OnlyIfNotSet)
            .with_user(UpdateKind::OnlyIfNotSet),
    );

    let mut processes: Vec<String> = system.processes()
        .values()
        .filter_map(|process| {
            let name = process.name().to_string_lossy().to_string();
            if name.is_empty() || name.starts_with('[') {
                return None;
            }

            let lname = name.to_lowercase();
            if lname == "system" || lname == "system idle process" || lname == "idle" {
                return None;
            }

            let exe_path = process.exe().map(|p| p.to_string_lossy().to_lowercase()).unwrap_or_default();
            if exe_path.starts_with("c:\\windows\\") || exe_path.contains("\\system32\\") || exe_path.contains("\\syswow64\\") {
                return None;
            }

            Some(name)
        })
        .collect();

    if processes.is_empty() {
        if let Ok(output) = std::process::Command::new("tasklist")
            .args(["/FO", "CSV", "/NH"])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                processes = stdout
                    .lines()
                    .filter_map(|line| {
                        let trimmed = line.trim();
                        let first = trimmed.strip_prefix('"')?.split("\",\"").next()?;
                        if first.is_empty() || first.starts_with('[') {
                            return None;
                        }
                        let fname = first.to_string();
                        let lf = fname.to_lowercase();
                        if lf == "system" || lf == "system idle process" || lf == "idle" {
                            return None;
                        }
                        Some(fname)
                    })
                    .collect();
            }
        }
    }

    processes.retain(|p| {
        let lp = p.to_lowercase();
        if lp.starts_with('[') {
            return false;
        }
        if lp == "system" || lp == "system idle process" || lp == "idle" {
            return false;
        }
        true
    });

    processes.sort();
    processes.dedup();
    processes.truncate(100);

    processes
}

fn normalize_site_target(site: &str) -> Option<String> {
    let trimmed = site.trim();
    if trimmed.is_empty() {
        return None;
    }

    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .or_else(|| trimmed.strip_prefix("socks5://"))
        .or_else(|| trimmed.strip_prefix("socks://"))
        .unwrap_or(trimmed);

    let host_port = without_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .trim();

    if host_port.is_empty() {
        return None;
    }

    let without_credentials = host_port
        .rsplit_once('@')
        .map(|(_, value)| value)
        .unwrap_or(host_port);

    let host = if without_credentials.starts_with('[') && without_credentials.ends_with(']') {
        &without_credentials[1..without_credentials.len() - 1]
    } else if without_credentials.matches(':').count() == 1 && !without_credentials.contains("::") {
        without_credentials
            .rsplit_once(':')
            .map(|(value, _)| value)
            .unwrap_or(without_credentials)
    } else {
        without_credentials
    };

    let normalized = host.trim().trim_matches('.').to_ascii_lowercase();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn is_ipv4_filter_pattern(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_digit() || matches!(ch, '.' | '*' | '-' | ';' | ',' | ' '))
}

fn build_site_rules(selected_sites: &[String], action: &str) -> (Vec<String>, Vec<String>) {
    let mut rules = Vec::new();
    let mut unresolved_sites = Vec::new();

    for site in selected_sites {
        let Some(site_target) = normalize_site_target(site) else {
            continue;
        };

        if is_ipv4_filter_pattern(&site_target) {
            let host_filter = site_target
                .split([',', ';', ' '])
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(";");
            if !host_filter.is_empty() {
                rules.push(format!("*:{}:*:BOTH:{}", host_filter, action));
            }
            continue;
        }

        let mut resolved_ips = BTreeSet::new();
        if let Ok(addresses) = (site_target.as_str(), 0).to_socket_addrs() {
            for address in addresses {
                if let SocketAddr::V4(ipv4) = address {
                    resolved_ips.insert(ipv4.ip().to_string());
                }
            }
        }

        if resolved_ips.is_empty() {
            unresolved_sites.push(site_target);
            continue;
        }

        rules.push(format!(
            "*:{}:*:BOTH:{}",
            resolved_ips.into_iter().collect::<Vec<_>>().join(";"),
            action
        ));
    }

    (rules, unresolved_sites)
}

pub(super) fn format_proxybridge_status(process_count: usize, site_count: usize, selected_apps_only: bool, started: bool) -> String {
    let prefix = if started {
        "✅ ProxyBridge запущен"
    } else {
        "Запуск ProxyBridge"
    };

    if selected_apps_only {
        match (process_count, site_count) {
            (0, sites) if sites > 0 => format!("{}: сайты через VPN [{}]", prefix, sites),
            (processes, 0) if processes > 0 => format!("{}: выбранные приложения [{}]", prefix, processes),
            (processes, sites) if processes > 0 && sites > 0 => {
                format!("{}: приложения [{}] и сайты [{}] через VPN", prefix, processes, sites)
            }
            _ => prefix.to_string(),
        }
    } else {
        match (process_count, site_count) {
            (0, 0) => format!("{}: вся система через VPN", prefix),
            (processes, 0) if processes > 0 => format!("{}: исключения процессов [{}]", prefix, processes),
            (0, sites) if sites > 0 => format!("{}: исключения сайтов [{}]", prefix, sites),
            (processes, sites) => format!(
                "{}: исключения процессов [{}] и сайтов [{}]",
                prefix,
                processes,
                sites
            ),
        }
    }
}

pub(super) fn start_proxybridge(processes: &[String], selected_sites: &[String], selected_apps_only: bool) -> Result<Option<std::process::Child>, String> {
    use std::fs::OpenOptions;
    #[cfg(target_os = "windows")]
    use std::os::windows::process::CommandExt;

    if selected_apps_only && processes.is_empty() && selected_sites.is_empty() {
        return Err("Не выбраны процессы для маршрутизации или сайты для VPN".to_string());
    }

    let mut rules: Vec<String> = Vec::new();
    if selected_apps_only {
        let (site_rules, unresolved_sites) = build_site_rules(selected_sites, "PROXY");

        if !processes.is_empty() {
            rules.extend(processes.iter().map(|process| format!("{}:*:*:BOTH:PROXY", process)));
        }

        if !unresolved_sites.is_empty() {
            log::warn!(
                "Не удалось разрешить IPv4 для сайтов через VPN: {}",
                unresolved_sites.join(", ")
            );
        }

        rules.extend(site_rules);

        if rules.is_empty() {
            if !unresolved_sites.is_empty() {
                return Err(format!(
                    "Не удалось разрешить IPv4 для сайтов через VPN: {}",
                    unresolved_sites.join(", ")
                ));
            }
            return Err("Не выбраны процессы для маршрутизации или сайты для VPN".to_string());
        }
    } else {
        let (site_rules, unresolved_sites) = build_site_rules(selected_sites, "DIRECT");

        if !processes.is_empty() {
            rules.extend(processes.iter().map(|process| format!("{}:*:*:BOTH:DIRECT", process)));
        }

        if !unresolved_sites.is_empty() {
            log::warn!(
                "Не удалось разрешить IPv4 для сайтов-исключений из VPN: {}",
                unresolved_sites.join(", ")
            );
        }

        rules.extend(site_rules);
        rules.push("ProxyBridge_CLI.exe:*:*:BOTH:DIRECT".to_string());
        rules.push("wireproxy.exe:*:*:BOTH:DIRECT".to_string());
        rules.push("*:*:*:BOTH:PROXY".to_string());
    }

    let deps = embedded_deps_bytes::ExtractedDeps::get()
        .map_err(|e| format!("Не удалось получить зависимости: {}", e))?;

    let cli_exe = &deps.proxybridge_cli;

    let current_exe = std::env::current_exe()
        .map_err(|_| "Не удалось определить текущий путь".to_string())?;
    let exe_dir = current_exe.parent()
        .ok_or("Не удалось получить директорию приложения".to_string())?;

    let cache_dir = super::managed_cache_dir();
    let log_path = super::managed_logs_dir().join("proxybridge.log");
    let pid_file = cache_dir.join("proxybridge.pid");

    let wait_for_start = |timeout_secs: u64| -> Result<(), String> {
        let start = std::time::Instant::now();
        loop {
            if start.elapsed() > std::time::Duration::from_secs(timeout_secs) {
                if let Ok(s) = std::fs::read_to_string(&log_path) {
                    let tail = if s.len() > 4000 { s[s.len()-4000..].to_string() } else { s };
                    return Err(format!("ProxyBridge не запустился в отведённое время. Лог:\n{}", tail));
                } else {
                    return Err("ProxyBridge не запустился и лог недоступен".to_string());
                }
            }

            if let Ok(s) = std::fs::read_to_string(&log_path) {
                if s.contains("ProxyBridge started") || s.contains("ProxyBridge started.") || s.contains("Local relay:") {
                    return Ok(());
                }
                if s.contains("Failed to open WinDivert") || s.contains("ERROR: Failed to start ProxyBridge") || s.contains("ERROR: ProxyBridge requires Administrator privileges") {
                    let tail = if s.len() > 4000 { s[s.len()-4000..].to_string() } else { s };
                    return Err(format!("ProxyBridge запущен с ошибкой:\n{}", tail));
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    };

    if super::is_elevated() {
        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|e| format!("Не удалось открыть лог файл: {}", e))?;
        let log_file_err = log_file
            .try_clone()
            .map_err(|e| format!("Не удалось клонировать лог файл: {}", e))?;

        let mut cmd = std::process::Command::new(&cli_exe);
        cmd.arg("--proxy")
            .arg("socks5://127.0.0.1:1080")
            .arg("--dns-via-proxy")
            .arg("False")
            .arg("--verbose")
            .arg("3")
            .stdout(std::process::Stdio::from(log_file))
            .stderr(std::process::Stdio::from(log_file_err))
            .current_dir(cli_exe.parent().unwrap_or(&exe_dir))
            .stdin(std::process::Stdio::null());

        #[cfg(target_os = "windows")]
        {
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        for r in &rules {
            cmd.arg("--rule").arg(r);
        }

        let child = cmd
            .spawn()
            .map_err(|e| format!("Не удалось запустить ProxyBridge: {}", e))?;

        wait_for_start(12)?;

        let _ = std::fs::write(pid_file, "running");
        return Ok(Some(child));
    }

    let batch_path = cache_dir.join("run_proxybridge_elevated.bat");
    let mut batch = String::new();
    batch.push_str("@echo off\r\n");
    batch.push_str(&format!("cd /d \"{}\"\r\n", cli_exe.parent().unwrap_or(&cache_dir).display()));
    let mut cmdline = format!("\"{}\" --proxy socks5://127.0.0.1:1080 --dns-via-proxy False --verbose 3",
                              cli_exe.display());
    for r in &rules {
        let safe = r.replace('"', "\\\"");
        cmdline.push_str(&format!(" --rule \"{}\"", safe));
    }
    cmdline.push_str(&format!(" >> \"{}\" 2>&1\r\n", log_path.display()));
    batch.push_str(&cmdline);

    std::fs::write(&batch_path, batch)
        .map_err(|e| format!("Не удалось создать батч-файл для запуска: {}", e))?;

    let ps_cmd = format!("Start-Process -FilePath '{}' -Verb RunAs -WindowStyle Hidden", batch_path.display());
    let mut ps = std::process::Command::new("powershell");
    ps.arg("-NoProfile").arg("-Command").arg(ps_cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .stdin(std::process::Stdio::null());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        ps.creation_flags(CREATE_NO_WINDOW);
    }
    let _ = ps.spawn()
        .map_err(|e| format!("Не удалось запустить PowerShell для запроса UAC: {}", e))?;

    wait_for_start(30)?;

    let _ = std::fs::write(pid_file, "running");
    Ok(None)
}

pub(super) fn stop_proxybridge() -> Result<(), String> {
    let cache_dir = super::managed_cache_dir();

    let pid_file = cache_dir.join("proxybridge.pid");
    if !pid_file.exists() {
        return Err("ProxyBridge не запущен (файл маркера не найден)".to_string());
    }

    let _ = std::fs::remove_file(&pid_file);

    let mut stop_cmd = std::process::Command::new("powershell");
    stop_cmd.args(["-Command", "Stop-Process -Name 'ProxyBridge_CLI' -Force -ErrorAction SilentlyContinue"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        stop_cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let _stop_result = stop_cmd
        .output()
        .map_err(|e| format!("Не удалось остановить ProxyBridge процессы: {}", e))?;

    let mut tk2 = std::process::Command::new("taskkill");
    tk2.arg("/IM").arg("ProxyBridge_CLI.exe").arg("/F")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        tk2.creation_flags(CREATE_NO_WINDOW);
    }
    let _ = tk2.output();

    std::thread::sleep(std::time::Duration::from_millis(500));

    let processes_still_running = check_proxybridge_processes();

    if processes_still_running {
        let mut ps_kill = std::process::Command::new("powershell");
        ps_kill.args(["-Command", "Get-Process -Name 'ProxyBridge_CLI' -ErrorAction SilentlyContinue | ForEach-Object { try { Stop-Process -Id $_.Id -Force -ErrorAction Stop } catch { Write-Error \"Failed to stop process $($_.Id): $_\" } }"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            ps_kill.creation_flags(CREATE_NO_WINDOW);
        }
        let _ = ps_kill.output();

        std::thread::sleep(std::time::Duration::from_millis(800));

        if check_proxybridge_processes() {
            let mut wmic_cmd = std::process::Command::new("wmic");
            wmic_cmd.args(["process", "where", "name='ProxyBridge_CLI.exe'", "delete"])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            #[cfg(target_os = "windows")]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x08000000;
                wmic_cmd.creation_flags(CREATE_NO_WINDOW);
            }
            let _ = wmic_cmd.output();

            std::thread::sleep(std::time::Duration::from_millis(500));

            if check_proxybridge_processes() {
                return Err("Не удалось остановить все процессы ProxyBridge_CLI.exe".to_string());
            }
        }
    }

    Ok(())
}

fn check_proxybridge_processes() -> bool {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let mut ps_check = std::process::Command::new("powershell");
        ps_check.args(["-Command", "Get-Process -Name 'ProxyBridge_CLI' -ErrorAction SilentlyContinue | Select-Object -First 1"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        ps_check.creation_flags(CREATE_NO_WINDOW);

        if let Ok(output) = ps_check.output() {
            if output.status.success() {
                let output_str = String::from_utf8_lossy(&output.stdout);
                return output_str.contains("ProxyBridge_CLI");
            }
        }
    }

    false
}

pub(super) fn kill_existing_processes() {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let mut wireproxy_kill = std::process::Command::new("taskkill");
        wireproxy_kill.arg("/IM").arg("wireproxy.exe").arg("/F").arg("/T")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        wireproxy_kill.creation_flags(CREATE_NO_WINDOW);
        let _ = wireproxy_kill.output();

        std::thread::sleep(std::time::Duration::from_millis(300));

        let mut proxybridge_kill = std::process::Command::new("taskkill");
        proxybridge_kill.arg("/IM").arg("ProxyBridge_CLI.exe").arg("/F").arg("/T")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        proxybridge_kill.creation_flags(CREATE_NO_WINDOW);
        let _ = proxybridge_kill.output();

        std::thread::sleep(std::time::Duration::from_millis(500));

        let mut ps_kill = std::process::Command::new("powershell");
        ps_kill.args(["-Command", "Get-Process -Name 'ProxyBridge_CLI' -ErrorAction SilentlyContinue | ForEach-Object { try { Stop-Process -Id $_.Id -Force -ErrorAction Stop } catch { Write-Error \"Failed to stop process $($_.Id): $_\" } }"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        ps_kill.creation_flags(CREATE_NO_WINDOW);
        let _ = ps_kill.output();

        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}