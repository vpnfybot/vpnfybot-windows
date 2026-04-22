use super::*;

impl Default for AppState {
    fn default() -> Self {
        let conf_path = load_saved_conf_path();
        let status = String::new();
        let selected_processes = load_selected_processes();
        let selected_sites = load_selected_sites();
        let proxy_mode_toggle = load_proxy_mode();
        let language = load_language();

        let s = Self {
            conf_path,
            status,
            error_log: None,
            status_rx: None,
            service_running: false,
            service_active: false,
            elevated: is_elevated(),
            session_traffic_bytes: 0,
            session_base_traffic_bytes: None,
            connected_at: None,
            startup_animation_frame: 0,
            wireproxy_info_addr: None,
            last_tunnel_traffic_poll: None,
            traffic_worker_receiver: None,
            traffic_worker_stop: None,
            last_tunnel_totals: None,
            last_time_display_update: None,
            cached_time_display: String::new(),
            cached_up_display: "0.00".to_string(),
            cached_down_display: "0.00".to_string(),
            last_upload_bps: 0.0,
            last_download_bps: 0.0,
            upload_icon: None,
            download_icon: None,
            top_image: None,
            settings_icon: None,
            settings_close_icon: None,
            language_icon: None,
            animated_frames: None,
            animated_frame_durations: Vec::new(),
            animated_frame_index: 0,
            animated_last_frame: Instant::now(),
            gif_pulse_start: None,
            gif_rotation_start: Instant::now(),
            window_frame_styled: false,
            window_frame_attempts: 0,
            tray_subclassed: false,
            tray_icon_added: false,
            tray_window: None,
            tray_icon: None,
            traffic_opacity: 0.0,
            import_button_opacity: 1.0,
            import_button_opacity_target: 1.0,
            connect_animation_start: None,
            disconnect_animation_start: None,
            last_notification: None,
            update_pending: None,
            proxybridge_running: false,
            selected_processes,
            selected_sites,
            process_window_receiver: None,
            site_window_receiver: None,
            show_settings: false,
            settings_tab: "processes".to_string(),
            cached_processes: Vec::new(),
            last_process_refresh: None,
            process_search_text: String::new(),
            proxy_mode_toggle,
            proxybridge_child: None,
            language,
            win_text_cache: std::collections::HashMap::new(),
            button_hfont: create_button_ui_font(),
            button_hfont_light: create_button_ui_font_light(),
        };
        update_check::spawn_update_check_thread();
        s
    }
}

impl AppState {
    #[allow(dead_code)]
    pub(super) fn get_tunnel_total_bytes(&self) -> Option<u64> {
        let info_addr = self.wireproxy_info_addr.as_deref()?;
        let metrics = fetch_wireproxy_metrics(info_addr)?;
        parse_wireproxy_metrics_rx_tx(&metrics).map(|(tx, rx)| tx.saturating_add(rx))
    }

    #[allow(dead_code)]
    pub(super) fn get_tunnel_rx_tx_totals(&self) -> Option<(u64, u64)> {
        let info_addr = self.wireproxy_info_addr.as_deref()?;
        let metrics = fetch_wireproxy_metrics(info_addr)?;
        parse_wireproxy_metrics_rx_tx(&metrics)
    }

    pub(super) fn start_tunnel_traffic_worker(&mut self) {
        self.stop_tunnel_traffic_worker();

        let Some(info_addr) = self.wireproxy_info_addr.clone() else {
            return;
        };

        let (tx, rx) = mpsc::channel();
        let stop_flag = Arc::new(AtomicBool::new(false));
        let worker_stop = stop_flag.clone();

        thread::spawn(move || {
            while !worker_stop.load(Ordering::Relaxed) {
                if let Some((tx_bytes, rx_bytes)) = fetch_wireproxy_metrics(&info_addr)
                    .and_then(|metrics| parse_wireproxy_metrics_rx_tx(&metrics))
                {
                    let sample = TunnelTrafficSample {
                        total_bytes: tx_bytes.saturating_add(rx_bytes),
                        tx_bytes,
                        rx_bytes,
                        captured_at: Instant::now(),
                    };

                    if tx.send(sample).is_err() {
                        break;
                    }
                }

                let wake_at = Instant::now() + TUNNEL_TRAFFIC_POLL_INTERVAL;
                while !worker_stop.load(Ordering::Relaxed) {
                    let now = Instant::now();
                    if now >= wake_at {
                        break;
                    }

                    thread::sleep((wake_at - now).min(Duration::from_millis(100)));
                }
            }
        });

        self.traffic_worker_receiver = Some(rx);
        self.traffic_worker_stop = Some(stop_flag);
    }

    pub(super) fn stop_tunnel_traffic_worker(&mut self) {
        if let Some(stop_flag) = self.traffic_worker_stop.take() {
            stop_flag.store(true, Ordering::Relaxed);
        }
        self.traffic_worker_receiver = None;
    }

    pub(super) fn apply_pending_tunnel_traffic_samples(&mut self) -> bool {
        let mut latest_sample = None;

        if let Some(rx) = &self.traffic_worker_receiver {
            while let Ok(sample) = rx.try_recv() {
                latest_sample = Some(sample);
            }
        }

        let Some(sample) = latest_sample else {
            return false;
        };

        let prev_instant = self.last_tunnel_traffic_poll;
        let prev_totals = self.last_tunnel_totals;
        let base = self
            .session_base_traffic_bytes
            .get_or_insert(sample.total_bytes);
        self.session_traffic_bytes = sample.total_bytes.saturating_sub(*base);

        if let Some((prev_tx, prev_rx)) = prev_totals {
            let elapsed = prev_instant
                .map(|p| sample.captured_at.duration_since(p))
                .unwrap_or(TUNNEL_TRAFFIC_POLL_INTERVAL);
            let secs = elapsed.as_secs_f64().max(0.000_001);
            self.last_upload_bps = sample.tx_bytes.saturating_sub(prev_tx) as f64 / secs;
            self.last_download_bps = sample.rx_bytes.saturating_sub(prev_rx) as f64 / secs;
        } else {
            self.last_upload_bps = 0.0;
            self.last_download_bps = 0.0;
        }

        self.last_tunnel_totals = Some((sample.tx_bytes, sample.rx_bytes));
        self.last_tunnel_traffic_poll = Some(sample.captured_at);
        true
    }

    pub(super) fn reset_tunnel_traffic_state(&mut self) {
        self.stop_tunnel_traffic_worker();
        self.session_traffic_bytes = 0;
        self.session_base_traffic_bytes = None;
        self.wireproxy_info_addr = None;
        self.last_tunnel_traffic_poll = None;
        self.last_tunnel_totals = None;
        self.last_upload_bps = 0.0;
        self.last_download_bps = 0.0;
        self.last_time_display_update = None;
        self.cached_time_display.clear();
        self.cached_up_display.clear();
        self.cached_up_display.push_str("0.00");
        self.cached_down_display.clear();
        self.cached_down_display.push_str("0.00");
    }

    pub(super) fn format_connection_time(&self) -> String {
        if let Some(started) = self.connected_at {
            let elapsed = started.elapsed().as_secs();
            let hours = elapsed / 3600;
            let minutes = (elapsed % 3600) / 60;
            let seconds = elapsed % 60;
            format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
        } else {
            "00:00:00".to_string()
        }
    }

    pub(super) fn gif_pulse_scale(&mut self) -> f32 {
        if let Some(start) = self.gif_pulse_start {
            let elapsed = start.elapsed().as_millis() as f32;
            let duration = 260.0;
            if elapsed >= duration {
                self.gif_pulse_start = None;
                1.0
            } else {
                let t = (elapsed / duration).clamp(0.0, 1.0);
                1.0 + 0.06 * (1.0 - (1.0 - t).powi(2))
            }
        } else {
            1.0
        }
    }

    pub(super) fn connect_effect_progress(&mut self) -> f32 {
        if let Some(start) = self.disconnect_animation_start {
            let elapsed = start.elapsed().as_millis() as f32;
            let duration = 400.0;
            if elapsed >= duration {
                self.disconnect_animation_start = None;
                0.0
            } else {
                let t = (elapsed / duration).clamp(0.0, 1.0);
                (1.0 - t).powi(3)
            }
        } else if let Some(start) = self.connect_animation_start {
            let elapsed = start.elapsed().as_millis() as f32;
            let duration = 400.0;
            if elapsed >= duration {
                self.connect_animation_start = None;
                1.0
            } else {
                let t = (elapsed / duration).clamp(0.0, 1.0);
                1.0 - (1.0 - t).powi(3)
            }
        } else if self.service_running || self.service_active {
            1.0
        } else {
            0.0
        }
    }

    pub(super) fn gif_rotation_angle(&self) -> f32 {
        let elapsed = self.gif_rotation_start.elapsed().as_secs_f32();
        let period = 90.0;
        let t = (elapsed % period) / period;
        t * std::f32::consts::TAU
    }

    pub(super) fn reset_app_settings(&mut self) {
        self.conf_path = None;
        self.selected_processes.clear();
        self.selected_sites.clear();
        self.proxy_mode_toggle = false;
        self.status.clear();
        self.error_log = None;
        self.status_rx = None;
        self.service_running = false;
        self.service_active = false;
        self.proxybridge_running = false;
        self.reset_tunnel_traffic_state();
        self.connected_at = None;
        self.startup_animation_frame = 0;
        self.traffic_opacity = 0.0;
        self.import_button_opacity = 1.0;
        self.import_button_opacity_target = 1.0;
        self.connect_animation_start = None;
        self.disconnect_animation_start = None;
        self.gif_pulse_start = None;
        self.show_settings = false;
        self.settings_tab = "processes".to_string();
        self.cached_processes.clear();
        self.last_process_refresh = None;
        self.process_search_text.clear();
        self.language = Language::En;
        self.win_text_cache.clear();
        delete_app_storage_dirs();
        save_language(self.language);
    }
}

fn relaunch_as_admin() -> bool {
    let exe = match env::current_exe() {
        Ok(path) => path,
        Err(_) => return false,
    };
    let exe_w: Vec<u16> = exe.as_os_str().encode_wide().chain(Some(0)).collect();
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("runas"),
            PCWSTR(exe_w.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    (result.0 as isize) > 32
}

fn check_single_instance() -> bool {
    let title_wide: Vec<u16> = OsStr::new(WINDOW_TITLE).encode_wide().chain(Some(0)).collect();
    unsafe {
        let existing_window = FindWindowW(None, PCWSTR(title_wide.as_ptr()));

        if existing_window.0 != 0 {
            SetForegroundWindow(existing_window);
            ShowWindow(existing_window, SW_RESTORE);
            return false;
        }
    }
    true
}

fn setup_firewall_rules() {
    thread::spawn(|| {
        if let Ok(deps) = embedded_deps_bytes::ExtractedDeps::get() {
            let wireproxy_path = deps.wireproxy.to_string_lossy().to_string();
            let proxybridge_path = deps.proxybridge_cli.to_string_lossy().to_string();

            let script = format!(r#"
# Функция для добавления или обновления правила брандмауэра
function Set-FirewallRule {{
    param(
        [string]$RuleName,
        [string]$ProgramPath
    )

    if (-not (Test-Path "$ProgramPath")) {{
        Write-Host "Файл не найден: $ProgramPath" -ForegroundColor Red
        return $false
    }}

    try {{
        netsh advfirewall firewall delete rule name="$RuleName" 2>$null | Out-Null

        netsh advfirewall firewall add rule `
            name="$RuleName" `
            dir=in `
            action=allow `
            program="$ProgramPath" `
            enable=yes `
            profile=any `
            remoteip=any `
            description="Разрешение для VPNFy - автоматически добавлено"

        if ($LASTEXITCODE -eq 0) {{
            Write-Host "✓ Добавлено правило: $RuleName" -ForegroundColor Green
        }} else {{
            Write-Host "⚠ Ошибка при добавлении правила: $RuleName" -ForegroundColor Yellow
        }}

        return $true
    }} catch {{
        Write-Host "✗ Исключение при установке правила $($RuleName): $_" -ForegroundColor Red
        return $false
    }}
}}

Set-FirewallRule -RuleName "VPNFy - wireproxy (incoming)" -ProgramPath "{wireproxy_path}"
Set-FirewallRule -RuleName "VPNFy - ProxyBridge (incoming)" -ProgramPath "{proxybridge_path}"

Write-Host "Готово: правила брандмауэра установлены" -ForegroundColor Cyan
"#);

            let mut cmd = std::process::Command::new("powershell");
            cmd.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &script])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());

            #[cfg(target_os = "windows")]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x08000000;
                cmd.creation_flags(CREATE_NO_WINDOW);
            }

            match cmd.spawn() {
                Ok(mut child) => match child.wait() {
                    Ok(status) => {
                        if status.success() {
                            eprintln!("✓ Правила брандмауэра успешно установлены");
                        } else {
                            eprintln!(
                                "⚠ Ошибка при установке правил брандмауэра (код {})",
                                status.code().unwrap_or(-1)
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("⚠ Ошибка ожидания процесса: {}", e);
                    }
                },
                Err(e) => {
                    eprintln!("⚠ Не удалось запустить PowerShell для установки правил: {}", e);
                }
            }
        } else {
            eprintln!("⚠ Не удалось получить пути к зависимостям для установки правил");
        }
    });
}

#[cfg(target_os = "windows")]
fn configure_process_notification_identity() {
    let app_id = to_wide(NOTIFICATION_APP_ID);
    unsafe {
        if let Err(error) = SetCurrentProcessExplicitAppUserModelID(PCWSTR(app_id.as_ptr())) {
            eprintln!(
                "⚠ Не удалось назначить AppUserModelID для уведомлений: {}",
                error
            );
        }
    }
}

pub(crate) fn app_main() -> eframe::Result<()> {
    if !check_single_instance() {
        std::process::exit(0);
    }

    let args: Vec<OsString> = env::args_os().collect();
    if args.len() >= 3 && args[1] == OsStr::new("/service") {
        run_wireproxy_mode(&args[2]);
    }

    if !is_elevated() {
        if relaunch_as_admin() {
            std::process::exit(0);
        }
    }

    match app_dirs::AppDirs::init() {
        Ok(app_dirs) => {
            if let Err(e) = app_dirs.reset_runtime_state() {
                eprintln!("⚠ Ошибка очистки runtime-временных файлов: {}", e);
            }

            eprintln!(
                "✓ Инициализирована структура приложения в: {}",
                app_dirs.root.display()
            );
            eprintln!("  ├─ Логи: {}", app_dirs.logs.display());
            eprintln!("  ├─ Разрешения: {}", app_dirs.permissions.display());
            eprintln!("  ├─ Конфиги: {}", app_dirs.configs.display());
            eprintln!("  └─ Кэш: {}", app_dirs.cache.display());
        }
        Err(e) => {
            eprintln!("⚠ Ошибка инициализации директорий: {}", e);
        }
    }

    #[cfg(target_os = "windows")]
    configure_process_notification_identity();

    setup_firewall_rules();

    let pid_file = managed_cache_dir().join("proxybridge.pid");
    if pid_file.exists() {
        let _ = stop_proxybridge();
        let _ = std::fs::remove_file(&pid_file);
    }

    let mut options = eframe::NativeOptions::default();
    options.viewport = egui::ViewportBuilder::default()
        .with_title(WINDOW_TITLE)
        .with_inner_size([
            MAIN_WINDOW_CLIENT_WIDTH as f32,
            MAIN_WINDOW_CLIENT_HEIGHT as f32,
        ])
        .with_min_inner_size([
            MAIN_WINDOW_CLIENT_WIDTH as f32,
            MAIN_WINDOW_CLIENT_HEIGHT as f32,
        ])
        .with_max_inner_size([MAIN_WINDOW_CLIENT_WIDTH as f32, 1000.0])
        .with_resizable(false)
        .with_maximize_button(false)
        .with_decorations(true)
        .with_icon(
            from_png_bytes(include_bytes!("../../gifs/vpnfy.png"))
                .expect("Failed to load icon"),
        );

    eframe::run_native(
        WINDOW_TITLE,
        options,
        Box::new(|cc| {
            configure_egui_button_font(&cc.egui_ctx);
            Box::new(AppState::default())
        }),
    )
}

fn run_wireproxy_mode(conf: &OsStr) -> ! {
    let conf_path = conf.to_string_lossy();

    let deps = match embedded_deps_bytes::ExtractedDeps::get() {
        Ok(paths) => paths,
        Err(e) => {
            eprintln!("Не удалось получить зависимости: {}", e);
            std::process::exit(1);
        }
    };

    match std::process::Command::new(&deps.wireproxy)
        .arg("-c")
        .arg(conf_path.as_ref())
        .spawn()
    {
        Ok(mut child) => {
            let exit_status = child
                .wait()
                .unwrap_or_else(|_| std::process::ExitStatus::default());
            if let Some(code) = exit_status.code() {
                std::process::exit(code);
            } else {
                std::process::exit(0);
            }
        }
        Err(e) => {
            eprintln!("Ошибка запуска wireproxy.exe: {}", e);
            std::process::exit(1);
        }
    }
}

#[link(name = "shell32")]
extern "system" {
    fn IsUserAnAdmin() -> i32;
}

pub(super) fn is_elevated() -> bool {
    unsafe { IsUserAnAdmin() != 0 }
}