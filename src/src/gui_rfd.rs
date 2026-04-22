#[cfg(target_os = "windows")]
#[allow(dead_code, non_snake_case)]
fn LOWORD(l: u32) -> u16 {
    (l & 0xffff) as u16
}
#[cfg(target_os = "windows")]
#[allow(dead_code)]
unsafe extern "system" fn connect_button_wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::*;
    match msg {
        WM_COMMAND => {
            let button_id = LOWORD(wparam.0 as u32);
            if button_id == 1001 {
                // Здесь вызываем Rust-логику подключения/отключения
                // TODO: Передать сигнал в AppState (например, через глобальный Mutex/AtomicBool)
            }
        }
        _ => {}
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}
use eframe::{egui, App, Frame};
use eframe::icon_data::from_png_bytes;
use image::codecs::gif::GifDecoder;
use image::AnimationDecoder;
#[allow(deprecated)]
use raw_window_handle::{HasRawWindowHandle, HasWindowHandle, RawWindowHandle};
use resvg::{tiny_skia, usvg};
use rfd::FileDialog;
use std::collections::BTreeSet;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{Cursor, Read, Write};
use std::net::{SocketAddr, ToSocketAddrs};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::sync::{mpsc::{self, Receiver}, Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use winrt_notification::Toast;
use windows::Data::Xml::Dom::XmlDocument;
use windows::core::{HSTRING, PCWSTR, w};
use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
use windows::Win32::Foundation::{BOOL, COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR};
use windows::Win32::Graphics::Gdi::{
    CreateBitmap, CreateFontW, CreateCompatibleBitmap, CreateCompatibleDC,
    DrawTextW,
    DeleteObject, DeleteDC, GetDC, GetDeviceCaps, GetDIBits, HFONT, LOGPIXELSY,
    ReleaseDC, SelectObject, SetBkMode, SetTextColor, BITMAPINFO, BITMAPINFOHEADER,
    DIB_RGB_COLORS, FillRect, CreateSolidBrush,
    DT_CENTER, DT_SINGLELINE, DT_VCENTER, TRANSPARENT,
};
use windows::Win32::UI::Shell::{DragAcceptFiles, DragFinish, DragQueryFileW, HDROP, NIIF_INFO, SetCurrentProcessExplicitAppUserModelID, ShellExecuteW, Shell_NotifyIconW, NOTIFYICONDATAW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY};
use windows::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, CallWindowProcW, CreateIconIndirect, FindWindowW,
    DestroyIcon, SendMessageW,
    SetForegroundWindow, SetWindowLongPtrW,
    ShowWindow, GWLP_WNDPROC, HICON, ICONINFO, WINDOW_EX_STYLE, WINDOW_STYLE,
    WM_APP, WM_DROPFILES, WM_LBUTTONUP, WM_RBUTTONUP, WM_SETFONT, WM_SIZE, WNDPROC,
    SIZE_MINIMIZED, SW_HIDE, SW_RESTORE, SW_SHOWNORMAL,
};

use windows::Win32::UI::WindowsAndMessaging::{WM_NCLBUTTONDOWN, HTMINBUTTON};

#[path = "embedded_deps_bytes.rs"]
mod embedded_deps_bytes;
#[path = "app_dirs.rs"]
mod app_dirs;
#[path = "update_check.rs"]
mod update_check;
#[path = "gui_rfd/app_storage.rs"]
mod app_storage;
#[path = "gui_rfd/tunnel_service.rs"]
mod tunnel_service;
#[path = "gui_rfd/ui_helpers.rs"]
mod ui_helpers;
#[path = "gui_rfd/error_dialog.rs"]
mod error_dialog;
#[path = "gui_rfd/process_editor.rs"]
mod process_editor;
#[path = "gui_rfd/site_editor.rs"]
mod site_editor;

use self::app_storage::*;
use self::tunnel_service::*;
use self::ui_helpers::*;

struct ServiceResult {
    message: String,
    active: bool,
    error_log: Option<String>,
    wireproxy_info_addr: Option<String>,
}

const APP_TITLE: &str = "vpnfybot-windows";
const WINDOW_TITLE: &str = "vpnfybot-windows";
const NOTIFICATION_APP_ID: &str = "vpnfybot-windows";
const SITES_EDITOR_CLASS: &str = "vpnfy_sites_editor_class";
const SITE_TEXT_CONTAINER_CLASS: &str = "vpnfy_sites_text_container_class";
const PROCESSES_EDITOR_CLASS: &str = "vpnfy_processes_editor_class";
const PROCESS_LIST_CLASS: &str = "vpnfy_process_list_class";
const PROCESS_LIST_ITEM_HEIGHT: i32 = 28;
const PROCESS_LIST_WHEEL_STEP: usize = 3;
const PROCESS_LIST_SCROLLBAR_GAP: i32 = 6;
const PROCESS_EDITOR_PADDING: i32 = 8;
const PROCESS_EDITOR_GAP: i32 = 8;
const PROCESS_SAVE_BUTTON_HEIGHT: i32 = 28;
const MAIN_WINDOW_CLIENT_WIDTH: i32 = 320;
const MAIN_WINDOW_CLIENT_HEIGHT: i32 = 380;
const SITE_SCROLLBAR_WIDTH: i32 = 8;
const SITE_TEXT_LINE_HEIGHT: i32 = 20;
const SITE_WHEEL_STEP: i32 = 3;
const UI_BUTTON_FONT_SIZE: f32 = 14.0;
const BUTTON_FONT_FAMILY_NAME: &str = "vpnfy_button_font";
const TUNNEL_TRAFFIC_POLL_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq)]
enum Language {
    En,
    Ru,
}

impl Language {
    fn next(self) -> Self {
        match self {
            Language::En => Language::Ru,
            Language::Ru => Language::En,
        }
    }

    fn code(self) -> &'static str {
        match self {
            Language::En => "EN",
            Language::Ru => "RU",
        }
    }

    fn translate(&self, key: &str) -> String {
        match self {
            Language::En => match key {
                "Импорт" => "Import",
                "Подключиться" => "Connect",
                "Отключиться" => "Disconnect",
                "Сначала импортируйте .conf файл" => "Please import .conf file first",
                "Нужны права администратора. Запустите приложение от имени администратора" => "Administrator rights required. Run the app as administrator",
                "Отключите туннель перед импортом конфигурации" => "Disconnect the tunnel before importing configuration",
                "Вся система" => "Whole system",
                "Выбранные приложения" => "Selected applications",
                "Подключен" => "Connected",
                "Отключен" => "Disconnected",
                "Туннель подключен" => "Tunnel connected",
                "Выберите процессы для маршрутизации" => "Select processes for routing",
                "Запуск ProxyBridge для выбранных процессов" => "Starting ProxyBridge for selected processes",
                "Запуск ProxyBridge для всей системы через VPN" => "Starting ProxyBridge for entire system via VPN",
                "Запуск ProxyBridge с исключениями" => "Starting ProxyBridge with exceptions",
                "Ошибка остановки ProxyBridge" => "ProxyBridge stop error",
                "Ошибка" => "Error",
                "Не удалось открыть ссылку" => "Failed to open link",
                "Процессы" => "Processes",
                "Сайты через VPN" => "Sites via VPN",
                "Исключенные сайты" => "Excluded sites",
                "Приложения через VPN" => "Apps via VPN",
                "Исключенные приложения" => "Excluded applications",
                "Введите сайты, которые должны работать через VPN" => "Enter sites that should work through VPN",
                "Режим VPN" => "VPN mode",
                "В режиме \"Вся система\" сайты из списка \"Исключенные сайты\" и приложения из списка \"Исключенные приложения\" будут исключены из VPN туннеля" => "In the \"Whole system\" mode, sites from the \"Excluded sites\" list and apps from the \"Excluded applications\" list will be excluded from the VPN tunnel",
                "В режиме \"Выбранные приложения\" сайты из списка \"Сайты через VPN\" и приложения из списка \"Приложения через VPN\" будут идти через VPN туннель" => "In the \"Selected applications\" mode, sites from the \"Sites via VPN\" list and apps from the \"Apps via VPN\" list will go through the VPN tunnel",
                "Сохранить" => "Save",
                "Закрыть" => "Close",
                "Сайты" => "Sites",
                "Экспорт" => "Export",
                "Поиск" => "Search",
                "Доступна новая версия: v{}" => "New version available: v{}",
                "Доступна новая версия" => "New version available",
                "Загрузка" => "Downloading",
                "Проверка обновлений..." => "Checking for updates...",
                "Проверка обновлений" => "Checking for updates",
                "Установить" => "Install",
                "Позже" => "Later",
                _ => key,
            },
            Language::Ru => key,
        }.to_string()
    }
}

struct AppState {
    conf_path: Option<String>,
    status: String,
    error_log: Option<String>,
    status_rx: Option<Receiver<ServiceResult>>,
    service_running: bool,
    service_active: bool,
    elevated: bool,
    session_traffic_bytes: u64,
    session_base_traffic_bytes: Option<u64>,
    connected_at: Option<Instant>,
    startup_animation_frame: usize,
    wireproxy_info_addr: Option<String>,
    last_tunnel_traffic_poll: Option<Instant>,
    // Last time the central time/traffic text was updated (throttle to 1s)
    last_time_display_update: Option<Instant>,
    // Cached string for the central time/traffic display (updated once per second)
    cached_time_display: String,
    // Cached strings for upload/download numeric displays (updated together with `cached_time_display`)
    cached_up_display: String,
    cached_down_display: String,
    // Last observed totals from wireproxy metrics: (tx_bytes, rx_bytes)
    last_tunnel_totals: Option<(u64, u64)>,
    // Latest computed speeds in bytes/sec
    last_upload_bps: f64,
    last_download_bps: f64,
    upload_icon: Option<egui::TextureHandle>,
    download_icon: Option<egui::TextureHandle>,
    top_image: Option<egui::TextureHandle>,
    settings_icon: Option<egui::TextureHandle>,
    settings_close_icon: Option<egui::TextureHandle>,
    language_icon: Option<egui::TextureHandle>,
    animated_frames: Option<Vec<egui::TextureHandle>>,
    animated_frame_durations: Vec<u64>,
    animated_frame_index: usize,
    animated_last_frame: Instant,
    gif_pulse_start: Option<Instant>,
    gif_rotation_start: Instant,
    window_frame_styled: bool,
    window_frame_attempts: u32, // Счётчик попыток применения чёрной рамки
    tray_subclassed: bool,
    tray_icon_added: bool,
    tray_window: Option<HWND>,
    tray_icon: Option<HICON>,
    traffic_opacity: f32,
    import_button_opacity: f32,
    import_button_opacity_target: f32,
    connect_animation_start: Option<Instant>,
    disconnect_animation_start: Option<Instant>,
    last_notification: Option<ToastNotification>,
    update_pending: Option<update_check::UpdateAvailable>,
    proxybridge_running: bool,
    selected_processes: Vec<String>,
    selected_sites: Vec<String>,
    process_window_receiver: Option<Receiver<Option<Vec<String>>>>,
    site_window_receiver: Option<Receiver<Option<String>>>,
    show_settings: bool,
    settings_tab: String,
    cached_processes: Vec<String>,
    last_process_refresh: Option<Instant>,
    process_search_text: String,
    proxy_mode_toggle: bool, // true = выбранные приложения через VPN, false = вся система через VPN
    proxybridge_child: Option<std::process::Child>, // Храним handle процесса ProxyBridge
    language: Language, // Текущий язык интерфейса

    // Cache for textures generated from WinAPI text rendering
    win_text_cache: std::collections::HashMap<String, egui::TextureHandle>,
    // Single HFONT instance for button rendering (created once)
    button_hfont: Option<HFONT>,
    // Lighter HFONT for non-bold mode text (used for "Вся система")
    button_hfont_light: Option<HFONT>,
    // HFONT for speed labels rendered without smoothing (removed - unused)
    
}

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
            // connect_button_hwnd удалён
        };
        // Run an update check at startup (background thread)
        update_check::spawn_update_check_thread();
        s
    }
}

impl App for AppState {
    fn update(&mut self, ctx: &egui::Context, frame: &mut Frame) {
        if ctx.input(|i| i.key_pressed(egui::Key::Num4)) {
            self.reset_app_settings();
        }

        let mut style = (*ctx.style()).clone();
        style.visuals.widgets.inactive.fg_stroke.color = egui::Color32::BLACK;
        style.visuals.widgets.hovered.fg_stroke.color = egui::Color32::BLACK;
        style.visuals.widgets.active.fg_stroke.color = egui::Color32::BLACK;
        style.visuals.widgets.open.fg_stroke.color = egui::Color32::BLACK;
        ctx.set_style(style);

        if !self.window_frame_styled {
            self.window_frame_styled = true;
            #[cfg(target_os = "windows")]
            {
                self.apply_black_window_frame(frame);
            }
        }
        
        // Продолжаем пытаться применить цвет рамки в течение первых 60 кадров
        #[cfg(target_os = "windows")]
        {
            if self.window_frame_attempts < 60 {
                self.window_frame_attempts += 1;
                self.apply_black_window_frame(frame);
            }
        }
        
        // (WinAPI Connect button удалён, egui-кнопки возвращены)

        // If a background update check queued an update, move it into state
        if self.update_pending.is_none() {
            if let Some(mutex) = update_check::UPDATE_AVAILABLE.get() {
                if let Ok(mut guard) = mutex.lock() {
                    if let Some(info) = guard.take() {
                        self.update_pending = Some(info);
                    }
                }
            }
        }

        // If we have a pending update, draw a centered modal with semi-opaque overlay
        if let Some(info) = &self.update_pending {
            let info = info.clone();
            let available = ctx.available_rect();
            egui::Area::new("update_modal_full".into())
                .fixed_pos(available.min)
                .show(ctx, |ui| {
                    ui.set_min_size(available.size());
                    // Consume clicks so background UI is not interactive
                    let _bg_resp = ui.allocate_rect(available, egui::Sense::click());

                    // Overlay: semi-opaque black (80% opacity)
                    let overlay_alpha = (0.80_f32 * 255.0_f32).round() as u8;
                    let overlay_color = egui::Color32::from_rgba_unmultiplied(0, 0, 0, overlay_alpha);
                    ui.painter().rect_filled(available, 0.0, overlay_color);

                    // Centered content panel
                    let max_content_w = (available.width() - 40.0).max(320.0);
                    let content_w = (available.width() * 0.7).clamp(320.0, max_content_w);
                    let content_h = 260.0_f32;
                    // Shift modal contents down by 20px (overall adjustment)
                    let content_rect = egui::Rect::from_center_size(available.center() + egui::vec2(0.0, 20.0), egui::vec2(content_w, content_h));
                    // No panel background: rely on semi-opaque overlay; draw only controls and text above it.
                    let mut content_ui = ui.child_ui(content_rect, egui::Layout::top_down(egui::Align::Center));
                    // shift all modal elements further down by 24px (increase top spacing)
                    content_ui.add_space(80.0);

                    // Progress area: show a single text object that either appears inside
                    // the progress bar (when not downloading) or above it (when downloading).
                    // Ensure the text uses the same font size in both states and the bar
                    // width matches the Install button width.
                    let downloading = update_check::UPDATE_DOWNLOADING.get().map(|a| a.load(std::sync::atomic::Ordering::Relaxed)).unwrap_or(false);
                    let progress_percent = update_check::UPDATE_DOWNLOAD_PROGRESS.get().map(|p| p.load(std::sync::atomic::Ordering::Relaxed)).unwrap_or(0usize);

                    // Make bar width equal to Install button width
                    let button_width = 220.0f32;
                    let bar_size = egui::vec2(button_width, 18.0);

                    // Single label text for both states
                    let label_text = if downloading {
                        self.language.translate("Загрузка")
                    } else {
                        self.language.translate("Доступна новая версия")
                    };

                    // Allocate the bar area once so layout and buttons stay stable
                    let (bar_rect, _) = content_ui.allocate_exact_size(bar_size, egui::Sense::hover());

                    if downloading {
                        // Draw the bar background only while downloading
                        let bar_bg = egui::Color32::from_rgba_unmultiplied(255, 255, 255, (0.20_f32 * 255.0_f32).round() as u8);
                        content_ui.painter().rect_filled(bar_rect, 9.0, bar_bg);

                        // Draw the label above the bar (8px gap) without changing layout
                        let label_pos = egui::pos2(bar_rect.center().x, bar_rect.min.y - 8.0);
                        content_ui.painter().text(
                            label_pos,
                            egui::Align2::CENTER_BOTTOM,
                            label_text,
                            egui::FontId::proportional(UI_BUTTON_FONT_SIZE),
                            egui::Color32::WHITE,
                        );

                        let progress_ratio = (progress_percent as f32 / 100.0).clamp(0.0, 1.0);
                        let fill_w = bar_rect.width() * progress_ratio;
                        if fill_w > 0.0 {
                            let fill_rect = egui::Rect::from_min_max(bar_rect.min, egui::pos2(bar_rect.min.x + fill_w, bar_rect.max.y));
                            content_ui.painter().rect_filled(fill_rect, 9.0, egui::Color32::WHITE);
                        }
                        // Percentage in black for readability over white bar
                        content_ui.painter().text(
                            bar_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            format!("{}%", progress_percent),
                            egui::FontId::proportional(14.0),
                            egui::Color32::BLACK,
                        );
                    } else {
                        // Not downloading: do not draw bar background — keep area reserved and show label inside
                        content_ui.painter().text(
                            bar_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            label_text,
                            egui::FontId::proportional(UI_BUTTON_FONT_SIZE),
                            egui::Color32::WHITE,
                        );
                    }

                    content_ui.add_space(14.0);

                    // Centered vertical layout: Install, then white "Позже" below
                    content_ui.vertical_centered(|ui| {
                        // Install button
                        let install_size = egui::vec2(220.0, 40.0);
                        let (install_rect, install_resp) = ui.allocate_exact_size(install_size, egui::Sense::click());
                        let install_hover_alpha = if install_resp.is_pointer_button_down_on() {
                            (255f32 * 0.50).round() as u8
                        } else if install_resp.hovered() {
                            (255f32 * 0.80).round() as u8
                        } else {
                            255u8
                        };
                        let enabled = !downloading;
                        let install_alpha = if enabled { install_hover_alpha } else { (255f32 * 0.45).round() as u8 };
                        let install_fill = egui::Color32::from_rgba_unmultiplied(220, 220, 220, install_alpha);
                        ui.painter().rect_filled(install_rect, 6.0, install_fill);
                        ui.painter().rect_stroke(install_rect, 6.0, egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(0,0,0, install_hover_alpha)));
                        #[cfg(target_os = "windows")]
                        {
                            let label = self.language.translate("Установить");
                            let ppp = ctx.pixels_per_point();
                            let w_px = (install_rect.width() * ppp).ceil() as usize;
                            let h_px = (install_rect.height() * ppp).ceil() as usize;
                            let key = format!("install_button:{}:{}:{}", label, w_px, h_px);
                            let text_color = egui::Color32::from_rgba_unmultiplied(0, 0, 0, install_hover_alpha);
                            if let Some(tex) = self.win_text_cache.get(&key) {
                                ui.painter().image(tex.id(), install_rect, egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(1.0,1.0)), egui::Color32::WHITE);
                            } else if let Some(tex) = win_text_to_texture(ctx, &key, &label, self.button_hfont, text_color, w_px, h_px) {
                                self.win_text_cache.insert(key.clone(), tex.clone());
                                ui.painter().image(tex.id(), install_rect, egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(1.0,1.0)), egui::Color32::WHITE);
                            } else {
                                ui.painter().text(install_rect.center(), egui::Align2::CENTER_CENTER, "Установить", egui::FontId::proportional(UI_BUTTON_FONT_SIZE), egui::Color32::BLACK);
                            }
                        }
                        #[cfg(not(target_os = "windows"))]
                        {
                            let label = self.language.translate("Установить");
                            ui.painter().text(install_rect.center(), egui::Align2::CENTER_CENTER, &label, egui::FontId::proportional(UI_BUTTON_FONT_SIZE), egui::Color32::BLACK);
                        }
                        if install_resp.is_pointer_button_down_on() {
                            ctx.set_cursor_icon(egui::CursorIcon::Default);
                        } else if install_resp.hovered() {
                            ctx.set_cursor_icon(if !downloading { egui::CursorIcon::PointingHand } else { egui::CursorIcon::NotAllowed });
                        }
                        if install_resp.clicked() && !downloading {
                            // Start download into exe directory with progress updates
                            let dl_url = info.download_url.clone();
                            let fname = info.asset_name.clone();
                            let progress_atomic = update_check::UPDATE_DOWNLOAD_PROGRESS.get_or_init(|| std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0))).clone();
                            let downloading_flag = update_check::UPDATE_DOWNLOADING.get_or_init(|| std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false))).clone();
                            progress_atomic.store(0, std::sync::atomic::Ordering::Relaxed);
                            downloading_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                            std::thread::spawn(move || {
                                let agent = "vpnfybot-windows-update-install";
                                if let Ok(resp) = ureq::get(&dl_url).set("User-Agent", agent).call() {
                                    let total_opt = resp.header("Content-Length").and_then(|s| s.parse::<usize>().ok());
                                    // Directory where current exe lives (fallbacks to CWD or temp)
                                    let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|pp| pp.to_path_buf())).unwrap_or_else(|| std::env::current_dir().unwrap_or(std::env::temp_dir()));

                                    // Determine whether the downloaded asset looks like our app (replace candidate)
                                    let asset_stem = std::path::Path::new(&fname).file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_lowercase();
                                    let cur_no_ext_opt = std::env::current_exe().ok().and_then(|p| p.file_name().and_then(|n| n.to_str()).map(|s| s.trim_end_matches(".exe").to_string().to_lowercase()));
                                    let replace_candidate = match &cur_no_ext_opt {
                                        Some(cur_no_ext) => {
                                            asset_stem == *cur_no_ext || asset_stem.contains("vpnfy") || cur_no_ext.contains(&asset_stem) || asset_stem.contains(cur_no_ext)
                                        }
                                        None => asset_stem.contains("vpnfy"),
                                    };

                                    // Choose download filename/location. If this is a replacement candidate,
                                    // download into the system temp dir with the final base name `vpnfybot-windows.exe`
                                    // so that after replacement the installed file has that canonical name.
                                    let downloaded_basename = if replace_candidate {
                                        "vpnfybot-windows.exe".to_string()
                                    } else {
                                        std::path::Path::new(&fname).file_name().and_then(|n| n.to_str()).unwrap_or("update_installer.exe").to_string()
                                    };

                                    let download_path = if replace_candidate {
                                        std::env::temp_dir().join(&downloaded_basename)
                                    } else {
                                        exe_dir.join(&downloaded_basename)
                                    };

                                    if let Ok(mut file) = std::fs::File::create(&download_path) {
                                        let mut reader = resp.into_reader();
                                        let mut buf = [0u8; 8192];
                                        let mut downloaded: usize = 0;
                                        loop {
                                            match reader.read(&mut buf) {
                                                Ok(0) => break,
                                                Ok(n) => {
                                                    if file.write_all(&buf[..n]).is_err() { break; }
                                                    downloaded += n;
                                                    if let Some(total) = total_opt {
                                                        let pct = ((downloaded as f64 / total as f64) * 100.0).round() as usize;
                                                        progress_atomic.store(pct.min(100), std::sync::atomic::Ordering::Relaxed);
                                                    } else {
                                                        let prev = progress_atomic.load(std::sync::atomic::Ordering::Relaxed);
                                                        let next = (prev + 1).min(99);
                                                        progress_atomic.store(next, std::sync::atomic::Ordering::Relaxed);
                                                    }
                                                }
                                                Err(_) => break,
                                            }
                                        }
                                        progress_atomic.store(100, std::sync::atomic::Ordering::Relaxed);

                                        #[cfg(target_os = "windows")]
                                        {
                                            // Decide whether to replace current exe or just run the downloaded file.
                                            let downloaded_path = download_path.clone();
                                            if let Ok(current_exe) = std::env::current_exe() {
                                                let current_name = current_exe
                                                    .file_name()
                                                    .and_then(|n| n.to_str())
                                                    .unwrap_or_default()
                                                    .to_lowercase();
                                                let cur_no_ext = current_name.trim_end_matches(".exe").to_string();
                                                let fname_no_ext = std::path::Path::new(&downloaded_basename)
                                                    .file_stem()
                                                    .and_then(|s| s.to_str())
                                                    .unwrap_or_default()
                                                    .to_lowercase();

                                                let replace_candidate_after = fname_no_ext == cur_no_ext
                                                    || fname_no_ext.contains("vpnfy")
                                                    || cur_no_ext.contains(&fname_no_ext)
                                                    || fname_no_ext.contains(&cur_no_ext);

                                                if replace_candidate_after {
                                                    // Create a small PowerShell script that waits for this process to exit, moves the downloaded exe into place, and launches it.
                                                    let script_name = format!("vpnfy_update_{}.ps1", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0u128));
                                                    let script_path = std::env::temp_dir().join(&script_name);
                                                    let src = downloaded_path.display().to_string().replace("'", "''");
                                                    let dst = current_exe.display().to_string().replace("'", "''");
                                                    let procname = cur_no_ext.replace("'", "''");
                                                    let script = format!(
    r#"$src = '{src}'
    $dst = '{dst}'
    $proc = '{proc}'
    Start-Sleep -Milliseconds 500
    $tries = 0
    while (Get-Process -Name $proc -ErrorAction SilentlyContinue) {{
        Start-Sleep -Seconds 1
        $tries += 1
        if ($tries -gt 120) {{ exit 1 }}
    }}
    $success = $false
    $tries = 0
    while (-not $success -and $tries -lt 120) {{
        try {{
            Move-Item -Path $src -Destination $dst -Force -ErrorAction Stop
            $success = $true
        }} catch {{
            Start-Sleep -Milliseconds 2500
            $tries += 1
        }}
    }}
    if ($success) {{
        Start-Process -FilePath $dst
        Remove-Item -Path $MyInvocation.MyCommand.Path -Force -ErrorAction SilentlyContinue
        exit 0
    }} else {{
        exit 1
    }}
    "#, src=src, dst=dst, proc=procname);

                                                    let _ = std::fs::write(&script_path, script.as_bytes());

                                                    use std::os::windows::process::CommandExt;
                                                    const CREATE_NO_WINDOW: u32 = 0x08000000;
                                                    let mut cmd = std::process::Command::new("powershell");
                                                    cmd.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", script_path.to_str().unwrap_or_default()]);
                                                    cmd.creation_flags(CREATE_NO_WINDOW);
                                                    if let Ok(_) = cmd.spawn() {
                                                        // Exit current process so the script can replace the exe.
                                                        std::process::exit(0);
                                                    } else {
                                                        // Fallback: try to spawn the downloaded file and exit.
                                                        let _ = std::process::Command::new(&downloaded_path).spawn();
                                                        std::process::exit(0);
                                                    }
                                                } else {
                                                    // Not a replacement candidate — just run the downloaded installer/app and exit.
                                                    let _ = std::process::Command::new(downloaded_path).spawn();
                                                    std::process::exit(0);
                                                }
                                            } else {
                                                // Couldn't determine current exe — fallback to launching downloaded file.
                                                let _ = std::process::Command::new(download_path).spawn();
                                                std::process::exit(0);
                                            }
                                        }
                                    }
                                }
                                downloading_flag.store(false, std::sync::atomic::Ordering::Relaxed);
                            });
                        }

                        ui.add_space(8.0);
                        let later_size = install_size; // same width/height as Install
                        let (later_rect, later_resp) = ui.allocate_exact_size(later_size, egui::Sense::click());
                        // Match Install button style: same fill/shade and font rendering
                        let later_hover_alpha = if later_resp.is_pointer_button_down_on() {
                            (255f32 * 0.50).round() as u8
                        } else if later_resp.hovered() {
                            (255f32 * 0.80).round() as u8
                        } else {
                            255u8
                        };
                        // Use disconnect-style red shade for "Позже"; dim when disabled
                        let later_alpha = if enabled { later_hover_alpha } else { (255f32 * 0.45).round() as u8 };
                        let later_fill = egui::Color32::from_rgba_unmultiplied(180, 80, 80, later_alpha);
                        ui.painter().rect_filled(later_rect, 6.0, later_fill);
                        ui.painter().rect_stroke(later_rect, 6.0, egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(0,0,0, later_hover_alpha)));
                        #[cfg(target_os = "windows")]
                        {
                            let label = self.language.translate("Позже");
                            let ppp = ctx.pixels_per_point();
                            let w_px = (later_rect.width() * ppp).ceil() as usize;
                            let h_px = (later_rect.height() * ppp).ceil() as usize;
                            let key = format!("later_button:{}:{}:{}", label, w_px, h_px);
                            let text_color = egui::Color32::from_rgba_unmultiplied(0, 0, 0, later_hover_alpha);
                            if let Some(tex) = self.win_text_cache.get(&key) {
                                ui.painter().image(tex.id(), later_rect, egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(1.0,1.0)), egui::Color32::WHITE);
                            } else if let Some(tex) = win_text_to_texture(ctx, &key, &label, self.button_hfont, text_color, w_px, h_px) {
                                self.win_text_cache.insert(key.clone(), tex.clone());
                                ui.painter().image(tex.id(), later_rect, egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(1.0,1.0)), egui::Color32::WHITE);
                            } else {
                                ui.painter().text(later_rect.center(), egui::Align2::CENTER_CENTER, &label, egui::FontId::proportional(UI_BUTTON_FONT_SIZE), egui::Color32::BLACK);
                            }
                        }
                        #[cfg(not(target_os = "windows"))]
                        {
                            let label = self.language.translate("Позже");
                            ui.painter().text(later_rect.center(), egui::Align2::CENTER_CENTER, &label, egui::FontId::proportional(UI_BUTTON_FONT_SIZE), egui::Color32::BLACK);
                        }
                        if later_resp.is_pointer_button_down_on() {
                            ctx.set_cursor_icon(egui::CursorIcon::Default);
                        } else if later_resp.hovered() {
                            ctx.set_cursor_icon(if !downloading { egui::CursorIcon::PointingHand } else { egui::CursorIcon::NotAllowed });
                        }
                        if later_resp.clicked() && !downloading {
                            self.update_pending = None;
                        }
                    });

                    // Keep repainting while downloading
                    if update_check::UPDATE_DOWNLOADING.get().map(|d| d.load(std::sync::atomic::Ordering::Relaxed)).unwrap_or(false) {
                        ctx.request_repaint();
                    }
                });
        }

        self.handle_dropped_files(ctx);

        if let Some(rx) = &self.process_window_receiver {
            if let Ok(result) = rx.try_recv() {
                self.process_window_receiver = None;
                if let Some(processes) = result {
                    self.selected_processes = processes;
                    save_selected_processes(&self.selected_processes);
                }
            }
        }

        if let Some(rx) = &self.site_window_receiver {
            if let Ok(result) = rx.try_recv() {
                self.site_window_receiver = None;
                if let Some(text) = result {
                    self.selected_sites = text
                        .lines()
                        .map(str::trim)
                        .filter(|line| !line.is_empty())
                        .map(String::from)
                        .collect();
                    save_selected_sites(&self.selected_sites);
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            self.ensure_tray_subclass(frame);
        }

        if self.top_image.is_none() {
            self.top_image = Some(load_texture(ctx, "vpnfy", include_bytes!("../../src/gifs/vpnfy.png")));
            if let Ok((frames, durations)) = load_gif_frames(ctx, include_bytes!("../../src/gifs/animated.gif")) {
                self.animated_frames = Some(frames);
                self.animated_frame_durations = durations;
                self.animated_frame_index = 0;
                self.animated_last_frame = Instant::now();
            }
        }

        if self.settings_icon.is_none() {
            self.settings_icon = load_svg_texture(ctx, "settings_icon", include_bytes!("icons/settings.svg"));
        }
        if self.settings_close_icon.is_none() {
            self.settings_close_icon = load_svg_texture(ctx, "settings_close_icon", include_bytes!("icons/settings-close.svg"));
        }
        if self.language_icon.is_none() {
            self.language_icon = load_svg_texture(ctx, "language_icon", include_bytes!("icons/language.svg"));
        }
        if self.upload_icon.is_none() {
            self.upload_icon = load_svg_texture(ctx, "upload_icon", include_bytes!("icons/arrow-up.svg"));
        }
        if self.download_icon.is_none() {
            self.download_icon = load_svg_texture(ctx, "download_icon", include_bytes!("icons/arrow-down.svg"));
        }

        if let Some(frames) = &self.animated_frames {
            if !frames.is_empty() {
                let delay = self.animated_frame_durations[self.animated_frame_index].max(50);
                if self.animated_last_frame.elapsed() >= Duration::from_millis(delay) {
                    self.animated_frame_index = (self.animated_frame_index + 1) % frames.len();
                    self.animated_last_frame = Instant::now();
                }
                ctx.request_repaint_after(Duration::from_millis(40));
            }
        }

        let connect_effect_progress = self.connect_effect_progress();
        let button_alpha = |response: &egui::Response, base_alpha: u8| {
            if response.is_pointer_button_down_on() {
                (base_alpha as f32 * 0.50).round().clamp(0.0, 255.0) as u8
            } else if response.hovered() {
                (base_alpha as f32 * 0.80).round().clamp(0.0, 255.0) as u8
            } else {
                base_alpha
            }
        };
        let apply_button_cursor = |ctx: &egui::Context, response: &egui::Response, enabled: bool| {
            if response.is_pointer_button_down_on() {
                ctx.set_cursor_icon(egui::CursorIcon::Default);
            } else if response.hovered() {
                ctx.set_cursor_icon(if enabled {
                    egui::CursorIcon::PointingHand
                } else {
                    egui::CursorIcon::NotAllowed
                });
            }
        };
        let button_font = button_font_id();
        let mut is_animating = false;
        let opacity_delta = self.import_button_opacity_target - self.import_button_opacity;
        if opacity_delta.abs() > 0.001 {
            let factor = if opacity_delta > 0.0 { 0.15 } else { 0.50 };
            self.import_button_opacity += opacity_delta * factor;
            self.import_button_opacity = self.import_button_opacity.clamp(0.0, 1.0);
            is_animating = true;
        }
        if connect_effect_progress > 0.0 && connect_effect_progress < 1.0 {
            is_animating = true;
        }

        // Top-left settings & language button offsets (12 physical pixels from edges)
        let edge_pad = 12.0 / ctx.pixels_per_point();

        // Top-left settings button overlay without affecting layout
        egui::Area::new("settings_button".into())
            .anchor(egui::Align2::LEFT_TOP, [edge_pad, edge_pad])
            .movable(false)
            .show(ctx, |ui| {
                let button_size = egui::vec2(26.0, 26.0);
                let (button_rect, response) = ui.allocate_exact_size(button_size, egui::Sense::click());
                let icon_alpha = button_alpha(&response, 255);
                if let Some(settings_icon) = &self.settings_icon {
                    ui.painter().image(
                        settings_icon.id(),
                        button_rect.shrink(2.0),
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, icon_alpha),
                    );
                } else {
                    #[cfg(target_os = "windows")]
                    {
                        let ppp = ctx.pixels_per_point();
                        let w_px = (button_rect.width() * ppp).ceil() as usize;
                        let h_px = (button_rect.height() * ppp).ceil() as usize;
                        let key = format!("settings_icon:{}:{}:{}", "\u{2699}", w_px, h_px);
                        let text_color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, icon_alpha);
                        if let Some(tex) = self.win_text_cache.get(&key) {
                            ui.painter().image(tex.id(), button_rect, egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(1.0,1.0)), egui::Color32::WHITE);
                        } else if let Some(tex) = win_text_to_texture(ctx, &key, "\u{2699}", self.button_hfont, text_color, w_px, h_px) {
                            self.win_text_cache.insert(key.clone(), tex.clone());
                            ui.painter().image(tex.id(), button_rect, egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(1.0,1.0)), egui::Color32::WHITE);
                        } else {
                            ui.painter().text(
                                button_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "\u{2699}",
                                egui::FontId::proportional(24.0),
                                egui::Color32::from_rgba_unmultiplied(255, 255, 255, icon_alpha),
                            );
                        }
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        ui.painter().text(
                            button_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "\u{2699}",
                            egui::FontId::proportional(24.0),
                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, icon_alpha),
                        );
                    }
                }
                apply_button_cursor(ctx, &response, true);
                if response.clicked() {
                    self.show_settings = true;
                    self.settings_tab = "processes".to_string();
                    self.cached_processes = get_running_processes();
                    self.cached_processes.sort();
                    self.cached_processes.dedup();
                    self.last_process_refresh = Some(Instant::now());
                }
            });

        // Manual update button removed; update checks run on startup.

        // Top-right language button overlay
        egui::Area::new("language_button".into())
            .anchor(egui::Align2::RIGHT_TOP, [-edge_pad, edge_pad])
            .movable(false)
            .show(ctx, |ui| {
                let button_size = egui::vec2(38.0, 26.0);
                let (button_rect, response) = ui.allocate_exact_size(button_size, egui::Sense::click());
                let icon_alpha = button_alpha(&response, 255);
                if let Some(language_icon) = &self.language_icon {
                    ui.painter().image(
                        language_icon.id(),
                        button_rect.shrink2(egui::vec2(8.0, 2.0)),
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, icon_alpha),
                    );
                } else {
                    let lang_text = self.language.code();
                    let lang_color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, icon_alpha);
                    #[cfg(target_os = "windows")]
                    {
                        let ppp = ctx.pixels_per_point();
                        let w_px = (button_rect.width() * ppp).ceil() as usize;
                        let h_px = (button_rect.height() * ppp).ceil() as usize;
                        let key = format!("language_button:{}:{}:{}", lang_text, w_px, h_px);
                        if let Some(tex) = self.win_text_cache.get(&key) {
                            ui.painter().image(tex.id(), button_rect, egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(1.0,1.0)), egui::Color32::WHITE);
                        } else if let Some(tex) = win_text_to_texture(ctx, &key, lang_text, self.button_hfont, lang_color, w_px, h_px) {
                            self.win_text_cache.insert(key.clone(), tex.clone());
                            ui.painter().image(tex.id(), button_rect, egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(1.0,1.0)), egui::Color32::WHITE);
                        } else {
                            let lang_font = button_font.clone();
                            for offset in [egui::vec2(-0.35, 0.0), egui::vec2(0.35, 0.0), egui::Vec2::ZERO] {
                                ui.painter().text(
                                    button_rect.center() + offset,
                                    egui::Align2::CENTER_CENTER,
                                    lang_text,
                                    lang_font.clone(),
                                    lang_color,
                                );
                            }
                        }
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        let lang_font = button_font.clone();
                        for offset in [egui::vec2(-0.35, 0.0), egui::vec2(0.35, 0.0), egui::Vec2::ZERO] {
                            ui.painter().text(
                                button_rect.center() + offset,
                                egui::Align2::CENTER_CENTER,
                                lang_text,
                                lang_font.clone(),
                                lang_color,
                            );
                        }
                    }
                }
                apply_button_cursor(ctx, &response, true);
                if response.clicked() {
                    self.language = self.language.next();
                    save_language(self.language);
                }
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::BLACK))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(24.0);
                    let pulse_scale = self.gif_pulse_scale();
                    let connect_scale = 1.0 + 0.20 * connect_effect_progress;
                    let connect_shift = 0.14 * 264.0 * connect_effect_progress;
                    if let Some(top_image) = &self.top_image {
                        let image_base = egui::vec2(132.0, 132.0);
                        let image_size = image_base * connect_scale;
                        let gif_size = egui::vec2(264.0, 264.0) * connect_scale;
                        let (rect, _) = ui.allocate_exact_size(image_base, egui::Sense::hover());
                        let image_center = rect.center() + egui::vec2(0.0, connect_shift);
                        if let Some(frames) = &self.animated_frames {
                            if let Some(frame_texture) = frames.get(self.animated_frame_index) {
                                let gif_rect = egui::Rect::from_center_size(image_center, gif_size * pulse_scale);
                                egui::Image::new(frame_texture)
                                    .fit_to_exact_size(gif_rect.size())
                                    .rotate(self.gif_rotation_angle(), egui::Vec2::splat(0.5))
                                    .paint_at(ui, gif_rect);
                            }
                        }
                        let top_rect = egui::Rect::from_center_size(image_center, image_size);
                        ui.painter().image(
                            top_image.id(),
                            top_rect,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 255),
                        );
                        
                        ui.add_space(20.0);
                        ui.add_space(20.0);
                    } else {
                        ui.add_space(20.0);
                    }

                let controls_locked_by_settings = self.show_settings;
                ui.add_space(-4.0);
                let import_button_text = if let Some(ref conf) = self.conf_path {
                    Path::new(conf)
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or(conf.as_str())
                        .to_string()
                } else {
                    self.language.translate("Импорт")
                };
                let import_button_enabled = !(self.service_running || self.service_active);
                let import_button_interactive = import_button_enabled && !controls_locked_by_settings;
                let import_button_alpha = (self.import_button_opacity * 255.0).round().clamp(0.0, 255.0) as u8;
                let button_size = egui::vec2(220.0, 40.0);
                let (button_rect, import_button_response) = ui.allocate_exact_size(
                    button_size,
                    if import_button_interactive {
                        egui::Sense::click()
                    } else {
                        egui::Sense::hover()
                    },
                );
                let render_import_alpha = if import_button_interactive {
                    button_alpha(&import_button_response, import_button_alpha)
                } else {
                    import_button_alpha
                };
                if import_button_alpha > 0 {
                    let stroke_width = 2.0;
                    let inner_rect = button_rect.shrink(stroke_width / 2.0);
                    let import_fill_alpha = if import_button_interactive
                        && import_button_response.hovered()
                        && !import_button_response.is_pointer_button_down_on()
                    {
                        (import_button_alpha as f32 * 0.20).round().clamp(0.0, 255.0) as u8
                    } else {
                        0
                    };
                    ui.painter().rect_filled(
                        inner_rect,
                        6.0,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, import_fill_alpha),
                    );
                    ui.painter().rect_stroke(
                        inner_rect,
                        6.0,
                        egui::Stroke::new(stroke_width, egui::Color32::from_rgba_unmultiplied(255, 255, 255, render_import_alpha)),
                    );
                    #[cfg(target_os = "windows")]
                    {
                        let ppp = ctx.pixels_per_point();
                        let w_px = (button_rect.width() * ppp).ceil() as usize;
                        let h_px = (button_rect.height() * ppp).ceil() as usize;
                        let key = format!("import_button:{}:{}:{}", import_button_text, w_px, h_px);
                        // If the button has a white inner fill (hovered), use black text for contrast
                        let text_rgb = if import_fill_alpha > 128 { (0u8, 0u8, 0u8) } else { (255u8, 255u8, 255u8) };
                        let text_color = egui::Color32::from_rgba_unmultiplied(text_rgb.0, text_rgb.1, text_rgb.2, render_import_alpha);
                        if let Some(tex) = self.win_text_cache.get(&key) {
                            ui.painter().image(tex.id(), button_rect, egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(1.0,1.0)), egui::Color32::WHITE);
                        } else if let Some(tex) = win_text_to_texture(ctx, &key, &import_button_text, self.button_hfont, text_color, w_px, h_px) {
                            self.win_text_cache.insert(key.clone(), tex.clone());
                            ui.painter().image(tex.id(), button_rect, egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(1.0,1.0)), egui::Color32::WHITE);
                        } else {
                            ui.painter().text(
                                button_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                import_button_text,
                                button_font.clone(),
                                egui::Color32::from_rgba_unmultiplied(text_color.r(), text_color.g(), text_color.b(), text_color.a()),
                            );
                        }
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        ui.painter().text(
                            button_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            import_button_text,
                            button_font.clone(),
                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, render_import_alpha),
                        );
                    }
                }
                if controls_locked_by_settings && import_button_response.hovered() {
                    ctx.set_cursor_icon(egui::CursorIcon::Default);
                } else {
                    apply_button_cursor(ctx, &import_button_response, import_button_enabled);
                }
                if import_button_interactive && import_button_response.clicked() {
                    if let Some(path) = FileDialog::new().add_filter("WireGuard config", &["conf"]).pick_file() {
                        let selected_path = path.display().to_string();
                        self.conf_path = Some(selected_path.clone());
                        self.error_log = None;
                        save_conf_path(self.conf_path.as_ref().unwrap());
                    }
                }

                let gap = 8.0 / ctx.pixels_per_point();
                // Ensure the visual gap from the Connect button to the traffic text
                // equals 8 physical pixels even after the upward nudge of the text.
                let ppp = ctx.pixels_per_point();
                let gap_connect_text = (8.0) / ppp; // 8px desired + 12px nudge
                ui.add_space(gap_connect_text);

                let connect_label = if self.service_active { self.language.translate("Отключиться") } else { self.language.translate("Подключиться") };
                let is_busy = self.service_running;
                let connect_enabled = self.conf_path.is_some() && !is_busy;
                let connect_interactive = connect_enabled && !controls_locked_by_settings;
                let connect_fill_alpha = if self.conf_path.is_none() {
                    128
                } else if self.service_active && !is_busy {
                    255
                } else if is_busy {
                    128
                } else {
                    255
                };
                let connect_button_size = egui::vec2(220.0, 40.0);
                let (connect_rect, connect_response) = ui.allocate_exact_size(
                    connect_button_size,
                    if connect_interactive {
                        egui::Sense::click()
                    } else {
                        egui::Sense::hover()
                    },
                );
                let connect_hover_alpha = if connect_interactive {
                    button_alpha(&connect_response, connect_fill_alpha)
                } else {
                    connect_fill_alpha
                };
                let connect_fill = if self.service_active && !is_busy {
                    egui::Color32::from_rgba_unmultiplied(180, 80, 80, connect_hover_alpha)
                } else if is_busy {
                    egui::Color32::from_rgba_unmultiplied(220, 220, 220, connect_hover_alpha)
                } else {
                    egui::Color32::from_rgba_unmultiplied(220, 220, 220, connect_hover_alpha)
                };
                ui.painter().rect_filled(connect_rect, 6.0, connect_fill);
                ui.painter().rect_stroke(
                    connect_rect,
                    6.0,
                    egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, connect_hover_alpha)),
                );
                #[cfg(target_os = "windows")]
                {
                    let ppp = ctx.pixels_per_point();
                    let w_px = (connect_rect.width() * ppp).ceil() as usize;
                    let h_px = (connect_rect.height() * ppp).ceil() as usize;
                    let key = format!("connect_button:{}:{}:{}", connect_label, w_px, h_px);
                    let text_color = egui::Color32::from_rgba_unmultiplied(0, 0, 0, connect_hover_alpha);
                    if let Some(tex) = self.win_text_cache.get(&key) {
                        ui.painter().image(tex.id(), connect_rect, egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(1.0,1.0)), egui::Color32::WHITE);
                    } else if let Some(tex) = win_text_to_texture(ctx, &key, &connect_label, self.button_hfont, text_color, w_px, h_px) {
                        self.win_text_cache.insert(key.clone(), tex.clone());
                        ui.painter().image(tex.id(), connect_rect, egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(1.0,1.0)), egui::Color32::WHITE);
                    } else {
                        ui.painter().text(
                            connect_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            connect_label,
                            egui::FontId::proportional(UI_BUTTON_FONT_SIZE),
                            egui::Color32::from_rgba_unmultiplied(0, 0, 0, connect_hover_alpha),
                        );
                    }
                }
                #[cfg(not(target_os = "windows"))]
                {
                    ui.painter().text(
                        connect_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        connect_label,
                        egui::FontId::proportional(UI_BUTTON_FONT_SIZE),
                        egui::Color32::from_rgba_unmultiplied(0, 0, 0, connect_hover_alpha),
                    );
                }
                if controls_locked_by_settings && connect_response.hovered() {
                    ctx.set_cursor_icon(egui::CursorIcon::Default);
                } else {
                    apply_button_cursor(ctx, &connect_response, connect_enabled);
                }
                if connect_interactive && connect_response.clicked() {
                    if let Some(ref conf) = self.conf_path {
                        if self.service_active {
                            if !self.elevated {
                                self.status = self.language.translate("Нужны права администратора. Запустите приложение от имени администратора");
                                show_error_dialog(&self.language.translate("Ошибка"), &self.status);
                            } else {
                                let conf_path = conf.clone();
                                let (tx, rx) = mpsc::channel();
                                self.status_rx = Some(rx);
                                self.service_running = true;
                                self.error_log = None;
                                self.disconnect_animation_start = Some(Instant::now());

                                thread::spawn(move || {
                                    let result = stop_and_delete_service(&conf_path);
                                    let _ = tx.send(result);
                                });
                            }
                        } else if !self.elevated {
                            self.status = self.language.translate("Нужны права администратора. Запустите приложение от имени администратора");
                            show_error_dialog(&self.language.translate("Ошибка"), &self.status);
                        } else {
                            self.import_button_opacity_target = 0.0;
                            self.connect_animation_start = Some(Instant::now());
                            let conf = conf.clone();
                            let (tx, rx) = mpsc::channel();
                            self.status_rx = Some(rx);
                            self.service_running = true;
                            self.error_log = None;
                            self.session_traffic_bytes = 0;
                            self.session_base_traffic_bytes = None;
                            self.wireproxy_info_addr = None;
                            self.last_tunnel_traffic_poll = None;
                            self.startup_animation_frame = 0;

                            // Сохраняем выбранные процессы и режим перед подключением — затем ProxyBridge
                            // будет запущен автоматически после успешного старта туннеля
                            save_selected_processes(&self.selected_processes);
                            save_proxy_mode(self.proxy_mode_toggle);

                            let status_sender = tx;
                            thread::spawn(move || {
                                kill_existing_processes();
                                let _ = stop_and_delete_service(&conf);
                                let result = create_and_start_service(&conf);
                                let _ = status_sender.send(result);
                            });
                        }
                    } else {
                        self.status = self.language.translate("Сначала импортируйте .conf файл");
                        show_error_dialog("Ошибка", &self.status);
                    }
                }

                ui.add_space(gap);
                let _traffic_alpha = (self.traffic_opacity * 255.0).round().clamp(0.0, 255.0) as u8;
                // Всегда показываем текст времени/трафика (полная непрозрачность)
                let text_alpha: u8 = 255u8;
                let text_width = connect_rect.width().min(ui.available_width());
                let (text_rect, _) = ui.allocate_exact_size(egui::vec2(text_width, connect_rect.height()), egui::Sense::hover());
                
                // Единая позиция для обоих текстов - сдвинута вверх на 12px (физических)
                let ppp = ctx.pixels_per_point();
                let text_nudge = 16.0 / ppp;
                // Shift the drawing rectangle upward so image-based text moves too
                let shifted_rect = text_rect.translate(egui::vec2(0.0, -text_nudge));
                // Move the time/traffic text an additional 2 physical pixels upward (DPI-aware)
                let text_position = shifted_rect.center() + egui::vec2(0.0, -(4.0 + 2.0 / ppp));
                
                // Update the time/traffic display at most once per second
                if self.last_time_display_update.map_or(true, |t| t.elapsed() >= Duration::from_secs(1)) {
                    let mb = self.session_traffic_bytes as f64 / 1024.0 / 1024.0;
                    let traffic_text = if mb > 1000.0 {
                        format!("{:.2} GB", mb / 1024.0)
                    } else {
                        format!("{:.2} MB", mb)
                    };
                    self.cached_time_display = format!("{} / {}", self.format_connection_time(), traffic_text);

                    // Also update displayed speeds at the same 1s cadence (use last computed bps)
                    let up_mbps = self.last_upload_bps / 1024.0 / 1024.0;
                    let down_mbps = self.last_download_bps / 1024.0 / 1024.0;
                    self.cached_up_display = format!("{:.2}", up_mbps);
                    self.cached_down_display = format!("{:.2}", down_mbps);

                    self.last_time_display_update = Some(Instant::now());
                }
                let display_text = &self.cached_time_display;

                // Рисуем текст (всегда, даже если VPN неактивен)
                {
                    #[cfg(target_os = "windows")]
                    {
                        let ppp = ctx.pixels_per_point();
                        let w_px = (text_rect.width() * ppp).ceil() as usize;
                        let h_px = (text_rect.height() * ppp).ceil() as usize;
                        let key = format!("center_mode_display:{}:{}:{}", display_text, w_px, h_px);
                        let text_color = egui::Color32::from_white_alpha(text_alpha);
                        if let Some(tex) = self.win_text_cache.get(&key) {
                            ui.painter().image(tex.id(), shifted_rect, egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(1.0,1.0)), egui::Color32::WHITE);
                        } else if let Some(tex) = win_text_to_texture(ctx, &key, &display_text, self.button_hfont, text_color, w_px, h_px) {
                            // Use the same HFONT as buttons so sizes match exactly
                            self.win_text_cache.insert(key.clone(), tex.clone());
                            ui.painter().image(tex.id(), shifted_rect, egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(1.0,1.0)), egui::Color32::WHITE);
                        } else {
                            ui.painter().text(
                                text_position,
                                egui::Align2::CENTER_CENTER,
                                &display_text,
                                egui::FontId::default(),
                                egui::Color32::from_white_alpha(text_alpha),
                            );
                        }
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        ui.painter().text(
                            text_position,
                            egui::Align2::CENTER_CENTER,
                            display_text,
                            egui::FontId::default(),
                            egui::Color32::from_white_alpha(text_alpha),
                        );
                    }
                }

                // Разделённые метки скорости: upload в левом нижнем углу, download в правом нижнем углу
                // text strings replaced by icons + numeric labels
                let pad_points = 12.0 / ppp;

                let speed_alpha = if self.service_active { 255u8 } else { 0u8 };

                egui::Area::new("upload_speed_area".into())
                    .anchor(egui::Align2::LEFT_BOTTOM, [pad_points, -pad_points])
                    .movable(false)
                    .show(ctx, |ui_area| {
                        let ppp_local = ctx.pixels_per_point();
                        // Increase icon size by 4 physical px (14 -> 18) and widen area by 20 physical px
                        let icon_size_points = 18.0 / ppp_local;
                        let spacing_points = 6.0 / ppp_local;
                        let added_width_points = 20.0 / ppp_local;
                        let text_str = self.cached_up_display.clone();
                        let font_id = button_font_id();
                        let galley = ui_area.fonts(|f| f.layout_no_wrap(text_str.clone(), font_id.clone(), egui::Color32::WHITE));
                        let text_size = galley.size();
                        // Reserve extra pixel margin to avoid glyph clipping when generating WinAPI textures
                        let text_px = (text_size.x * ppp_local).ceil() as usize;
                        let pixel_margin = 8usize;
                        let w_px = (text_px + pixel_margin).max(1usize);
                        // Use the allocated rect height in pixels so rendering matches center text
                        let h_px = (( (icon_size_points + spacing_points + (w_px as f32) / ppp_local + added_width_points).max(icon_size_points) ) * ppp_local).ceil() as usize;
                        let text_points_with_margin = (w_px as f32) / ppp_local;
                        let total_width = icon_size_points + spacing_points + text_points_with_margin + added_width_points;
                        let total_height = (h_px as f32 / ppp_local).max(icon_size_points);
                        let (rect, _) = ui_area.allocate_exact_size(egui::vec2(total_width, total_height), egui::Sense::hover());
                        let painter = ui_area.painter();

                        // Place icon pinned to the left-bottom corner inside the area (Area anchor provides outer 12px pad)
                        // Snap icon coordinates to device pixels to ensure crisp rendering
                        let icon_x = (rect.min.x * ppp_local).round() / ppp_local;
                        let icon_y = ((rect.max.y - icon_size_points) * ppp_local).round() / ppp_local;
                        let icon_rect = egui::Rect::from_min_size(egui::pos2(icon_x, icon_y), egui::vec2(icon_size_points, icon_size_points));
                        if let Some(tex) = &self.upload_icon {
                            painter.image(tex.id(), icon_rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::from_white_alpha(speed_alpha));
                        }

                        // Center numeric text horizontally inside the full area (keep within bounds)
                        let mut text_left = rect.center().x - text_size.x * 0.5;
                        if text_left < rect.min.x {
                            text_left = rect.min.x;
                        }
                        if text_left > rect.max.x - text_size.x {
                            text_left = rect.max.x - text_size.x;
                        }
                        let text_y = rect.min.y + (rect.height() - text_size.y) * 0.5;
                        // Snap positions to device pixels to avoid texture scaling blur
                        let snapped_x = (text_left * ppp_local).round() / ppp_local;
                        let snapped_y = (text_y * ppp_local).round() / ppp_local;
                        // Make the texture height match the allocated rect height for consistent rendering
                        let text_rect = egui::Rect::from_min_size(
                            egui::pos2(snapped_x, snapped_y),
                            egui::vec2((w_px as f32) / ppp_local, rect.height()),
                        );
                        let text_color = egui::Color32::from_white_alpha(speed_alpha);
                        #[cfg(target_os = "windows")]
                        {
                            // Use the precomputed pixel sizes (w_px/h_px) so textures include margin
                            let key = format!("speed_up:{}:{}:{}", text_str, w_px, h_px);
                            if let Some(tex) = self.win_text_cache.get(&key) {
                                painter.image(tex.id(), text_rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), text_color);
                            } else if let Some(tex) = win_text_to_texture(ctx, &key, &text_str, self.button_hfont, text_color, w_px, h_px) {
                                self.win_text_cache.insert(key.clone(), tex.clone());
                                painter.image(tex.id(), text_rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), text_color);
                            } else {
                                painter.galley(text_rect.min, galley.clone(), text_color);
                            }
                        }
                        #[cfg(not(target_os = "windows"))]
                        {
                            painter.galley(text_rect.min, galley.clone(), text_color);
                        }
                    });

                egui::Area::new("download_speed_area".into())
                    .anchor(egui::Align2::RIGHT_BOTTOM, [-pad_points, -pad_points])
                    .movable(false)
                    .show(ctx, |ui_area| {
                        let ppp_local = ctx.pixels_per_point();
                        // Increase icon size by 4 physical px (14 -> 18) and widen area by 20 physical px
                        let icon_size_points = 18.0 / ppp_local;
                        let spacing_points = 6.0 / ppp_local;
                        let added_width_points = 20.0 / ppp_local;
                        let text_str = self.cached_down_display.clone();
                        let font_id = button_font_id();
                        let galley = ui_area.fonts(|f| f.layout_no_wrap(text_str.clone(), font_id.clone(), egui::Color32::WHITE));
                        let text_size = galley.size();
                        // Reserve extra pixel margin to avoid glyph clipping when generating WinAPI textures
                        let text_px = (text_size.x * ppp_local).ceil() as usize;
                        let pixel_margin = 8usize;
                        let w_px = (text_px + pixel_margin).max(1usize);
                        // Use the allocated rect height in pixels so rendering matches center text
                        let text_points_with_margin = (w_px as f32) / ppp_local;
                        let font_px_est = (UI_BUTTON_FONT_SIZE * current_ui_scale_factor()).round() as usize;
                        let text_height_points_est = ((font_px_est + 4usize) as f32) / ppp_local;
                        let total_width = text_points_with_margin + spacing_points + icon_size_points + added_width_points;
                        let total_height = text_height_points_est.max(icon_size_points);
                        let (rect, _) = ui_area.allocate_exact_size(egui::vec2(total_width, total_height), egui::Sense::hover());
                        let painter = ui_area.painter();
                        let h_px = (rect.height() * ppp_local).ceil() as usize;

                        // Center numeric text horizontally inside the full area (keep within bounds)
                        let mut text_left = rect.center().x - text_size.x * 0.5;
                        if text_left < rect.min.x {
                            text_left = rect.min.x;
                        }
                        if text_left > rect.max.x - text_size.x {
                            text_left = rect.max.x - text_size.x;
                        }
                        let text_y = rect.min.y + (rect.height() - text_size.y) * 0.5;
                        // Snap positions to device pixels to avoid texture scaling blur
                        let snapped_x = (text_left * ppp_local).round() / ppp_local;
                        let snapped_y = (text_y * ppp_local).round() / ppp_local;
                        // Make the texture height match the allocated rect height for consistent rendering
                        let text_rect = egui::Rect::from_min_size(
                            egui::pos2(snapped_x, snapped_y),
                            egui::vec2((w_px as f32) / ppp_local, rect.height()),
                        );
                        let text_color = egui::Color32::from_white_alpha(speed_alpha);
                        #[cfg(target_os = "windows")]
                        {
                            // Use the precomputed pixel sizes (w_px/h_px) so textures include margin
                            let key = format!("speed_down:{}:{}:{}", text_str, w_px, h_px);
                            if let Some(tex) = self.win_text_cache.get(&key) {
                                painter.image(tex.id(), text_rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), text_color);
                            } else if let Some(tex) = win_text_to_texture(ctx, &key, &text_str, self.button_hfont, text_color, w_px, h_px) {
                                self.win_text_cache.insert(key.clone(), tex.clone());
                                painter.image(tex.id(), text_rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), text_color);
                            } else {
                                painter.galley(text_rect.min, galley.clone(), text_color);
                            }
                        }
                        #[cfg(not(target_os = "windows"))]
                        {
                            painter.galley(text_rect.min, galley.clone(), text_color);
                        }

                        // Snap icon to device pixels as well and pin to bottom-right inside the area
                        let icon_x = ((rect.max.x - icon_size_points) * ppp_local).round() / ppp_local;
                        let icon_y = ((rect.max.y - icon_size_points) * ppp_local).round() / ppp_local;
                        let icon_rect = egui::Rect::from_min_size(egui::pos2(icon_x, icon_y), egui::vec2(icon_size_points, icon_size_points));
                        if let Some(tex) = &self.download_icon {
                            painter.image(tex.id(), icon_rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::from_white_alpha(speed_alpha));
                        }
                    });

                if !self.status.is_empty() {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(&self.status)
                            .color(egui::Color32::WHITE)
                            .text_style(egui::TextStyle::Body),
                    );
                }

                ui.add_space(4.0);

                if let Some(rx) = &self.status_rx {
                    if let Ok(service_result) = rx.try_recv() {
                        let was_active = self.service_active;
                        self.service_running = false;
                        self.service_active = service_result.active;
                        self.error_log = service_result.error_log;
                        self.wireproxy_info_addr = service_result.wireproxy_info_addr.clone();
                        if self.service_active {
                            if !was_active {
                                self.connected_at = Some(Instant::now());
                                self.session_traffic_bytes = 0;
                                self.session_base_traffic_bytes = None;
                                self.last_tunnel_traffic_poll = None;
                                self.animated_frame_index = 0;
                                self.animated_last_frame = Instant::now();
                                self.show_windows_notification(&self.language.translate("Подключен"));
                                // Если есть сохраненные процессы, выбран системный режим или сайты через VPN, ProxyBridge должен быть запущен
                                let selected_processes = load_selected_processes();
                                let proxy_mode = load_proxy_mode();
                                let selected_sites = self.selected_sites.clone();
                                let should_run_proxybridge = !proxy_mode || !selected_processes.is_empty() || !selected_sites.is_empty();

                                if should_run_proxybridge {
                                    let status_text = format_proxybridge_status(
                                        selected_processes.len(),
                                        selected_sites.len(),
                                        proxy_mode,
                                        false,
                                    );
                                    self.status = status_text;
                                    ui.ctx().request_repaint(); // Обновляем интерфейс немедленно
                                    
                                    match start_proxybridge(&selected_processes, &selected_sites, proxy_mode) {
                                        Ok(child_opt) => {
                                            self.proxybridge_running = true;
                                            self.proxybridge_child = child_opt;
                                            self.status = format_proxybridge_status(
                                                selected_processes.len(),
                                                selected_sites.len(),
                                                proxy_mode,
                                                true,
                                            );
                                        }
                                        Err(e) => {
                                            self.proxybridge_running = false;
                                            self.proxybridge_child = None;
                                            self.status = format!("❌ ProxyBridge ошибка: {}", e);
                                            show_error_dialog(&self.language.translate("Ошибка"), &self.status);
                                        }
                                    }
                                } else {
                                    self.status = format!("✅ {}: {}", self.language.translate("Туннель подключен"), self.language.translate("Выберите процессы для маршрутизации"));
                                }
                            }
                            self.status.clear();
                        } else {
                            self.connected_at = None;
                            self.session_traffic_bytes = 0;
                            self.session_base_traffic_bytes = None;
                            self.wireproxy_info_addr = None;
                            self.last_tunnel_traffic_poll = None;
                            self.import_button_opacity_target = 1.0;
                            self.connect_animation_start = None;

                            // Показываем сообщение только если есть лог ошибки; в обычном случае скрываем статус
                            let had_error = self.error_log.is_some();
                            if had_error {
                                self.status = service_result.message.clone();
                            } else {
                                self.status.clear();
                            }

                            // Останавливаем ProxyBridge при отключении туннеля (UI отвечает за это)
                            if self.proxybridge_running {
                                match stop_proxybridge() {
                                    Ok(_) => {
                                        // При успешной остановке не показываем лишние сообщения
                                        if !had_error {
                                            self.status.clear();
                                        }
                                    }
                                    Err(e) => {
                                        self.status = format!("{}: {}", self.language.translate("Ошибка остановки ProxyBridge"), e);
                                        show_error_dialog(&self.language.translate("Ошибка"), &self.status);
                                    }
                                }
                                self.proxybridge_running = false;
                            }
                        }
                        if !self.service_active {
                            if let Some(ref error_log) = self.error_log {
                                show_error_dialog(&self.language.translate("Ошибка"), error_log);
                            } else if was_active {
                                self.show_windows_notification(&self.language.translate("Отключен"));
                            }
                        }
                        self.status_rx = None;
                    }
                }

                if self.service_running {
                    self.startup_animation_frame = self.startup_animation_frame.wrapping_add(1);
                }

                if self.service_active {
                    self.refresh_session_traffic();
                }

                    let target_traffic_opacity = if self.service_active { 1.0 } else { 0.0 };
                    let traffic_delta = target_traffic_opacity - self.traffic_opacity;
                    if traffic_delta.abs() > 0.001 {
                        self.traffic_opacity += traffic_delta * 0.154;
                        self.traffic_opacity = self.traffic_opacity.clamp(0.0, 1.0);
                        is_animating = true;
                    }

                
// Full-screen overlay settings layer
                if self.show_settings {
                    let app_rect = ctx.available_rect();

                    // Draw semi-transparent overlay covering entire app
                    let painter = ctx.layer_painter(egui::LayerId::new(
                        egui::Order::Middle,
                        egui::Id::new("settings_overlay_bg"),
                    ));
                    painter.rect_filled(app_rect, 0.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 204)); // 80% opacity

                    let old_style = (*ctx.style()).clone();
                    let mut white_style = old_style.clone();
                    white_style.visuals.override_text_color = Some(egui::Color32::WHITE);
                    ctx.set_style(white_style);
                    
                    let content_rect = app_rect.shrink2(egui::vec2(edge_pad, edge_pad));
                    let settings_header_left = content_rect.min.x;
                    let settings_header_top = content_rect.min.y;
                    let settings_header_width = content_rect.width().max(0.0);
                    let settings_close_size = egui::vec2(36.0, 28.0);
                    
                    // Close button in its own area with highest order
                    let close_response = egui::Area::new(egui::Id::new("settings_close_button"))
                        .fixed_pos(egui::pos2(
                            settings_header_left + settings_header_width - settings_close_size.x,
                            settings_header_top,
                        ))
                        .movable(false)
                        .order(egui::Order::Debug) // Highest possible order
                        .interactable(true)
                        .show(ctx, |ui| {
                            let (button_rect, response) = ui.allocate_exact_size(settings_close_size, egui::Sense::click());
                            let close_alpha = button_alpha(&response, 255);
                            if let Some(settings_close_icon) = &self.settings_close_icon {
                                ui.painter().image(
                                    settings_close_icon.id(),
                                    button_rect.shrink2(egui::vec2(6.0, 2.0)),
                                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, close_alpha),
                                );
                            } else {
                                #[cfg(target_os = "windows")]
                                {
                                    let ppp = ctx.pixels_per_point();
                                    let w_px = (button_rect.width() * ppp).ceil() as usize;
                                    let h_px = (button_rect.height() * ppp).ceil() as usize;
                                    let key = format!("settings_close:{}:{}:{}", "❌", w_px, h_px);
                                    let text_color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, close_alpha);
                                    if let Some(tex) = self.win_text_cache.get(&key) {
                                        ui.painter().image(tex.id(), button_rect, egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(1.0,1.0)), egui::Color32::WHITE);
                                    } else if let Some(tex) = win_text_to_texture(ctx, &key, "❌", self.button_hfont, text_color, w_px, h_px) {
                                        self.win_text_cache.insert(key.clone(), tex.clone());
                                        ui.painter().image(tex.id(), button_rect, egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(1.0,1.0)), egui::Color32::WHITE);
                                    } else {
                                        ui.painter().text(
                                            button_rect.center(),
                                            egui::Align2::CENTER_CENTER,
                                            "❌",
                                            egui::FontId::proportional(24.0),
                                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, close_alpha),
                                        );
                                    }
                                }
                                #[cfg(not(target_os = "windows"))]
                                {
                                    ui.painter().text(
                                        button_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        "❌",
                                        egui::FontId::proportional(24.0),
                                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, close_alpha),
                                    );
                                }
                            }
                            response
                        }).inner;

                    apply_button_cursor(ctx, &close_response, true);
                    if close_response.clicked() {
                        self.show_settings = false;
                    }
                    egui::Area::new(egui::Id::new("settings_content_area"))
                        .fixed_pos(content_rect.min)
                        .order(egui::Order::Foreground)
                        .show(ctx, |ui| {
                            ui.set_max_width(content_rect.width());
                            ui.set_max_height(content_rect.height());

                            egui::Frame::none()
                                .inner_margin(egui::Margin {
                                    left: 0.0,
                                    right: 0.0,
                                    top: 0.0,
                                    bottom: 0.0,
                                })
                                .show(ui, |ui| {
                            ui.add_space(settings_close_size.y + 8.0);

                            let sites_window_open = self.site_window_receiver.is_some() || site_editor::is_open();
                            let sites_button_enabled = !self.service_active && !sites_window_open;
                            // Label changes depending on proxy mode: in "Вся система" show "Исключенные сайты", otherwise "Сайты через VPN".
                            let sites_label_key = if self.proxy_mode_toggle { "Сайты через VPN" } else { "Исключенные сайты" };
                            let sites_button_text = format!(
                                "{} [{}]",
                                self.language.translate(sites_label_key),
                                self.selected_sites.len()
                            );

                            let process_window_open = self.process_window_receiver.is_some() || process_editor::is_open();
                            let process_button_enabled = !self.service_active && !process_window_open;
                            // For processes: in "Выбранные приложения" mode show "Приложения через VPN", otherwise "Исключенные приложения".
                            let process_label_key = if self.proxy_mode_toggle { "Приложения через VPN" } else { "Исключенные приложения" };
                            let process_button_text = format!(
                                "{} [{}]",
                                self.language.translate(process_label_key),
                                self.selected_processes.len()
                            );
                            let mode_text = if self.proxy_mode_toggle {
                                self.language.translate("Выбранные приложения")
                            } else {
                                self.language.translate("Вся система")
                            };
                            let mode_description_text = if self.proxy_mode_toggle {
                                self.language.translate("В режиме \"Выбранные приложения\" сайты из списка \"Сайты через VPN\" и приложения из списка \"Приложения через VPN\" будут идти через VPN туннель")
                            } else {
                                self.language.translate("В режиме \"Вся система\" сайты из списка \"Исключенные сайты\" и приложения из списка \"Исключенные приложения\" будут исключены из VPN туннеля")
                            };
                            let mode_enabled = !self.service_active;
                            let (settings_rect, _) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), ui.available_height()),
                                egui::Sense::hover(),
                            );
                            let button_width = settings_rect.width();
                            let button_height = 28.0;
                            let button_spacing = 8.0;
                            // Use 8 physical pixels as bottom padding, converted to points
                            let bottom_padding = 8.0 / ctx.pixels_per_point();

                            // Compute an additional downward shift of 8 physical pixels (converted to points)
                            let shift_points = 8.0 / ctx.pixels_per_point();
                            let mut mode_rect = egui::Rect::from_min_size(
                                egui::pos2(
                                    settings_rect.left(),
                                    settings_rect.bottom() - bottom_padding - button_height,
                                ),
                                egui::vec2(button_width, button_height),
                            );
                            // Shift the buttons group down by the requested amount; do not touch the close button
                            mode_rect = mode_rect.translate(egui::vec2(0.0, shift_points));
                            let process_rect = mode_rect.translate(egui::vec2(0.0, -(button_height + button_spacing)));
                            let sites_rect = process_rect.translate(egui::vec2(0.0, -(button_height + button_spacing)));

                            // Make the information text area 40px wider for better readability
                            let description_width = ((settings_rect.width() * 0.7) + 40.0).max(160.0).min(settings_rect.width());
                            let description_color = egui::Color32::WHITE;
                            let mut description_lines = Vec::new();
                            let mut current_line = String::new();
                            for word in mode_description_text.split_whitespace() {
                                let candidate_line = if current_line.is_empty() {
                                    word.to_string()
                                } else {
                                    format!("{} {}", current_line, word)
                                };
                                let candidate_galley = ui.fonts(|fonts| {
                                    fonts.layout_no_wrap(candidate_line.clone(), button_font.clone(), description_color)
                                });
                                if !current_line.is_empty() && candidate_galley.size().x > description_width {
                                    description_lines.push(ui.fonts(|fonts| {
                                        fonts.layout_no_wrap(current_line.clone(), button_font.clone(), description_color)
                                    }));
                                    current_line = word.to_string();
                                } else {
                                    current_line = candidate_line;
                                }
                            }
                            if !current_line.is_empty() {
                                description_lines.push(ui.fonts(|fonts| {
                                    fonts.layout_no_wrap(current_line.clone(), button_font.clone(), description_color)
                                }));
                            }
                            let description_top = settings_rect.top();
                            let description_bottom = (sites_rect.top() - button_spacing).max(description_top);
                            let description_center_y = description_top + ((description_bottom - description_top) * 0.5);
                            let line_spacing = 2.0;
                            let total_description_height = description_lines.iter().map(|galley| galley.size().y).sum::<f32>()
                                + line_spacing * description_lines.len().saturating_sub(1) as f32;
                            let mut description_y = description_center_y - total_description_height * 0.5;

                            let mode_response = ui.interact(
                                mode_rect,
                                ui.id().with("settings_mode_button"),
                                if mode_enabled {
                                    egui::Sense::click()
                                } else {
                                    egui::Sense::hover()
                                },
                            );
                            let button_alpha_val = if mode_enabled {
                                button_alpha(&mode_response, 255)
                            } else {
                                128
                            };
                            let button_fill = if self.proxy_mode_toggle {
                                egui::Color32::from_rgba_unmultiplied(255, 255, 255, button_alpha_val)
                            } else {
                                egui::Color32::from_rgba_unmultiplied(180, 80, 80, button_alpha_val)
                            };
                            let text_color = if self.proxy_mode_toggle {
                                egui::Color32::from_rgba_unmultiplied(0, 0, 0, button_alpha_val)
                            } else {
                                egui::Color32::from_rgba_unmultiplied(255, 255, 255, button_alpha_val)
                            };
                            ui.painter().rect_filled(mode_rect, 6.0, button_fill);
                            // Prefer WinAPI/HFONT-rendered text on Windows (cached texture)
                            #[cfg(target_os = "windows")]
                            {
                                let ppp = ctx.pixels_per_point();
                                let w_px = (mode_rect.width() * ppp).ceil() as usize;
                                let h_px = (mode_rect.height() * ppp).ceil() as usize;
                                // Always use the regular button HFONT for the mode button to avoid bold appearance
                                let key = format!("settings_mode:{}:{}:{}", mode_text, w_px, h_px);
                                let chosen_font = self.button_hfont;

                                if let Some(tex) = self.win_text_cache.get(&key) {
                                    ui.painter().image(tex.id(), mode_rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0,1.0)), egui::Color32::WHITE);
                                } else if let Some(tex) = win_text_to_texture(ctx, &key, &mode_text, chosen_font, text_color, w_px, h_px) {
                                    self.win_text_cache.insert(key.clone(), tex.clone());
                                    ui.painter().image(tex.id(), mode_rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0,1.0)), egui::Color32::WHITE);
                                } else {
                                    ui.painter().text(
                                        mode_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        mode_text,
                                        button_font.clone(),
                                        text_color,
                                    );
                                }
                            }
                            #[cfg(not(target_os = "windows"))]
                            {
                                ui.painter().text(
                                    mode_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    mode_text,
                                    button_font.clone(),
                                    text_color,
                                );
                            }
                            apply_button_cursor(ctx, &mode_response, mode_enabled);
                            if mode_response.clicked() && mode_enabled {
                                self.proxy_mode_toggle = !self.proxy_mode_toggle;
                            }

                            let process_response = ui.interact(
                                process_rect,
                                ui.id().with("settings_process_button"),
                                if process_button_enabled {
                                    egui::Sense::click()
                                } else {
                                    egui::Sense::hover()
                                },
                            );
                            let process_alpha_val = if process_button_enabled {
                                button_alpha(&process_response, 255)
                            } else {
                                128
                            };
                            ui.painter().rect_filled(
                                process_rect,
                                6.0,
                                egui::Color32::from_rgba_unmultiplied(255, 255, 255, process_alpha_val),
                            );
                            #[cfg(target_os = "windows")]
                            {
                                let ppp = ctx.pixels_per_point();
                                let w_px = (process_rect.width() * ppp).ceil() as usize;
                                let h_px = (process_rect.height() * ppp).ceil() as usize;
                                let key = format!("settings_process:{}:{}:{}", process_button_text, w_px, h_px);
                                let text_color = egui::Color32::from_rgba_unmultiplied(0, 0, 0, process_alpha_val);
                                if let Some(tex) = self.win_text_cache.get(&key) {
                                    ui.painter().image(tex.id(), process_rect, egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(1.0,1.0)), egui::Color32::WHITE);
                                } else if let Some(tex) = win_text_to_texture(ctx, &key, &process_button_text, self.button_hfont, text_color, w_px, h_px) {
                                    self.win_text_cache.insert(key.clone(), tex.clone());
                                    ui.painter().image(tex.id(), process_rect, egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(1.0,1.0)), egui::Color32::WHITE);
                                } else {
                                    ui.painter().text(
                                        process_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        process_button_text,
                                        button_font.clone(),
                                        egui::Color32::from_rgba_unmultiplied(0, 0, 0, process_alpha_val),
                                    );
                                }
                            }
                            #[cfg(not(target_os = "windows"))]
                            {
                                ui.painter().text(
                                    process_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    process_button_text,
                                    button_font.clone(),
                                    egui::Color32::from_rgba_unmultiplied(0, 0, 0, process_alpha_val),
                                );
                            }
                            apply_button_cursor(ctx, &process_response, process_button_enabled);
                            if process_response.clicked() && process_button_enabled {
                                self.cached_processes = get_running_processes();
                                self.cached_processes.sort();
                                self.cached_processes.dedup();
                                self.last_process_refresh = Some(Instant::now());

                                if process_editor::show_existing() {
                                    // Existing window is now visible and focused.
                                } else if self.process_window_receiver.is_none() {
                                    // Title depends on current proxy mode: in "Вся система" mode
                                    // show "Исключенные приложения", otherwise show "Приложения через VPN".
                                    let process_window_title = if self.proxy_mode_toggle {
                                        self.language.translate("Приложения через VPN")
                                    } else {
                                        self.language.translate("Исключенные приложения")
                                    };
                                    self.process_window_receiver = Some(process_editor::open_external(
                                        self.cached_processes.clone(),
                                        self.selected_processes.clone(),
                                        process_window_title,
                                        self.language.translate("Сохранить"),
                                    ));
                                }
                            }

                            let sites_response = ui.interact(
                                sites_rect,
                                ui.id().with("settings_sites_button"),
                                if sites_button_enabled {
                                    egui::Sense::click()
                                } else {
                                    egui::Sense::hover()
                                },
                            );
                            let sites_alpha_val = if sites_button_enabled {
                                button_alpha(&sites_response, 255)
                            } else {
                                128
                            };
                            ui.painter().rect_filled(
                                sites_rect,
                                6.0,
                                egui::Color32::from_rgba_unmultiplied(255, 255, 255, sites_alpha_val),
                            );
                            #[cfg(target_os = "windows")]
                            {
                                let ppp = ctx.pixels_per_point();
                                let w_px = (sites_rect.width() * ppp).ceil() as usize;
                                let h_px = (sites_rect.height() * ppp).ceil() as usize;
                                let key = format!("settings_sites:{}:{}:{}", sites_button_text, w_px, h_px);
                                let text_color = egui::Color32::from_rgba_unmultiplied(0, 0, 0, sites_alpha_val);
                                if let Some(tex) = self.win_text_cache.get(&key) {
                                    ui.painter().image(tex.id(), sites_rect, egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(1.0,1.0)), egui::Color32::WHITE);
                                } else if let Some(tex) = win_text_to_texture(ctx, &key, &sites_button_text, self.button_hfont, text_color, w_px, h_px) {
                                    self.win_text_cache.insert(key.clone(), tex.clone());
                                    ui.painter().image(tex.id(), sites_rect, egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(1.0,1.0)), egui::Color32::WHITE);
                                } else {
                                    ui.painter().text(
                                        sites_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        sites_button_text,
                                        button_font.clone(),
                                        egui::Color32::from_rgba_unmultiplied(0, 0, 0, sites_alpha_val),
                                    );
                                }
                            }
                            #[cfg(not(target_os = "windows"))]
                            {
                                ui.painter().text(
                                    sites_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    sites_button_text,
                                    button_font.clone(),
                                    egui::Color32::from_rgba_unmultiplied(0, 0, 0, sites_alpha_val),
                                );
                            }
                            apply_button_cursor(ctx, &sites_response, sites_button_enabled);
                            if sites_response.clicked() && sites_button_enabled {
                                // Title depends on current proxy mode: in "Вся система" mode
                                // show "Исключенные сайты", otherwise "Сайты через VPN".
                                let sites_window_title = if self.proxy_mode_toggle {
                                    self.language.translate("Сайты через VPN")
                                } else {
                                    self.language.translate("Исключенные сайты")
                                };
                                if site_editor::show_existing() {
                                    // Existing window is now visible and focused.
                                } else if self.site_window_receiver.is_none() {
                                    self.site_window_receiver = Some(site_editor::open_external(
                                        self.selected_sites.join("\r\n"),
                                        sites_window_title,
                                        self.language.translate("Сохранить"),
                                    ));
                                }
                            }

                            for description_line in description_lines {
                                let line_height = description_line.size().y;
                                let description_pos = egui::pos2(
                                    settings_rect.center().x - description_line.size().x * 0.5,
                                    description_y,
                                );
                                ui.painter().galley(
                                    description_pos,
                                    description_line,
                                    description_color,
                                );
                                description_y += line_height + line_spacing;
                            }
                        });

                    });

                    ctx.set_style(old_style);
                }
                
                ui.add_space(0.0);

                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.add_space(10.0);
                    {
                        let version_text = if update_check::UPDATE_CHECK_RUNNING.get().map(|b| b.load(std::sync::atomic::Ordering::Relaxed)).unwrap_or(false) {
                            self.language.translate("Проверка обновлений")
                        } else {
                            env!("CARGO_PKG_VERSION").to_string()
                        };
                        ui.label(
                            egui::RichText::new(version_text)
                                .color(egui::Color32::from_white_alpha(64))
                                .text_style(egui::TextStyle::Button),
                        );
                    }
                    ui.add_space(10.0);
                    let link_enabled = !controls_locked_by_settings;
                    let link_text = "t.me/vpnfybot";
                    let link_color = egui::Color32::from_rgb(0, 170, 255);
                    let galley = ui.fonts(|fonts| fonts.layout_no_wrap(link_text.to_string(), button_font.clone(), link_color));
                    // Increase width by 20 physical pixels (convert to points using DPI)
                    let ppp = ctx.pixels_per_point();
                    let extra_px = 20.0f32;
                    let extra_points = extra_px / ppp;
                    // Move visual content down by 8 physical pixels (DPI-aware)
                    let extra_y_px = 8.0f32;
                    let extra_y_points = extra_y_px / ppp;
                    let widget_size = egui::vec2(galley.size().x + extra_points, galley.size().y);
                    let (link_rect, response) = ui.allocate_exact_size(
                        widget_size,
                        if link_enabled { egui::Sense::click() } else { egui::Sense::hover() },
                    );

                    #[cfg(target_os = "windows")]
                    {
                        let ppp = ctx.pixels_per_point();
                        let w_px = (link_rect.width() * ppp).ceil() as usize;
                        let h_px = (link_rect.height() * ppp).ceil() as usize;
                        let key = format!("link:{}:{}:{}", link_text, w_px, h_px);
                        if let Some(tex) = self.win_text_cache.get(&key) {
                            ui.painter().image(tex.id(), link_rect.translate(egui::vec2(0.0, extra_y_points)), egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(1.0,1.0)), egui::Color32::WHITE);
                        } else if let Some(tex) = win_text_to_texture(ctx, &key, link_text, self.button_hfont, link_color, w_px, h_px) {
                            self.win_text_cache.insert(key.clone(), tex.clone());
                            ui.painter().image(tex.id(), link_rect.translate(egui::vec2(0.0, extra_y_points)), egui::Rect::from_min_max(egui::pos2(0.0,0.0), egui::pos2(1.0,1.0)), egui::Color32::WHITE);
                        } else {
                            ui.painter().galley(link_rect.min + egui::vec2(0.0, extra_y_points), galley.clone(), link_color);
                        }
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        ui.painter().galley(link_rect.min + egui::vec2(0.0, extra_y_points), galley.clone(), link_color);
                    }

                    if link_enabled && response.hovered() {
                        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    if response.is_pointer_button_down_on() {
                        ctx.set_cursor_icon(egui::CursorIcon::Default);
                    }

                    if link_enabled && response.clicked() {
                        open_url("https://t.me/vpnfybot");
                    }
                });
            });
        });

        if is_animating {
            ctx.request_repaint_after(Duration::from_millis(20));
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(ref conf) = self.conf_path {
            let _ = stop_and_delete_service(conf);
        }
        
        // Останавливаем ProxyBridge при выходе из приложения
        if self.proxybridge_running {
            // Сначала пробуем убить дочерний процесс напрямую, если он существует
            if let Some(mut child) = self.proxybridge_child.take() {
                if let Err(e) = child.kill() {
                    log::warn!("Не удалось убить дочерний процесс ProxyBridge напрямую: {}", e);
                }
                let _ = child.wait(); // Ожидаем завершения процесса
            }
            
            // Затем используем стандартный метод остановки через PowerShell/taskkill
            match stop_proxybridge() {
                Ok(_) => {
                    log::info!("ProxyBridge успешно остановлен при выходе из приложения");
                }
                Err(e) => {
                    log::error!("Ошибка остановки ProxyBridge при выходе: {}", e);
                }
            }
            self.proxybridge_running = false;
        }
        
        self.remove_tray_icon();
        if let Some(font) = self.button_hfont.take() {
            unsafe {
                let _ = DeleteObject(font);
            }
        }
        if let Some(font) = self.button_hfont_light.take() {
            unsafe {
                let _ = DeleteObject(font);
            }
        }
    }
}

impl AppState {
    #[allow(dead_code)]
    fn get_tunnel_total_bytes(&self) -> Option<u64> {
        let info_addr = self.wireproxy_info_addr.as_deref()?;
        let metrics = fetch_wireproxy_metrics(info_addr)?;
        // Keep compatibility: return sum(tx+rx)
        parse_wireproxy_metrics_rx_tx(&metrics).map(|(tx, rx)| tx.saturating_add(rx))
    }

    fn get_tunnel_rx_tx_totals(&self) -> Option<(u64, u64)> {
        let info_addr = self.wireproxy_info_addr.as_deref()?;
        let metrics = fetch_wireproxy_metrics(info_addr)?;
        parse_wireproxy_metrics_rx_tx(&metrics)
    }

    fn refresh_session_traffic(&mut self) {
        if self
            .last_tunnel_traffic_poll
            .is_some_and(|last| last.elapsed() < TUNNEL_TRAFFIC_POLL_INTERVAL)
        {
            return;
        }

        // Determine time and previous sample
        let now = Instant::now();
        let prev_instant = self.last_tunnel_traffic_poll;

        // Fetch per-direction totals (tx = upload, rx = download)
        let Some((current_tx, current_rx)) = self.get_tunnel_rx_tx_totals() else {
            return;
        };

        let current_total = current_tx.saturating_add(current_rx);

        // Initialize base total if needed
        let base = self.session_base_traffic_bytes.get_or_insert(current_total);
        self.session_traffic_bytes = current_total.saturating_sub(*base);

        // Compute speeds using previous sample when available
        if let Some((prev_tx, prev_rx)) = self.last_tunnel_totals {
            // Use previous poll time; if missing, assume one polling interval
            let elapsed = prev_instant.map(|p| now.duration_since(p)).unwrap_or(TUNNEL_TRAFFIC_POLL_INTERVAL);
            let secs = elapsed.as_secs_f64().max(0.000_001);
            let delta_tx = current_tx.saturating_sub(prev_tx) as f64;
            let delta_rx = current_rx.saturating_sub(prev_rx) as f64;
            self.last_upload_bps = delta_tx / secs;
            self.last_download_bps = delta_rx / secs;
        } else {
            self.last_upload_bps = 0.0;
            self.last_download_bps = 0.0;
        }

        // Store current sample and timestamp
        self.last_tunnel_totals = Some((current_tx, current_rx));
        self.last_tunnel_traffic_poll = Some(now);
    }

    fn reset_tunnel_traffic_state(&mut self) {
        self.session_traffic_bytes = 0;
        self.session_base_traffic_bytes = None;
        self.wireproxy_info_addr = None;
        self.last_tunnel_traffic_poll = None;
    }

    fn format_connection_time(&self) -> String {
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

    fn gif_pulse_scale(&mut self) -> f32 {
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

    fn connect_effect_progress(&mut self) -> f32 {
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

    fn gif_rotation_angle(&self) -> f32 {
        let elapsed = self.gif_rotation_start.elapsed().as_secs_f32();
        let period = 90.0;
        let t = (elapsed % period) / period;
        t * std::f32::consts::TAU
    }

    #[cfg(target_os = "windows")]
    #[allow(deprecated)]
    fn ensure_tray_subclass(&mut self, frame: &mut Frame) {
        if self.tray_subclassed {
            return;
        }

        if let Ok(window_handle) = frame.window_handle() {
            if let Ok(RawWindowHandle::Win32(handle)) = window_handle.raw_window_handle() {
                let hwnd = HWND(handle.hwnd.get());
                let needs_reset = self.tray_window != Some(hwnd);
                if needs_reset {
                    if self.tray_icon_added {
                        self.remove_tray_icon();
                    }
                    self.tray_window = Some(hwnd);
                    self.tray_subclassed = false;
                }

                if !self.tray_subclassed {
                    self.add_tray_icon(hwnd);
                    unsafe {
                        let prev = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, subclass_wndproc as *const () as isize);
                        ORIGINAL_WNDPROC = std::mem::transmute(prev);
                        let _ = DragAcceptFiles(hwnd, BOOL(1));
                    }
                    self.tray_subclassed = true;
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    fn load_tray_icon(&self) -> Option<HICON> {
        let icon_data = from_png_bytes(include_bytes!("../../src/gifs/vpnfy.png")).ok()?;
        let width = icon_data.width as i32;
        let height = icon_data.height as i32;
        let mut rgba = icon_data.rgba;
        for chunk in rgba.chunks_exact_mut(4) {
            chunk.swap(0, 2);
        }

        unsafe {
            let hbmp_color = CreateBitmap(width, height, 1, 32, Some(rgba.as_ptr() as *const _));
            if hbmp_color.is_invalid() {
                return None;
            }
            let hbmp_mask = CreateBitmap(width, height, 1, 1, Some(std::ptr::null()));
            if hbmp_mask.is_invalid() {
                let _ = DeleteObject(hbmp_color);
                return None;
            }

            let mut icon_info = ICONINFO::default();
            icon_info.fIcon = BOOL(1);
            icon_info.hbmColor = hbmp_color;
            icon_info.hbmMask = hbmp_mask;

            let hicon = CreateIconIndirect(&icon_info).ok()?;
            if hicon.is_invalid() {
                let _ = DeleteObject(hbmp_color);
                let _ = DeleteObject(hbmp_mask);
                return None;
            }

            Some(hicon)
        }
    }

    #[cfg(target_os = "windows")]
    fn add_tray_icon(&mut self, hwnd: HWND) {
        if self.tray_icon_added {
            return;
        }

        if self.tray_icon.is_none() {
            self.tray_icon = self.load_tray_icon();
        }

        let mut nid = NOTIFYICONDATAW::default();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = TRAY_ICON_ID;
        nid.uFlags = NIF_MESSAGE | NIF_TIP;
        nid.uCallbackMessage = TRAY_CALLBACK_MESSAGE;
        if let Some(icon) = self.tray_icon {
            nid.uFlags |= NIF_ICON;
            nid.hIcon = icon;
        }
        let tip: Vec<u16> = OsStr::new(APP_TITLE).encode_wide().chain(Some(0)).collect();
        for (i, &c) in tip.iter().enumerate() {
            if i < nid.szTip.len() {
                nid.szTip[i] = c;
            }
        }

        unsafe {
            let _ = Shell_NotifyIconW(NIM_ADD, &nid);
        }
        self.tray_icon_added = true;
    }

    #[cfg(target_os = "windows")]
    fn remove_tray_icon(&mut self) {
        if !self.tray_icon_added {
            return;
        }
        if let Some(hwnd) = self.tray_window {
            let mut nid = NOTIFYICONDATAW::default();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = hwnd;
            nid.uID = TRAY_ICON_ID;
            unsafe {
                let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
            }
        }
        if let Some(icon) = self.tray_icon {
            unsafe {
                let _ = DestroyIcon(icon);
            }
            self.tray_icon = None;
        }
        self.tray_icon_added = false;
    }
}

const TRAY_CALLBACK_MESSAGE: u32 = WM_APP + 1;
static mut ORIGINAL_WNDPROC: WNDPROC = None;
const TRAY_ICON_ID: u32 = 1;
static DROP_FILE_PATH: OnceLock<Mutex<Option<String>>> = OnceLock::new();
static MINIMIZE_VIA_MINBUTTON: AtomicBool = AtomicBool::new(false);

fn open_url(url: &str) {
    let url_w: Vec<u16> = OsStr::new(url).encode_wide().chain(Some(0)).collect();
    unsafe {
        let result = ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(url_w.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );

        if (result.0 as isize) <= 32 {
            show_error_dialog("Ошибка", "Не удалось открыть ссылку");
        }
    }
}

fn show_error_dialog(title: &str, message: &str) {
    error_dialog::show(title.to_owned(), message.to_owned());
}

unsafe extern "system" fn subclass_wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_NCLBUTTONDOWN => {
            // User clicked non-client area; check if it was the minimize button
            if (wparam.0 as u32) == HTMINBUTTON {
                MINIMIZE_VIA_MINBUTTON.store(true, Ordering::SeqCst);
            }
        }
        WM_SIZE => {
            if wparam.0 as u32 == SIZE_MINIMIZED {
                // Only hide the window (minimize to tray) when the minimize button
                // was explicitly clicked. If minimized via taskbar click or other
                // means, leave it minimized in the taskbar.
                let via_min_button = MINIMIZE_VIA_MINBUTTON.swap(false, Ordering::SeqCst);
                if via_min_button {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }
            }
        }
        WM_DROPFILES => {
            unsafe {
                let hdrop = HDROP(wparam.0 as isize);
                let count = DragQueryFileW(hdrop, u32::MAX, None);
                for i in 0..count {
                    let mut buffer: Vec<u16> = vec![0; 260];
                    let length = DragQueryFileW(hdrop, i, Some(&mut buffer[..])) as usize;
                    if length == 0 {
                        continue;
                    }
                    buffer.truncate(length);
                    if let Ok(path) = OsString::from_wide(&buffer).into_string() {
                        if Path::new(&path).extension().and_then(|e| e.to_str()).map_or(false, |ext| ext.eq_ignore_ascii_case("conf")) {
                            let drop_storage = DROP_FILE_PATH.get_or_init(|| Mutex::new(None));
                            let mut guard = drop_storage.lock().unwrap();
                            *guard = Some(path);
                        }
                    }
                }
                DragFinish(hdrop);
            }
        }
        TRAY_CALLBACK_MESSAGE => {
            match lparam.0 as u32 {
                WM_LBUTTONUP | WM_RBUTTONUP => {
                    let _ = ShowWindow(hwnd, SW_RESTORE);
                    let _ = SetForegroundWindow(hwnd);
                }
                _ => {}
            }
        }
        _ => {}
    }

    unsafe {
        CallWindowProcW(ORIGINAL_WNDPROC, hwnd, msg, wparam, lparam)
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

impl AppState {
    fn reset_app_settings(&mut self) {
        self.conf_path = None;
        self.selected_processes.clear();
        self.proxy_mode_toggle = false; // По умолчанию "Вся система" как при первом запуске
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
        self.language = Language::En; // Сброс языка на EN
        delete_app_storage_dirs();
        save_language(self.language);
    }

    #[cfg(target_os = "windows")]
    fn show_tray_balloon_notification(&mut self, message: &str) -> bool {
        if self.tray_icon.is_none() {
            self.tray_icon = self.load_tray_icon();
        }

        if !self.tray_icon_added {
            if let Some(hwnd) = self.tray_window {
                self.add_tray_icon(hwnd);
            }
        }

        let Some(hwnd) = self.tray_window else {
            return false;
        };
        if !self.tray_icon_added {
            return false;
        }

        let mut nid = NOTIFYICONDATAW::default();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = TRAY_ICON_ID;
        nid.uFlags = NIF_INFO;
        if let Some(icon) = self.tray_icon {
            nid.uFlags |= NIF_ICON;
            nid.hIcon = icon;
        }
        nid.dwInfoFlags = NIIF_INFO;
        copy_wide_truncated(&mut nid.szInfoTitle, NOTIFICATION_APP_ID);
        copy_wide_truncated(&mut nid.szInfo, message);

        unsafe { Shell_NotifyIconW(NIM_MODIFY, &nid).as_bool() }
    }

    fn show_windows_notification(&mut self, message: &str) {
        #[cfg(target_os = "windows")]
        if self.show_tray_balloon_notification(message) {
            return;
        }

        let result: windows::core::Result<()> = (|| -> windows::core::Result<()> {
            let toast_xml = XmlDocument::new()?;
            let image_xml = notification_icon_uri()
                .map(|uri| {
                    format!(
                        "<image placement=\"appLogoOverride\" hint-crop=\"none\" src=\"{}\"/>",
                        xml_escape(&uri),
                    )
                })
                .unwrap_or_default();
            let xml = format!(
                "<toast duration=\"short\"><visual><binding template=\"ToastGeneric\">{}<text>{}</text><text>{}</text></binding></visual></toast>",
                image_xml,
                xml_escape(NOTIFICATION_APP_ID),
                xml_escape(message),
            );
            let xml_hstring = HSTRING::from(xml);
            toast_xml.LoadXml(&xml_hstring)?;
            let toast = ToastNotification::CreateToastNotification(&toast_xml)?;
            let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(NOTIFICATION_APP_ID))
                .or_else(|_| ToastNotificationManager::CreateToastNotifier())
                .or_else(|_| ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(Toast::POWERSHELL_APP_ID)))?;
            if let Some(existing) = self.last_notification.take() {
                let _ = notifier.Hide(&existing);
            }
            notifier.Show(&toast)?;
            self.last_notification = Some(toast);
            Ok(())
        })();

        if let Err(e) = result {
            eprintln!("⚠ Не удалось показать Windows-уведомление: {}", e);
        }
    }

    #[cfg(target_os = "windows")]
    fn apply_black_window_frame(&self, _frame: &Frame) {
        unsafe {
            let title_wide: Vec<u16> = OsStr::new(WINDOW_TITLE).encode_wide().chain(Some(0)).collect();
            let hwnd = FindWindowW(None, PCWSTR(title_wide.as_ptr()));
            if hwnd.0 != 0 {
                let color: u32 = 0x000000;
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_CAPTION_COLOR,
                    &color as *const _ as *const _,
                    std::mem::size_of::<u32>() as u32,
                );
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_BORDER_COLOR,
                    &color as *const _ as *const _,
                    std::mem::size_of::<u32>() as u32,
                );
            }
        }
    }

    fn handle_dropped_files(&mut self, _ctx: &egui::Context) {
        let maybe_path = DROP_FILE_PATH.get_or_init(|| Mutex::new(None)).lock().unwrap().take();
        let path = match maybe_path {
            Some(path) => path,
            None => return,
        };

        if self.service_running || self.service_active {
            self.status = self.language.translate("Отключите туннель перед импортом конфигурации");
            show_error_dialog(&self.language.translate("Ошибка"), &self.status);
            return;
        }

        self.conf_path = Some(path.clone());
        self.error_log = None;
        save_conf_path(self.conf_path.as_ref().unwrap());
        self.status = format!("Импортирован {}", Path::new(&path).file_name().and_then(|s| s.to_str()).unwrap_or("файл"));
    }
}


fn check_single_instance() -> bool {
    // Ищем окно с таким же заголовком
    let title_wide: Vec<u16> = OsStr::new(WINDOW_TITLE).encode_wide().chain(Some(0)).collect();
    unsafe {
        let existing_window = FindWindowW(
            None, // Класс окна
            PCWSTR(title_wide.as_ptr())
        );
        
        if existing_window.0 != 0 {
            // Окно существует, активируем его и выходим
            SetForegroundWindow(existing_window);
            ShowWindow(existing_window, SW_RESTORE);
            return false;
        }
    }
    true
}

fn setup_firewall_rules() {
    // Эта функция создает правила в брандмауэре Windows для разрешения работы
    // wireproxy.exe и ProxyBridge_CLI.exe в частных и общедоступных сетях
    
    // Запускаем в отдельном потоке, чтобы не блокировать UI
    thread::spawn(|| {
        // Пытаемся получить пути к зависимостям
        if let Ok(deps) = embedded_deps_bytes::ExtractedDeps::get() {
            let wireproxy_path = deps.wireproxy.to_string_lossy().to_string();
            let proxybridge_path = deps.proxybridge_cli.to_string_lossy().to_string();
            
            // Создаем PowerShell скрипт с правильными путями
            let script = format!(r#"
# Функция для добавления или обновления правила брандмауэра
function Set-FirewallRule {{
    param(
        [string]$RuleName,
        [string]$ProgramPath
    )
    
    # Проверяем, существует ли файл
    if (-not (Test-Path "$ProgramPath")) {{
        Write-Host "Файл не найден: $ProgramPath" -ForegroundColor Red
        return $false
    }}
    
    try {{
        # Сначала пытаемся удалить существующее правило
        netsh advfirewall firewall delete rule name="$RuleName" 2>$null | Out-Null
        
        # Добавляем правило для входящего трафика (обе сети)
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

# Добавляем правила для обоих приложений
Set-FirewallRule -RuleName "VPNFy - wireproxy (incoming)" -ProgramPath "{wireproxy_path}"
Set-FirewallRule -RuleName "VPNFy - ProxyBridge (incoming)" -ProgramPath "{proxybridge_path}"

Write-Host "Готово: правила брандмауэра установлены" -ForegroundColor Cyan
"#);
            
            // Запускаем PowerShell со скриптом
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
            
            // Ждем завершения скрипта
            match cmd.spawn() {
                Ok(mut child) => {
                    match child.wait() {
                        Ok(status) => {
                            if status.success() {
                                eprintln!("✓ Правила брандмауэра успешно установлены");
                            } else {
                                eprintln!("⚠ Ошибка при установке правил брандмауэра (код {})", status.code().unwrap_or(-1));
                            }
                        }
                        Err(e) => {
                            eprintln!("⚠ Ошибка ожидания процесса: {}", e);
                        }
                    }
                }
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
            eprintln!("⚠ Не удалось назначить AppUserModelID для уведомлений: {}", error);
        }
    }
}



pub(crate) fn app_main() -> eframe::Result<()> {
    // Проверяем, не запущен ли уже экземпляр приложения
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

    // Инициализируем структуру директорий приложения только для реального рабочего экземпляра
    match app_dirs::AppDirs::init() {
        Ok(app_dirs) => {
            if let Err(e) = app_dirs.reset_runtime_state() {
                eprintln!("⚠ Ошибка очистки runtime-временных файлов: {}", e);
            }

            eprintln!("✓ Инициализирована структура приложения в: {}", app_dirs.root.display());
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
    
    // Настраиваем правила брандмауэра при запуске
    setup_firewall_rules();

    // Проверяем, не остался ли запущенным ProxyBridge от предыдущего сеанса
    // Удаляем маркер запуска, если он существует
    let pid_file = managed_cache_dir().join("proxybridge.pid");
    if pid_file.exists() {
        // Пытаемся остановить оставшийся процесс
        let _ = stop_proxybridge();
        // Удаляем маркер
        let _ = std::fs::remove_file(&pid_file);
    }

    let mut options = eframe::NativeOptions::default();
    options.viewport = egui::ViewportBuilder::default()
        .with_title(WINDOW_TITLE)
        .with_inner_size([MAIN_WINDOW_CLIENT_WIDTH as f32, MAIN_WINDOW_CLIENT_HEIGHT as f32])
        .with_min_inner_size([MAIN_WINDOW_CLIENT_WIDTH as f32, MAIN_WINDOW_CLIENT_HEIGHT as f32])
        .with_max_inner_size([MAIN_WINDOW_CLIENT_WIDTH as f32, 1000.0])
        .with_resizable(false)
        .with_maximize_button(false)
        .with_decorations(true)
        .with_icon(from_png_bytes(include_bytes!("../../src/gifs/vpnfy.png")).expect("Failed to load icon"));

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
    
    // Получаем wireproxy.exe из встроенных зависимостей
    let deps = match embedded_deps_bytes::ExtractedDeps::get() {
        Ok(paths) => paths,
        Err(e) => {
            eprintln!("Не удалось получить зависимости: {}", e);
            std::process::exit(1);
        }
    };
    
    // Start wireproxy with the config file
    match std::process::Command::new(&deps.wireproxy)
        .arg("-c")
        .arg(conf_path.as_ref())
        .spawn()
    {
        Ok(mut child) => {
            // Wait for wireproxy to finish
            let exit_status = child.wait().unwrap_or_else(|_| std::process::ExitStatus::default());
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

// `launch_chrome_with_proxy` removed — Chrome launch button was deleted from GUI

#[link(name = "shell32")]
extern "system" {
    fn IsUserAnAdmin() -> i32;
}

fn is_elevated() -> bool {
    unsafe { IsUserAnAdmin() != 0 }
}

