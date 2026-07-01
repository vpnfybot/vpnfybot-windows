use super::*;

const PROCESS_LIST_CACHE_TTL: Duration = Duration::from_secs(5);
const PROCESS_EXIT_WAIT_TIMEOUT: Duration = Duration::from_secs(2);
const PROCESS_EXIT_POLL_INTERVAL: Duration = Duration::from_millis(100);
const ELEVATED_HELPER_WAIT_TIMEOUT: Duration = Duration::from_secs(20);
const WIREPROXY_START_WAIT_TIMEOUT: Duration = Duration::from_secs(20);
const PROXYBRIDGE_START_WAIT_TIMEOUT: Duration = Duration::from_secs(20);
const PROXYBRIDGE_ELEVATED_START_WAIT_TIMEOUT: Duration = Duration::from_secs(30);
const PROXYBRIDGE_START_POLL_INTERVAL: Duration = Duration::from_millis(200);
const PROXYBRIDGE_START_SCAN_WINDOW_BYTES: usize = 1024;
const PROXYBRIDGE_START_DIAGNOSTIC_BYTES: usize = 16 * 1024;

static PROCESS_LIST_CACHE: OnceLock<Mutex<Option<ProcessListCache>>> = OnceLock::new();
static PROCESS_LIST_REFRESH_RUNNING: AtomicBool = AtomicBool::new(false);

struct ProcessListCache {
    processes: Vec<String>,
    refreshed_at: Instant,
}

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
    let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| {
        format!(
            "Не удалось выделить локальный порт для метрик wireproxy: {}",
            e
        )
    })?;
    let addr = listener
        .local_addr()
        .map_err(|e| format!("Не удалось определить адрес метрик wireproxy: {}", e))?;
    drop(listener);
    Ok(addr.to_string())
}

pub(super) fn fetch_wireproxy_metrics(info_addr: &str) -> Option<String> {
    let socket_addr: SocketAddr = info_addr.parse().ok()?;
    let mut stream =
        std::net::TcpStream::connect_timeout(&socket_addr, Duration::from_millis(250)).ok()?;
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

        if !matches!(
            key.trim(),
            "tx_bytes" | "rx_bytes" | "transfer_tx" | "transfer_rx"
        ) {
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

fn enumerate_running_processes() -> Vec<String> {
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

    let mut processes: Vec<String> = system
        .processes()
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

            let exe_path = process
                .exe()
                .map(|path| path.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if exe_path.starts_with("c:\\windows\\")
                || exe_path.contains("\\system32\\")
                || exe_path.contains("\\syswow64\\")
            {
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

    processes.retain(|process_name| {
        let lower = process_name.to_lowercase();
        if lower.starts_with('[') {
            return false;
        }
        if lower == "system" || lower == "system idle process" || lower == "idle" {
            return false;
        }
        true
    });

    processes.sort();
    processes.dedup();
    processes.truncate(100);
    processes
}

fn process_list_cache() -> &'static Mutex<Option<ProcessListCache>> {
    PROCESS_LIST_CACHE.get_or_init(|| Mutex::new(None))
}

fn store_running_processes(processes: &[String]) {
    if let Ok(mut guard) = process_list_cache().lock() {
        *guard = Some(ProcessListCache {
            processes: processes.to_vec(),
            refreshed_at: Instant::now(),
        });
    }
}

fn get_cached_running_processes() -> Option<Vec<String>> {
    let guard = process_list_cache().lock().ok()?;
    let cache = guard.as_ref()?;
    if cache.refreshed_at.elapsed() <= PROCESS_LIST_CACHE_TTL {
        Some(cache.processes.clone())
    } else {
        None
    }
}

pub(super) fn refresh_running_processes_async() {
    if PROCESS_LIST_REFRESH_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }

    thread::spawn(|| {
        struct ResetFlag;

        impl Drop for ResetFlag {
            fn drop(&mut self) {
                PROCESS_LIST_REFRESH_RUNNING.store(false, Ordering::SeqCst);
            }
        }

        let _reset_flag = ResetFlag;
        let processes = enumerate_running_processes();
        store_running_processes(&processes);
    });
}

fn process_name_matches(process: &sysinfo::Process, expected_name: &str) -> bool {
    let actual = process.name().to_string_lossy().to_ascii_lowercase();
    let normalized_expected = expected_name.trim_end_matches(".exe").to_ascii_lowercase();

    actual == normalized_expected
        || actual == format!("{}.exe", normalized_expected)
        || actual.contains(&normalized_expected)
}

fn refresh_processes_for_matching(system: &mut sysinfo::System) {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, UpdateKind};

    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::new().with_cmd(UpdateKind::OnlyIfNotSet),
    );
}

fn kill_processes_matching<F>(mut predicate: F) -> bool
where
    F: FnMut(&sysinfo::Process) -> bool,
{
    let mut system = sysinfo::System::new();
    refresh_processes_for_matching(&mut system);

    let mut killed_any = false;
    for process in system.processes().values() {
        if predicate(process) {
            let _ = process.kill();
            killed_any = true;
        }
    }

    killed_any
}

fn any_process_matches<F>(mut predicate: F) -> bool
where
    F: FnMut(&sysinfo::Process) -> bool,
{
    let mut system = sysinfo::System::new();
    refresh_processes_for_matching(&mut system);

    system
        .processes()
        .values()
        .any(|process| predicate(process))
}

fn wait_until_processes_exit<F>(predicate: F, timeout: Duration) -> bool
where
    F: FnMut(&sysinfo::Process) -> bool + Copy,
{
    let started = Instant::now();
    let mut system = sysinfo::System::new();
    loop {
        refresh_processes_for_matching(&mut system);
        if !system.processes().values().any(predicate) {
            return true;
        }

        if started.elapsed() >= timeout {
            return false;
        }

        thread::sleep(PROCESS_EXIT_POLL_INTERVAL);
    }
}

fn fallback_taskkill_image(image_name: &str) {
    let mut taskkill = std::process::Command::new("taskkill");
    taskkill
        .arg("/IM")
        .arg(image_name)
        .arg("/F")
        .arg("/T")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        taskkill.creation_flags(CREATE_NO_WINDOW);
    }

    let _ = taskkill.output();
}

fn read_log_since(path: &Path, offset: &mut u64) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};

    let mut file = std::fs::File::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    if length < *offset {
        *offset = 0;
    }

    file.seek(SeekFrom::Start(*offset)).ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    *offset = file.stream_position().unwrap_or(length);
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn append_bounded_log(buffer: &mut String, chunk: &str, max_bytes: usize) {
    buffer.push_str(chunk);
    if buffer.len() <= max_bytes {
        return;
    }

    let mut keep_from = buffer.len() - max_bytes;
    while !buffer.is_char_boundary(keep_from) {
        keep_from += 1;
    }
    buffer.drain(..keep_from);
}

fn proxybridge_start_succeeded(output: &str) -> bool {
    output.contains("ProxyBridge started")
        || output.contains("ProxyBridge started.")
        || output.contains("Local relay:")
}

fn proxybridge_start_failed(output: &str) -> bool {
    output.contains("Failed to open WinDivert")
        || output.contains("ERROR: Failed to start ProxyBridge")
        || output.contains("ERROR: ProxyBridge requires Administrator privileges")
}

fn proxybridge_start_error(message: &str, output: &str) -> String {
    let output = output.trim();
    if output.is_empty() {
        message.to_string()
    } else {
        format!("{}. Лог текущей попытки:\n{}", message, output)
    }
}

fn wait_for_proxybridge_start(
    log_path: &Path,
    initial_log_offset: u64,
    timeout: Duration,
    mut child: Option<&mut std::process::Child>,
) -> Result<(), String> {
    let started_at = Instant::now();
    let mut log_offset = initial_log_offset;
    let mut scan_window = String::new();
    let mut diagnostic_log = String::new();

    loop {
        if let Some(chunk) = read_log_since(log_path, &mut log_offset) {
            scan_window.push_str(&chunk);
            append_bounded_log(
                &mut diagnostic_log,
                &chunk,
                PROXYBRIDGE_START_DIAGNOSTIC_BYTES,
            );

            if proxybridge_start_succeeded(&scan_window) {
                if let Some(process) = child.as_deref_mut() {
                    if let Ok(Some(status)) = process.try_wait() {
                        return Err(proxybridge_start_error(
                            &format!("ProxyBridge завершился сразу после запуска ({})", status),
                            &diagnostic_log,
                        ));
                    }
                }
                return Ok(());
            }

            if proxybridge_start_failed(&scan_window) {
                return Err(proxybridge_start_error(
                    "ProxyBridge запущен с ошибкой",
                    &diagnostic_log,
                ));
            }

            if scan_window.len() > PROXYBRIDGE_START_SCAN_WINDOW_BYTES {
                let mut keep_from = scan_window.len() - PROXYBRIDGE_START_SCAN_WINDOW_BYTES;
                while !scan_window.is_char_boundary(keep_from) {
                    keep_from += 1;
                }
                scan_window.drain(..keep_from);
            }
        }

        if let Some(process) = child.as_deref_mut() {
            if let Ok(Some(status)) = process.try_wait() {
                return Err(proxybridge_start_error(
                    &format!("ProxyBridge завершился до окончания запуска ({})", status),
                    &diagnostic_log,
                ));
            }
        }

        if started_at.elapsed() >= timeout {
            return Err(proxybridge_start_error(
                "ProxyBridge не запустился в отведённое время",
                &diagnostic_log,
            ));
        }

        thread::sleep(PROXYBRIDGE_START_POLL_INTERVAL);
    }
}

fn wait_for_wireproxy_start(info_addr: &str, timeout: Duration) -> Result<(), String> {
    let started = Instant::now();

    loop {
        if fetch_wireproxy_metrics(info_addr).is_some() {
            return Ok(());
        }

        if started.elapsed() >= timeout {
            return Err("Wireproxy не запустился в отведённое время".to_string());
        }

        thread::sleep(Duration::from_millis(200));
    }
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
                ipv4_part
                    .replace("/24", "/32")
                    .replace("/25", "/32")
                    .replace("/23", "/32")
                    .replace("/22", "/32")
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

    if !super::is_elevated() {
        let launch_result = super::app_runtime::launch_self_elevated(&[
            OsString::from("/service"),
            runtime_config_path.as_os_str().to_os_string(),
            OsString::from(&wireproxy_info_addr),
        ]);

        if let Err(e) = launch_result {
            return ServiceResult {
                message: e.clone(),
                active: false,
                error_log: Some(e),
                wireproxy_info_addr: None,
            };
        }

        if let Err(e) = wait_for_wireproxy_start(&wireproxy_info_addr, WIREPROXY_START_WAIT_TIMEOUT)
        {
            return ServiceResult {
                message: e.clone(),
                active: false,
                error_log: Some(e),
                wireproxy_info_addr: None,
            };
        }

        save_config_to_cache(conf);

        return ServiceResult {
            message: format!(
                "Wireproxy запущен для конфига {}",
                Path::new(conf)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("tunnel")
            ),
            active: true,
            error_log: None,
            wireproxy_info_addr: Some(wireproxy_info_addr),
        };
    }

    let mut wire_cmd = std::process::Command::new(&wireproxy_exe);
    wire_cmd
        .arg("-c")
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
        Ok(mut child) => {
            if let Err(e) =
                wait_for_wireproxy_start(&wireproxy_info_addr, WIREPROXY_START_WAIT_TIMEOUT)
            {
                let _ = child.kill();
                let _ = child.wait();
                return ServiceResult {
                    message: e.clone(),
                    active: false,
                    error_log: Some(e),
                    wireproxy_info_addr: None,
                };
            }

            save_config_to_cache(conf);

            ServiceResult {
                message: format!(
                    "Wireproxy запущен для конфига {}",
                    Path::new(conf)
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("tunnel")
                ),
                active: true,
                error_log: None,
                wireproxy_info_addr: Some(wireproxy_info_addr),
            }
        }
        Err(e) => ServiceResult {
            message: format!("Не удалось запустить wireproxy: {}", e),
            active: false,
            error_log: Some(format!("Ошибка запуска wireproxy: {}", e)),
            wireproxy_info_addr: None,
        },
    }
}

pub(super) fn stop_and_delete_service(conf: &str) -> ServiceResult {
    let config_path = Path::new(conf)
        .canonicalize()
        .ok()
        .map(|p| p.to_string_lossy().to_string());
    let temp_config_path = super::managed_cache_dir()
        .join("vpnfy_wireproxy_temp.conf")
        .to_string_lossy()
        .to_string();

    let matches_target_process = |process: &sysinfo::Process| {
        if !process_name_matches(process, "wireproxy.exe") {
            return false;
        }

        let has_matching_config = process.cmd().iter().any(|arg| {
            let arg_str = arg.to_string_lossy();
            config_path
                .as_ref()
                .map_or(false, |cp| arg_str.contains(cp))
                || arg_str.contains(&temp_config_path)
        });

        has_matching_config || config_path.is_none()
    };

    if !any_process_matches(matches_target_process) {
        return ServiceResult {
            message: "Wireproxy не запущен".to_string(),
            active: false,
            error_log: None,
            wireproxy_info_addr: None,
        };
    }

    if !super::is_elevated() {
        let launch_result = super::app_runtime::launch_self_elevated(&[
            OsString::from("/stop-service"),
            OsString::from(conf),
        ]);

        if let Err(e) = launch_result {
            return ServiceResult {
                message: e.clone(),
                active: true,
                error_log: Some(e),
                wireproxy_info_addr: None,
            };
        }

        if wait_until_processes_exit(matches_target_process, ELEVATED_HELPER_WAIT_TIMEOUT) {
            return ServiceResult {
                message: "Wireproxy остановлен".to_string(),
                active: false,
                error_log: None,
                wireproxy_info_addr: None,
            };
        }

        return ServiceResult {
            message: "Не удалось остановить wireproxy через elevated helper".to_string(),
            active: true,
            error_log: Some("Не удалось остановить wireproxy через elevated helper".to_string()),
            wireproxy_info_addr: None,
        };
    }

    let mut killed = kill_processes_matching(matches_target_process);

    if killed {
        let _ = wait_until_processes_exit(matches_target_process, PROCESS_EXIT_WAIT_TIMEOUT);
    }

    if any_process_matches(matches_target_process) {
        fallback_taskkill_image("wireproxy.exe");
        killed = true;
        let _ = wait_until_processes_exit(matches_target_process, PROCESS_EXIT_WAIT_TIMEOUT);
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
    if let Some(processes) = get_cached_running_processes() {
        return processes;
    }

    let processes = enumerate_running_processes();
    store_running_processes(&processes);
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

fn resolve_site_rule_targets(selected_sites: &[String]) -> (Vec<String>, Vec<String>) {
    let mut targets = Vec::new();
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
                targets.push(host_filter);
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

        targets.push(resolved_ips.into_iter().collect::<Vec<_>>().join(";"));
    }

    (targets, unresolved_sites)
}

fn format_site_rules(targets: &[String], ports: &str, protocol: &str, action: &str) -> Vec<String> {
    targets
        .iter()
        .map(|target| format!("*:{}:{}:{}:{}", target, ports, protocol, action))
        .collect()
}

fn build_site_rules_with_options(
    selected_sites: &[String],
    ports: &str,
    protocol: &str,
    action: &str,
) -> (Vec<String>, Vec<String>) {
    let (targets, unresolved_sites) = resolve_site_rule_targets(selected_sites);
    (
        format_site_rules(&targets, ports, protocol, action),
        unresolved_sites,
    )
}

fn build_site_rules(selected_sites: &[String], action: &str) -> (Vec<String>, Vec<String>) {
    build_site_rules_with_options(selected_sites, "*", "BOTH", action)
}

pub(super) fn format_proxybridge_status(
    process_count: usize,
    site_count: usize,
    selected_apps_only: bool,
    started: bool,
) -> String {
    let prefix = if started {
        "✅ ProxyBridge запущен"
    } else {
        "Запуск ProxyBridge"
    };

    if selected_apps_only {
        match (process_count, site_count) {
            (0, sites) if sites > 0 => format!("{}: сайты через VPN [{}]", prefix, sites),
            (processes, 0) if processes > 0 => {
                format!("{}: выбранные приложения [{}]", prefix, processes)
            }
            (processes, sites) if processes > 0 && sites > 0 => {
                format!(
                    "{}: приложения [{}] и сайты [{}] через VPN",
                    prefix, processes, sites
                )
            }
            _ => prefix.to_string(),
        }
    } else {
        match (process_count, site_count) {
            (0, 0) => format!("{}: вся система через VPN", prefix),
            (processes, 0) if processes > 0 => {
                format!("{}: исключения процессов [{}]", prefix, processes)
            }
            (0, sites) if sites > 0 => format!("{}: исключения сайтов [{}]", prefix, sites),
            (processes, sites) => format!(
                "{}: исключения процессов [{}] и сайтов [{}]",
                prefix, processes, sites
            ),
        }
    }
}

pub(super) fn start_proxybridge(
    processes: &[String],
    selected_sites: &[String],
    selected_apps_only: bool,
    wireproxy_info_addr: Option<&str>,
    tunnel_dns_servers: &[String],
) -> Result<Option<std::process::Child>, String> {
    use std::fs::OpenOptions;
    #[cfg(target_os = "windows")]
    use std::os::windows::process::CommandExt;

    if selected_apps_only && processes.is_empty() && selected_sites.is_empty() {
        return Err("Не выбраны процессы для маршрутизации или сайты для VPN".to_string());
    }

    let current_exe =
        std::env::current_exe().map_err(|_| "Не удалось определить текущий путь".to_string())?;
    let current_exe_name = current_exe
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string);

    // ProxyBridge inserts each added rule at the head, so later entries have higher priority.
    let mut rules: Vec<String> = Vec::new();
    let append_internal_direct_rules = |rules: &mut Vec<String>| {
        rules.push("ProxyBridge_CLI.exe:*:*:BOTH:DIRECT".to_string());
        rules.push("wireproxy.exe:*:*:BOTH:DIRECT".to_string());

        if let (Some(process_name), Some(info_addr)) =
            (current_exe_name.as_deref(), wireproxy_info_addr)
        {
            if let Ok(info_socket) = info_addr.parse::<SocketAddr>() {
                rules.push(format!(
                    "{}:{}:{}:TCP:DIRECT",
                    process_name,
                    info_socket.ip(),
                    info_socket.port()
                ));
            }
        }
    };

    if selected_apps_only {
        let (site_targets, unresolved_sites) = resolve_site_rule_targets(selected_sites);
        let site_rules = format_site_rules(&site_targets, "*", "BOTH", "PROXY");
        let site_udp_443_block_rules = format_site_rules(&site_targets, "443", "UDP", "BLOCK");

        if !processes.is_empty() {
            rules.extend(
                processes
                    .iter()
                    .map(|process| format!("{}:*:*:BOTH:PROXY", process)),
            );
            rules.extend(
                processes
                    .iter()
                    .map(|process| format!("{}:*:443:UDP:BLOCK", process)),
            );
        }

        if !unresolved_sites.is_empty() {
            log::warn!(
                "Не удалось разрешить IPv4 для сайтов через VPN: {}",
                unresolved_sites.join(", ")
            );
        }

        rules.extend(site_rules);
        rules.extend(site_udp_443_block_rules);

        if rules.is_empty() {
            if !unresolved_sites.is_empty() {
                return Err(format!(
                    "Не удалось разрешить IPv4 для сайтов через VPN: {}",
                    unresolved_sites.join(", ")
                ));
            }
            return Err("Не выбраны процессы для маршрутизации или сайты для VPN".to_string());
        }
        rules.extend(
            tunnel_dns_servers
                .iter()
                .map(|server| format!("*:{}:53:BOTH:PROXY", server)),
        );
        append_internal_direct_rules(&mut rules);
    } else {
        let (site_rules, unresolved_sites) = build_site_rules(selected_sites, "DIRECT");

        rules.push("*:*:*:BOTH:PROXY".to_string());
        rules.push("*:*:443:UDP:BLOCK".to_string());

        if !processes.is_empty() {
            rules.extend(
                processes
                    .iter()
                    .map(|process| format!("{}:*:*:BOTH:DIRECT", process)),
            );
        }

        if !unresolved_sites.is_empty() {
            log::warn!(
                "Не удалось разрешить IPv4 для сайтов-исключений из VPN: {}",
                unresolved_sites.join(", ")
            );
        }

        rules.extend(site_rules);
        append_internal_direct_rules(&mut rules);
    }

    let deps = embedded_deps_bytes::ExtractedDeps::get()
        .map_err(|e| format!("Не удалось получить зависимости: {}", e))?;

    let cli_exe = &deps.proxybridge_cli;

    let exe_dir = current_exe
        .parent()
        .ok_or("Не удалось получить директорию приложения".to_string())?;

    let cache_dir = super::managed_cache_dir();
    let log_path = super::managed_logs_dir().join("proxybridge.log");
    let pid_file = cache_dir.join("proxybridge.pid");
    let localhost_via_proxy = "False";

    if super::is_elevated() {
        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|e| format!("Не удалось открыть лог файл: {}", e))?;
        let log_file_err = log_file
            .try_clone()
            .map_err(|e| format!("Не удалось клонировать лог файл: {}", e))?;
        let log_offset = log_file
            .metadata()
            .map(|metadata| metadata.len())
            .unwrap_or(0);

        let mut cmd = std::process::Command::new(&cli_exe);
        cmd.arg("--proxy")
            .arg("socks5://127.0.0.1:1080")
            .arg("--dns-via-proxy")
            .arg("False")
            .arg("--localhost-via-proxy")
            .arg(localhost_via_proxy)
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

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Не удалось запустить ProxyBridge: {}", e))?;

        if let Err(error) = wait_for_proxybridge_start(
            &log_path,
            log_offset,
            PROXYBRIDGE_START_WAIT_TIMEOUT,
            Some(&mut child),
        ) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }

        let _ = std::fs::write(pid_file, "running");
        return Ok(Some(child));
    }

    let batch_path = cache_dir.join("run_proxybridge_elevated.bat");
    let mut batch = String::new();
    batch.push_str("@echo off\r\n");
    batch.push_str(&format!(
        "cd /d \"{}\"\r\n",
        cli_exe.parent().unwrap_or(&cache_dir).display()
    ));
    let mut cmdline = format!(
        "\"{}\" --proxy socks5://127.0.0.1:1080 --dns-via-proxy False --localhost-via-proxy {} --verbose 3",
        cli_exe.display(),
        localhost_via_proxy
    );
    for r in &rules {
        let safe = r.replace('"', "\\\"");
        cmdline.push_str(&format!(" --rule \"{}\"", safe));
    }
    cmdline.push_str(&format!(" >> \"{}\" 2>&1\r\n", log_path.display()));
    batch.push_str(&cmdline);

    std::fs::write(&batch_path, batch)
        .map_err(|e| format!("Не удалось создать батч-файл для запуска: {}", e))?;

    let log_offset = std::fs::metadata(&log_path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    super::app_runtime::launch_self_elevated(&[
        OsString::from("/start-proxybridge"),
        batch_path.as_os_str().to_os_string(),
    ])
    .map_err(|e| {
        format!(
            "Не удалось запустить ProxyBridge с правами администратора: {}",
            e
        )
    })?;

    wait_for_proxybridge_start(
        &log_path,
        log_offset,
        PROXYBRIDGE_ELEVATED_START_WAIT_TIMEOUT,
        None,
    )?;

    let _ = std::fs::write(pid_file, "running");
    Ok(None)
}

pub(super) fn stop_proxybridge() -> Result<(), String> {
    let cache_dir = super::managed_cache_dir();

    let pid_file = cache_dir.join("proxybridge.pid");
    let matches_proxybridge_process =
        |process: &sysinfo::Process| process_name_matches(process, "ProxyBridge_CLI.exe");

    if !pid_file.exists() && !any_process_matches(matches_proxybridge_process) {
        return Err("ProxyBridge не запущен (файл маркера не найден)".to_string());
    }

    if !super::is_elevated() {
        let launch_result =
            super::app_runtime::launch_self_elevated(&[OsString::from("/stop-proxybridge")]);

        if let Err(error) = launch_result {
            return Err(error);
        }

        if wait_until_processes_exit(matches_proxybridge_process, ELEVATED_HELPER_WAIT_TIMEOUT) {
            let _ = std::fs::remove_file(&pid_file);
            return Ok(());
        }

        return Err("Не удалось остановить все процессы ProxyBridge_CLI.exe".to_string());
    }

    let _ = std::fs::remove_file(&pid_file);

    let _ = kill_processes_matching(matches_proxybridge_process);

    if !wait_until_processes_exit(matches_proxybridge_process, PROCESS_EXIT_WAIT_TIMEOUT) {
        fallback_taskkill_image("ProxyBridge_CLI.exe");

        if !wait_until_processes_exit(matches_proxybridge_process, PROCESS_EXIT_WAIT_TIMEOUT) {
            return Err("Не удалось остановить все процессы ProxyBridge_CLI.exe".to_string());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_log_path(test_name: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "vpnfybot-proxybridge-{}-{}-{}.log",
            test_name,
            std::process::id(),
            unique
        ))
    }

    #[test]
    fn reads_only_current_proxybridge_start_attempt() {
        let path = temp_log_path("current-attempt");
        fs::write(&path, "[LOG] ProxyBridge started\nold traffic\n").unwrap();
        let mut offset = fs::metadata(&path).unwrap().len();

        let mut log = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        writeln!(log, "new attempt without a ready marker").unwrap();
        log.flush().unwrap();

        let current_attempt = read_log_since(&path, &mut offset).unwrap();
        assert!(!proxybridge_start_succeeded(&current_attempt));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn finds_start_marker_before_large_traffic_burst() {
        let mut output = "[LOG] ProxyBridge started\n".to_string();
        output.push_str(&"[CONN] busy process traffic\n".repeat(500));

        assert!(output.len() > 4096);
        assert!(!output[output.len() - 4096..].contains("ProxyBridge started"));
        assert!(proxybridge_start_succeeded(&output));
    }
}
