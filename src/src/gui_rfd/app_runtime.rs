use super::*;
#[cfg(target_os = "windows")]
use super::app_windows::{
    publish_taskbar_traffic_widget_snapshot, set_taskbar_traffic_widget_worker_enabled,
};

const SUBSCRIPTION_SCHEDULED_TASK_NAME: &str = "vpnfybot-windows-subscription-check";
const SUBSCRIPTION_SCHEDULED_TASK_ARG: &str = "/subscription-check";

impl Default for AppState {
    fn default() -> Self {
        let conf_path = load_saved_conf_path();
        let imported_conf_is_amnezia_wireguard = conf_path
            .as_deref()
            .is_some_and(is_amnezia_wireguard_config_path);
        let status = String::new();
        let selected_processes = load_selected_processes();
        let selected_sites = load_selected_sites();
        let proxy_mode_toggle = load_proxy_mode();
        let taskbar_widget_enabled = load_taskbar_widget_enabled();
        let language = load_language();

        let mut s = Self {
            conf_path,
            imported_conf_is_amnezia_wireguard,
            status,
            error_log: None,
            status_rx: None,
            subscription_info_rx: None,
            service_running: false,
            service_active: false,
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
            subscription_for_date_display: None,
            subscription_expires_at_unix: None,
            cached_up_display: "0.00".to_string(),
            cached_down_display: "0.00".to_string(),
            last_upload_bps: 0.0,
            last_download_bps: 0.0,
            traffic_history: VecDeque::new(),
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
            #[cfg(target_os = "windows")]
            taskbar_widget_window: None,
            #[cfg(target_os = "windows")]
            taskbar_widget_monitor: None,
            taskbar_widget_enabled,
            traffic_opacity: 0.0,
            import_button_opacity: 1.0,
            connect_animation_start: None,
            disconnect_animation_start: None,
            last_notification: None,
            connection_notification_pending: false,
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
        cleanup_legacy_subscription_notifications();
        s.spawn_subscription_info_refresh();
        s
    }
}

impl AppState {
    pub(super) fn set_imported_conf_path(&mut self, path: String) {
        self.imported_conf_is_amnezia_wireguard = is_amnezia_wireguard_config_path(&path);
        self.conf_path = Some(path);
        self.error_log = None;
        save_conf_path(self.conf_path.as_ref().unwrap());
        self.status.clear();
        self.reset_subscription_info_display();
        self.spawn_subscription_info_refresh();
    }

    pub(super) fn reset_subscription_info_display(&mut self) {
        self.subscription_info_rx = None;
        self.subscription_for_date_display = None;
        self.subscription_expires_at_unix = None;
        self.last_time_display_update = None;
        self.cached_time_display.clear();
    }

    pub(super) fn spawn_subscription_info_refresh(&mut self) {
        self.subscription_info_rx = None;

        let Some(conf_path) = self.conf_path.clone() else {
            return;
        };

        let (tx, rx) = mpsc::channel();
        self.subscription_info_rx = Some(rx);

        thread::spawn(move || {
            let info = fetch_subscription_info(&conf_path);
            let _ = tx.send(info);
        });
    }

    pub(super) fn apply_pending_subscription_info(&mut self) -> bool {
        let recv_result = match self.subscription_info_rx.as_ref() {
            Some(rx) => rx.try_recv(),
            None => return false,
        };

        match recv_result {
            Ok(info) => {
                if let Some(info) = info {
                    self.subscription_for_date_display = Some(info.display_date);
                    self.subscription_expires_at_unix = Some(info.expires_at_unix);
                } else {
                    self.subscription_for_date_display = None;
                    self.subscription_expires_at_unix = None;
                }
                self.subscription_info_rx = None;
                self.last_time_display_update = None;
                self.cached_time_display.clear();
                true
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => false,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.subscription_info_rx = None;
                false
            }
        }
    }

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

        #[cfg(target_os = "windows")]
        set_taskbar_traffic_widget_worker_enabled(self.taskbar_widget_enabled);

        let (tx, rx) = mpsc::channel();
        let stop_flag = Arc::new(AtomicBool::new(false));
        let worker_stop = stop_flag.clone();

        thread::spawn(move || {
            let mut worker_last_totals = None;
            let mut worker_last_poll = None;
            let mut worker_history: VecDeque<TrafficHistoryPoint> = VecDeque::new();

            while !worker_stop.load(Ordering::Relaxed) {
                if let Some((tx_bytes, rx_bytes)) = fetch_wireproxy_metrics(&info_addr)
                    .and_then(|metrics| parse_wireproxy_metrics_rx_tx(&metrics))
                {
                    let captured_at = Instant::now();
                    let total_bytes = tx_bytes.saturating_add(rx_bytes);
                    let (upload_bps, download_bps) =
                        if let Some((prev_tx, prev_rx)) = worker_last_totals {
                            let elapsed = worker_last_poll
                                .map(|previous| captured_at.duration_since(previous))
                                .unwrap_or(TUNNEL_TRAFFIC_POLL_INTERVAL);
                            let secs = elapsed.as_secs_f64().max(0.000_001);
                            (
                                tx_bytes.saturating_sub(prev_tx) as f64 / secs,
                                rx_bytes.saturating_sub(prev_rx) as f64 / secs,
                            )
                        } else {
                            (0.0, 0.0)
                        };

                    worker_history.push_back(TrafficHistoryPoint {
                        upload_bps,
                        download_bps,
                        captured_at,
                    });
                    while worker_history.len() > TASKBAR_TRAFFIC_HISTORY_CAPACITY {
                        let _ = worker_history.pop_front();
                    }
                    while worker_history.front().is_some_and(|point| {
                        captured_at.duration_since(point.captured_at)
                            > TASKBAR_TRAFFIC_HISTORY_WINDOW
                    }) {
                        let _ = worker_history.pop_front();
                    }

                    #[cfg(target_os = "windows")]
                    publish_taskbar_traffic_widget_snapshot(
                        true,
                        upload_bps,
                        download_bps,
                        worker_history
                            .iter()
                            .map(|point| (point.upload_bps, point.download_bps))
                            .collect(),
                    );

                    worker_last_totals = Some((tx_bytes, rx_bytes));
                    worker_last_poll = Some(captured_at);

                    let sample = TunnelTrafficSample {
                        total_bytes,
                        tx_bytes,
                        rx_bytes,
                        captured_at,
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
        #[cfg(target_os = "windows")]
        publish_taskbar_traffic_widget_snapshot(false, 0.0, 0.0, Vec::new());
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

        self.traffic_history.push_back(TrafficHistoryPoint {
            upload_bps: self.last_upload_bps,
            download_bps: self.last_download_bps,
            captured_at: sample.captured_at,
        });
        while self.traffic_history.len() > TASKBAR_TRAFFIC_HISTORY_CAPACITY {
            let _ = self.traffic_history.pop_front();
        }
        while self.traffic_history.front().is_some_and(|point| {
            sample.captured_at.duration_since(point.captured_at) > TASKBAR_TRAFFIC_HISTORY_WINDOW
        }) {
            let _ = self.traffic_history.pop_front();
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
        self.traffic_history.clear();
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

    pub(super) fn format_subscription_active_until_text(&self) -> String {
        let display = self
            .subscription_for_date_display
            .as_deref()
            .unwrap_or("--.--.----");
        self.language
            .translate("Активна до: {}")
            .replacen("{}", display, 1)
    }

    pub(super) fn format_center_status_text(&self) -> String {
        if self.connected_at.is_some() {
            let mb = self.session_traffic_bytes as f64 / 1024.0 / 1024.0;
            let traffic_text = if mb > 1000.0 {
                format!("{:.2} GB", mb / 1024.0)
            } else {
                format!("{:.2} MB", mb)
            };
            format!("{} / {}", self.format_connection_time(), traffic_text)
        } else if self.subscription_for_date_display.is_some() {
            self.format_subscription_active_until_text()
        } else {
            "00:00:00".to_string()
        }
    }

    pub(super) fn subscription_is_expired(&self) -> bool {
        self.subscription_expires_at_unix
            .zip(current_unix_timestamp())
            .is_some_and(|(expires_at_unix, now_unix)| expires_at_unix <= now_unix)
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
        if let Err(error) = restore_tunnel_dns() {
            log::warn!("Failed to restore DNS while resetting settings: {}", error);
        }
        self.conf_path = None;
        self.imported_conf_is_amnezia_wireguard = false;
        self.selected_processes.clear();
        self.selected_sites.clear();
        self.proxy_mode_toggle = false;
        self.status.clear();
        self.error_log = None;
        self.status_rx = None;
        self.service_running = false;
        self.service_active = false;
        self.connection_notification_pending = false;
        self.proxybridge_running = false;
        self.reset_tunnel_traffic_state();
        self.connected_at = None;
        self.taskbar_widget_enabled = true;
        self.reset_subscription_info_display();
        self.startup_animation_frame = 0;
        self.traffic_opacity = 0.0;
        self.import_button_opacity = 1.0;
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
        cleanup_legacy_subscription_notifications();
        delete_app_storage_dirs();
        save_language(self.language);
    }
}

const SUBINFO_URL: &str = "https://vpnfybot.duckdns.org/subinfo";

pub(super) fn is_amnezia_wireguard_config_path(conf_path: &str) -> bool {
    fs::read_to_string(conf_path)
        .map(|config| is_amnezia_wireguard_config_content(&config))
        .unwrap_or(false)
}

fn is_amnezia_wireguard_config_content(config: &str) -> bool {
    let mut current_section = "";

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

        let Some((key, _value)) = line.split_once('=') else {
            continue;
        };

        if is_amnezia_wireguard_key(key.trim()) {
            return true;
        }
    }

    false
}

fn is_amnezia_wireguard_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "jc" | "jmin"
            | "jmax"
            | "s1"
            | "s2"
            | "s3"
            | "s4"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "i1"
            | "i2"
            | "i3"
            | "i4"
            | "i5"
    )
}

fn fetch_subscription_info(conf_path: &str) -> Option<SubscriptionInfo> {
    let payload = parse_subscription_info_payload(conf_path)?;
    let body = serde_json::json!({
        "host": payload.host,
        "private_key": payload.private_key,
    })
    .to_string();

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout_read(Duration::from_secs(5))
        .timeout_write(Duration::from_secs(5))
        .build();

    let response = agent
        .post(SUBINFO_URL)
        .set("Accept", "application/json")
        .set("Content-Type", "application/json")
        .send_string(&body)
        .ok()?;

    let body = response.into_string().ok()?;
    let json: serde_json::Value = serde_json::from_str(&body).ok()?;
    extract_subscription_info(&json)
}

fn parse_subscription_info_payload(conf_path: &str) -> Option<SubscriptionInfoPayload> {
    let config = fs::read_to_string(conf_path).ok()?;
    let mut current_section = "";
    let mut private_key = None;
    let mut endpoint_host = None;

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

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };

        let key = key.trim();
        let value = value.trim();
        if value.is_empty() {
            continue;
        }

        if current_section.eq_ignore_ascii_case("Interface")
            && key.eq_ignore_ascii_case("PrivateKey")
        {
            private_key = Some(value.to_string());
            continue;
        }

        if endpoint_host.is_none()
            && current_section.eq_ignore_ascii_case("Peer")
            && key.eq_ignore_ascii_case("Endpoint")
        {
            endpoint_host = parse_endpoint_host(value);
        }
    }

    let endpoint_host = endpoint_host?;
    let host = resolve_subscription_host(&endpoint_host).unwrap_or(endpoint_host);
    let private_key = private_key?;

    Some(SubscriptionInfoPayload { host, private_key })
}

fn parse_endpoint_host(endpoint: &str) -> Option<String> {
    let endpoint = endpoint.trim();
    if endpoint.is_empty() {
        return None;
    }

    if let Some(rest) = endpoint.strip_prefix('[') {
        let closing = rest.find(']')?;
        let host = rest[..closing].trim();
        return (!host.is_empty()).then(|| host.to_string());
    }

    if let Some((host, _port)) = endpoint.rsplit_once(':') {
        let host = host.trim();
        if !host.is_empty() {
            return Some(host.to_string());
        }
    }

    Some(endpoint.to_string())
}

fn resolve_subscription_host(host: &str) -> Option<String> {
    let host = host.trim().trim_matches(['[', ']']);
    if host.is_empty() {
        return None;
    }

    let mut fallback = None;
    if let Ok(addresses) = (host, 0).to_socket_addrs() {
        for address in addresses {
            match address {
                SocketAddr::V4(ipv4) => return Some(ipv4.ip().to_string()),
                SocketAddr::V6(ipv6) => {
                    if fallback.is_none() {
                        fallback = Some(ipv6.ip().to_string());
                    }
                }
            }
        }
    }

    fallback.or_else(|| Some(host.to_string()))
}

fn extract_subscription_info(value: &serde_json::Value) -> Option<SubscriptionInfo> {
    match value {
        serde_json::Value::Null => None,
        serde_json::Value::Bool(_) => None,
        serde_json::Value::Number(number) => {
            let timestamp = number.as_i64()?;
            subscription_info_from_unix_timestamp(timestamp)
        }
        serde_json::Value::String(text) => parse_subscription_info_from_string(text),
        serde_json::Value::Array(items) => items.iter().find_map(extract_subscription_info),
        serde_json::Value::Object(map) => map
            .get("for_date")
            .and_then(extract_subscription_info)
            .or_else(|| map.values().find_map(extract_subscription_info)),
    }
}

fn parse_subscription_info_from_string(raw: &str) -> Option<SubscriptionInfo> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(timestamp) = trimmed.parse::<i64>() {
        return subscription_info_from_unix_timestamp(timestamp);
    }

    parse_subscription_datetime_to_unix(trimmed).and_then(subscription_info_from_unix_timestamp)
}

fn subscription_info_from_unix_timestamp(timestamp: i64) -> Option<SubscriptionInfo> {
    let expires_at_unix = normalize_unix_timestamp(timestamp);
    Some(SubscriptionInfo {
        expires_at_unix,
        display_date: format_unix_date_display(expires_at_unix)?,
    })
}

fn normalize_unix_timestamp(timestamp: i64) -> i64 {
    if timestamp.abs() >= 1_000_000_000_000 {
        timestamp / 1000
    } else {
        timestamp
    }
}

fn parse_subscription_datetime_to_unix(raw: &str) -> Option<i64> {
    let trimmed = raw.trim();
    if trimmed.len() < 10 {
        return None;
    }

    let year = trimmed.get(0..4)?.parse::<i32>().ok()?;
    if trimmed.get(4..5)? != "-" || trimmed.get(7..8)? != "-" {
        return None;
    }
    let month = trimmed.get(5..7)?.parse::<u32>().ok()?;
    let day = trimmed.get(8..10)?.parse::<u32>().ok()?;

    let mut index = 10usize;
    let bytes = trimmed.as_bytes();

    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    if index < bytes.len() && matches!(bytes[index], b'T' | b't') {
        index += 1;
    }
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }

    let mut hours = 0i64;
    let mut minutes = 0i64;
    let mut seconds = 0i64;
    if index + 8 <= bytes.len()
        && bytes.get(index + 2) == Some(&b':')
        && bytes.get(index + 5) == Some(&b':')
    {
        hours = trimmed.get(index..index + 2)?.parse::<i64>().ok()?;
        minutes = trimmed.get(index + 3..index + 5)?.parse::<i64>().ok()?;
        seconds = trimmed.get(index + 6..index + 8)?.parse::<i64>().ok()?;
        index += 8;
    }

    if index < bytes.len() && bytes[index] == b'.' {
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
    }

    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }

    let mut offset_seconds = 0i64;
    if index < bytes.len() {
        match bytes[index] {
            b'Z' | b'z' => {}
            b'+' | b'-' => {
                let sign = if bytes[index] == b'+' { 1i64 } else { -1i64 };
                index += 1;
                let offset_hours = trimmed.get(index..index + 2)?.parse::<i64>().ok()?;
                index += 2;
                if index < bytes.len() && bytes[index] == b':' {
                    index += 1;
                }
                let offset_minutes = if index + 2 <= bytes.len()
                    && bytes[index..index + 2]
                        .iter()
                        .all(|byte| byte.is_ascii_digit())
                {
                    trimmed.get(index..index + 2)?.parse::<i64>().ok()?
                } else {
                    0
                };
                offset_seconds = sign * (offset_hours * 3600 + offset_minutes * 60);
            }
            _ => {}
        }
    }

    let days = days_from_civil(year, month, day)?;
    let naive_seconds = days * 86_400 + hours * 3600 + minutes * 60 + seconds;
    Some(naive_seconds - offset_seconds)
}

fn format_unix_date_display(timestamp: i64) -> Option<String> {
    let seconds = normalize_unix_timestamp(timestamp);

    let days = seconds.div_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    Some(format!("{:02}.{:02}.{:04}", month, day, year))
}

fn current_unix_timestamp() -> Option<i64> {
    Some(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_secs() as i64,
    )
}

fn days_from_civil(year: i32, month: u32, day: u32) -> Option<i64> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }

    let year = year as i64 - if month <= 2 { 1 } else { 0 };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month = month as i64;
    let day = day as i64;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    Some(era * 146_097 + day_of_era - 719_468)
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }

    (year as i32, month as u32, day as u32)
}

fn quote_windows_argument(argument: &OsStr) -> String {
    let value = argument.to_string_lossy();
    if !value.contains([' ', '\t', '\n', '"']) {
        return value.into_owned();
    }

    let mut escaped = String::from("\"");
    let mut backslash_count = 0usize;

    for ch in value.chars() {
        match ch {
            '\\' => backslash_count += 1,
            '"' => {
                escaped.push_str(&"\\".repeat(backslash_count * 2 + 1));
                escaped.push('"');
                backslash_count = 0;
            }
            _ => {
                escaped.push_str(&"\\".repeat(backslash_count));
                escaped.push(ch);
                backslash_count = 0;
            }
        }
    }

    escaped.push_str(&"\\".repeat(backslash_count * 2));
    escaped.push('"');
    escaped
}

fn remove_legacy_subscription_check_task() -> Result<(), String> {
    let mut command = std::process::Command::new("schtasks");
    command.args(["/Delete", "/TN", SUBSCRIPTION_SCHEDULED_TASK_NAME, "/F"]);

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let output = command.output().map_err(|error| {
        format!(
            "Не удалось запустить schtasks для удаления задачи: {}",
            error
        )
    })?;

    if output.status.success() || String::from_utf8_lossy(&output.stderr).contains("cannot find") {
        Ok(())
    } else {
        Err(format!(
            "schtasks /Delete завершился с кодом {}",
            output.status.code().unwrap_or(-1)
        ))
    }
}

fn cleanup_legacy_subscription_notifications() {
    clear_legacy_subscription_notification_state();
    if let Err(error) = remove_legacy_subscription_check_task() {
        eprintln!(
            "⚠ Не удалось удалить устаревшую задачу проверки подписки: {}",
            error
        );
    }
}

fn initialize_app_environment(reset_runtime_state: bool) {
    match app_dirs::AppDirs::init() {
        Ok(app_dirs) => {
            if reset_runtime_state {
                if let Err(error) = app_dirs.reset_runtime_state() {
                    eprintln!("⚠ Ошибка очистки runtime-временных файлов: {}", error);
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
        }
        Err(error) => {
            eprintln!("⚠ Ошибка инициализации директорий: {}", error);
        }
    }

    #[cfg(target_os = "windows")]
    if let Err(error) = super::app_windows::ensure_notification_shortcut_registered() {
        eprintln!(
            "⚠ Не удалось зарегистрировать ярлык уведомлений для {}: {}",
            NOTIFICATION_APP_ID, error
        );
    }

    #[cfg(target_os = "windows")]
    configure_process_notification_identity();
}

fn run_legacy_subscription_check_cleanup_mode() -> ! {
    cleanup_legacy_subscription_notifications();
    std::process::exit(0);
}

pub(crate) fn launch_self_elevated(arguments: &[OsString]) -> Result<(), String> {
    let exe = match env::current_exe() {
        Ok(path) => path,
        Err(e) => return Err(format!("Не удалось определить путь к приложению: {}", e)),
    };

    let exe_w: Vec<u16> = exe.as_os_str().encode_wide().chain(Some(0)).collect();
    let parameters = arguments
        .iter()
        .map(|argument| quote_windows_argument(argument.as_os_str()))
        .collect::<Vec<_>>()
        .join(" ");
    let params_w: Vec<u16> = OsStr::new(&parameters)
        .encode_wide()
        .chain(Some(0))
        .collect();
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("runas"),
            PCWSTR(exe_w.as_ptr()),
            if parameters.is_empty() {
                PCWSTR::null()
            } else {
                PCWSTR(params_w.as_ptr())
            },
            PCWSTR::null(),
            SW_HIDE,
        )
    };

    if (result.0 as isize) > 32 {
        Ok(())
    } else {
        Err(format!(
            "Не удалось запустить elevated helper (ShellExecuteW code {})",
            result.0 as isize
        ))
    }
}

fn check_single_instance() -> bool {
    let title_wide: Vec<u16> = OsStr::new(WINDOW_TITLE)
        .encode_wide()
        .chain(Some(0))
        .collect();
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

pub(crate) fn ensure_firewall_rules() -> Result<(), String> {
    if !is_elevated() {
        return Ok(());
    }

    let deps = embedded_deps_bytes::ExtractedDeps::get().map_err(|e| {
        format!(
            "Не удалось получить пути к зависимостям для брандмауэра: {}",
            e
        )
    })?;

    install_firewall_rules(
        deps.wireproxy.to_string_lossy().as_ref(),
        deps.proxybridge_cli.to_string_lossy().as_ref(),
    )
}

fn install_firewall_rules(wireproxy_path: &str, proxybridge_path: &str) -> Result<(), String> {
    let script = format!(
        r#"
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
            description="Разрешение для vpnfybot-windows - автоматически добавлено"

        if ($LASTEXITCODE -eq 0) {{
            Write-Host "✓ Добавлено правило: $RuleName" -ForegroundColor Green
        }} else {{
            Write-Host "⚠ Ошибка при добавлении правила: $RuleName" -ForegroundColor Yellow
            exit 1
        }}
    }} catch {{
        Write-Host "✗ Исключение при установке правила $($RuleName): $_" -ForegroundColor Red
        exit 1
    }}
}}

Set-FirewallRule -RuleName "vpnfybot-windows - wireproxy (incoming)" -ProgramPath "{wireproxy_path}"
Set-FirewallRule -RuleName "vpnfybot-windows - ProxyBridge (incoming)" -ProgramPath "{proxybridge_path}"
"#
    );

    let mut cmd = std::process::Command::new("powershell");
    cmd.args([
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        &script,
    ])
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null());

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd.spawn().map_err(|e| {
        format!(
            "Не удалось запустить PowerShell для установки правил: {}",
            e
        )
    })?;

    let status = child.wait().map_err(|e| {
        format!(
            "Ошибка ожидания процесса установки правил брандмауэра: {}",
            e
        )
    })?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "Установка правил брандмауэра завершилась с кодом {}",
            status.code().unwrap_or(-1)
        ))
    }
}

fn setup_firewall_rules() {
    if !is_elevated() {
        return;
    }

    thread::spawn(|| match ensure_firewall_rules() {
        Ok(()) => {
            eprintln!("✓ Правила брандмауэра успешно установлены");
        }
        Err(error) => {
            eprintln!("⚠ Ошибка при установке правил брандмауэра: {}", error);
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
    let args: Vec<OsString> = env::args_os().collect();
    if args.len() >= 2 && args[1] == OsStr::new("/stop-proxybridge") {
        run_stop_proxybridge_mode();
    }
    if args.len() >= 3 && args[1] == OsStr::new("/start-proxybridge") {
        run_start_proxybridge_mode(&args[2]);
    }
    if args.len() >= 2 && args[1] == OsStr::new(SUBSCRIPTION_SCHEDULED_TASK_ARG) {
        run_legacy_subscription_check_cleanup_mode();
    }
    if args.len() >= 3 && args[1] == OsStr::new("/service") {
        let info_addr = args.get(3).map(|value| value.as_os_str());
        run_wireproxy_mode(&args[2], info_addr);
    }
    if args.len() >= 3 && args[1] == OsStr::new("/stop-service") {
        run_stop_wireproxy_mode(&args[2]);
    }

    if !check_single_instance() {
        std::process::exit(0);
    }

    initialize_app_environment(true);

    if let Err(error) = restore_tunnel_dns() {
        log::warn!("Failed to restore stale DNS state on startup: {}", error);
    }

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
            from_png_bytes(include_bytes!("../../gifs/vpnfy.png")).expect("Failed to load icon"),
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

fn run_wireproxy_mode(conf: &OsStr, info_addr: Option<&OsStr>) -> ! {
    let _ = app_dirs::AppDirs::init();

    let conf_path = conf.to_string_lossy();

    let deps = match embedded_deps_bytes::ExtractedDeps::get() {
        Ok(paths) => paths,
        Err(e) => {
            eprintln!("Не удалось получить зависимости: {}", e);
            std::process::exit(1);
        }
    };

    if let Err(error) = apply_tunnel_dns(conf_path.as_ref()) {
        eprintln!(
            "Failed to apply tunnel DNS before wireproxy start: {}",
            error
        );
    }

    let mut command = std::process::Command::new(&deps.wireproxy);
    command
        .arg("-c")
        .arg(conf_path.as_ref())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    if let Some(info_addr) = info_addr {
        command
            .arg("--info")
            .arg(info_addr.to_string_lossy().as_ref());
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    match command.spawn() {
        Ok(mut child) => {
            // The installer already creates these rules. Refresh them in the background for
            // portable/self-updated builds without delaying tunnel readiness.
            setup_firewall_rules();
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

fn run_stop_wireproxy_mode(conf: &OsStr) -> ! {
    let result = stop_and_delete_service(conf.to_string_lossy().as_ref());
    if let Some(error_log) = result.error_log {
        eprintln!("{}", error_log);
        std::process::exit(1);
    }

    eprintln!("{}", result.message);
    std::process::exit(0);
}

fn run_stop_proxybridge_mode() -> ! {
    match stop_proxybridge() {
        Ok(_) => std::process::exit(0),
        Err(error) => {
            eprintln!("{}", error);
            std::process::exit(1);
        }
    }
}

fn run_start_proxybridge_mode(batch_path: &OsStr) -> ! {
    let mut command = std::process::Command::new("cmd");
    command
        .arg("/C")
        .arg(batch_path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    match command.spawn() {
        Ok(_) => std::process::exit(0),
        Err(error) => {
            eprintln!("Не удалось запустить ProxyBridge: {}", error);
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
