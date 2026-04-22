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
use std::sync::{mpsc::{self, Receiver}, Arc, Mutex, OnceLock};
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
    AdjustWindowRectEx, CallWindowProcW, ChangeWindowMessageFilterEx, CreateIconIndirect, EnumChildWindows, FindWindowW,
    DestroyIcon, SendMessageW,
    GetAncestor, SetForegroundWindow, SetWindowLongPtrW,
    ShowWindow, GWLP_WNDPROC, HICON, ICONINFO, WINDOW_EX_STYLE, WINDOW_STYLE,
    GA_ROOT, MSGFLT_ALLOW, WM_APP, WM_COPYDATA, WM_DROPFILES, WM_LBUTTONUP, WM_RBUTTONUP, WM_SETFONT, WM_SIZE, WNDPROC,
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

struct TunnelTrafficSample {
    total_bytes: u64,
    tx_bytes: u64,
    rx_bytes: u64,
    captured_at: Instant,
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

    fn translate(&self, key: &'static str) -> &'static str {
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
        }
    }
}

struct AppState {
    conf_path: Option<String>,
    status: String,
    error_log: Option<String>,
    status_rx: Option<Receiver<ServiceResult>>,
    service_running: bool,
    service_active: bool,
    session_traffic_bytes: u64,
    session_base_traffic_bytes: Option<u64>,
    connected_at: Option<Instant>,
    startup_animation_frame: usize,
    wireproxy_info_addr: Option<String>,
    last_tunnel_traffic_poll: Option<Instant>,
    traffic_worker_receiver: Option<Receiver<TunnelTrafficSample>>,
    traffic_worker_stop: Option<Arc<AtomicBool>>,
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
    window_frame_attempts: u32,
    tray_subclassed: bool,
    tray_icon_added: bool,
    tray_window: Option<HWND>,
    tray_icon: Option<HICON>,
    traffic_opacity: f32,
    import_button_opacity: f32,
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
    proxy_mode_toggle: bool,
    proxybridge_child: Option<std::process::Child>,
    language: Language,
    win_text_cache: std::collections::HashMap<String, egui::TextureHandle>,
    button_hfont: Option<HFONT>,
    button_hfont_light: Option<HFONT>,
}

#[path = "gui_rfd/app_runtime.rs"]
mod app_runtime;
#[path = "gui_rfd/app_view.rs"]
mod app_view;
#[path = "gui_rfd/app_windows.rs"]
mod app_windows;

use self::app_runtime::is_elevated;
pub(crate) use self::app_runtime::app_main;

