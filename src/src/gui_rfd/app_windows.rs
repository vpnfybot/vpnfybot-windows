use super::*;

#[cfg(target_os = "windows")]
use std::mem::ManuallyDrop;
#[cfg(target_os = "windows")]
use windows::core::{ComInterface, GUID, HRESULT, PWSTR};
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::SIZE;
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreatePen, EndPaint, GetMonitorInfoW, GetTextExtentPoint32W, HMONITOR,
    InvalidateRect, LineTo, MonitorFromWindow, MoveToEx, DRAW_TEXT_FORMAT, DT_LEFT, MONITORINFO,
    MONITOR_DEFAULTTONEAREST, PAINTSTRUCT, PS_SOLID,
};
#[cfg(target_os = "windows")]
use windows::Win32::Storage::EnhancedStorage::PKEY_AppUserModel_ID;
#[cfg(target_os = "windows")]
use windows::Win32::System::Com::StructuredStorage::{
    PROPVARIANT, PROPVARIANT_0, PROPVARIANT_0_0, PROPVARIANT_0_0_0,
};
#[cfg(target_os = "windows")]
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED, VT_LPWSTR,
};
#[cfg(target_os = "windows")]
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
#[cfg(target_os = "windows")]
use windows::Win32::UI::Shell::{IShellLinkW, PropertiesSystem::IPropertyStore};
#[cfg(target_os = "windows")]
use windows::Win32::UI::Controls::WM_MOUSELEAVE;
#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    TrackMouseEvent, TRACKMOUSEEVENT, TME_LEAVE,
};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, GetWindowRect, IsWindow,
    LoadCursorW, RegisterClassW, SetCursor, SetLayeredWindowAttributes, SetWindowPos, FindWindowExW,
    HWND_TOPMOST, HTCLIENT, IDC_HAND, LWA_COLORKEY, SWP_NOACTIVATE, SWP_NOOWNERZORDER,
    SWP_SHOWWINDOW, WM_ERASEBKGND, WM_GETOBJECT, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCHITTEST,
    WM_PAINT, WM_SETCURSOR, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_POPUP, WS_VISIBLE,
};

const TRAY_CALLBACK_MESSAGE: u32 = WM_APP + 1;
const WM_COPYGLOBALDATA: u32 = 0x0049;
static mut ORIGINAL_WNDPROC: WNDPROC = None;
const TRAY_ICON_ID: u32 = 1;
#[cfg(target_os = "windows")]
const NOTIFICATION_SHORTCUT_NAME: &str = "vpnfybot-windows.lnk";
#[cfg(target_os = "windows")]
const CLSID_SHELL_LINK: GUID = GUID::from_u128(0x00021401_0000_0000_c000_000000000046);
static DROP_FILE_PATH: OnceLock<Mutex<Option<String>>> = OnceLock::new();
static MINIMIZE_VIA_MINBUTTON: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "windows")]
const TASKBAR_TRAFFIC_WIDGET_CLASS: &str = "vpnfy_taskbar_traffic_widget";
#[cfg(target_os = "windows")]
const TASKBAR_TRAFFIC_WIDGET_WIDTH: f32 = 260.0;
#[cfg(target_os = "windows")]
const TASKBAR_TRAFFIC_WIDGET_HEIGHT: f32 = 42.0;
#[cfg(target_os = "windows")]
static TASKBAR_TRAFFIC_WIDGET_CLASS_REGISTERED: OnceLock<()> = OnceLock::new();
#[cfg(target_os = "windows")]
static TASKBAR_TRAFFIC_WIDGET_DATA: OnceLock<Mutex<TaskbarTrafficWidgetSnapshot>> = OnceLock::new();
#[cfg(target_os = "windows")]
static TASKBAR_TRAFFIC_WIDGET_HOVERED: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "windows")]
static TASKBAR_TRAFFIC_WIDGET_HWND: std::sync::atomic::AtomicIsize =
    std::sync::atomic::AtomicIsize::new(0);
#[cfg(target_os = "windows")]
static TASKBAR_TRAFFIC_WIDGET_WORKER_ENABLED: AtomicBool = AtomicBool::new(true);

#[cfg(target_os = "windows")]
#[derive(Clone, Copy)]
struct TaskbarWidgetAnchor {
    rect: RECT,
}

#[cfg(target_os = "windows")]
#[derive(Clone)]
struct TaskbarTrafficWidgetSnapshot {
    visible: bool,
    upload_bps: f64,
    download_bps: f64,
    history: Vec<(f64, f64)>,
}

#[cfg(target_os = "windows")]
impl Default for TaskbarTrafficWidgetSnapshot {
    fn default() -> Self {
        Self {
            visible: false,
            upload_bps: 0.0,
            download_bps: 0.0,
            history: Vec::new(),
        }
    }
}

#[cfg(target_os = "windows")]
fn windows_error(message: impl Into<String>) -> windows::core::Error {
    windows::core::Error::new(HRESULT(0x80004005u32 as i32), HSTRING::from(message.into()))
}

#[cfg(target_os = "windows")]
fn taskbar_traffic_widget_data() -> &'static Mutex<TaskbarTrafficWidgetSnapshot> {
    TASKBAR_TRAFFIC_WIDGET_DATA.get_or_init(|| Mutex::new(TaskbarTrafficWidgetSnapshot::default()))
}

#[cfg(target_os = "windows")]
pub(super) fn set_taskbar_traffic_widget_worker_enabled(enabled: bool) {
    TASKBAR_TRAFFIC_WIDGET_WORKER_ENABLED.store(enabled, Ordering::Relaxed);
}

#[cfg(target_os = "windows")]
pub(super) fn publish_taskbar_traffic_widget_snapshot(
    visible: bool,
    upload_bps: f64,
    download_bps: f64,
    history: Vec<(f64, f64)>,
) {
    let visible = visible && TASKBAR_TRAFFIC_WIDGET_WORKER_ENABLED.load(Ordering::Relaxed);
    if let Ok(mut guard) = taskbar_traffic_widget_data().lock() {
        *guard = TaskbarTrafficWidgetSnapshot {
            visible,
            upload_bps,
            download_bps,
            history,
        };
    }

    let hwnd_raw = TASKBAR_TRAFFIC_WIDGET_HWND.load(Ordering::Relaxed);
    if hwnd_raw == 0 {
        return;
    }

    let hwnd = HWND(hwnd_raw);
    unsafe {
        if !IsWindow(hwnd).as_bool() {
            TASKBAR_TRAFFIC_WIDGET_HWND.store(0, Ordering::Relaxed);
            return;
        }

        if visible {
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            let _ = InvalidateRect(hwnd, None, false);
        } else {
            TASKBAR_TRAFFIC_WIDGET_HOVERED.store(false, Ordering::Relaxed);
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
    }
}

#[cfg(target_os = "windows")]
fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF((r as u32) | ((g as u32) << 8) | ((b as u32) << 16))
}

#[cfg(target_os = "windows")]
fn taskbar_widget_transparent_color() -> COLORREF {
    rgb(1, 2, 3)
}

#[cfg(target_os = "windows")]
fn format_taskbar_speed(bytes_per_second: f64) -> (String, &'static str) {
    let mut value = bytes_per_second.max(0.0);
    let mut unit_index = 0usize;
    let units = ["B/s", "KB/s", "MB/s", "GB/s"];
    while value >= 1024.0 && unit_index + 1 < units.len() {
        value /= 1024.0;
        unit_index += 1;
    }

    let value_text = if unit_index == 0 {
        format!("{:.0}", value)
    } else if value >= 100.0 {
        format!("{:.0}", value)
    } else {
        format!("{:.1}", value)
    };

    (value_text, units[unit_index])
}

#[cfg(target_os = "windows")]
fn widget_dimensions_for_taskbar(taskbar_width: i32, taskbar_height: i32) -> (i32, i32) {
    let scale = current_ui_scale_factor().max(1.0);
    let width = (TASKBAR_TRAFFIC_WIDGET_WIDTH * scale).round() as i32;
    let desired_height = (TASKBAR_TRAFFIC_WIDGET_HEIGHT * scale).round() as i32;
    let cross_size = taskbar_width.min(taskbar_height).max(1);
    let height = desired_height.min((cross_size - 4).max(30));
    (width.max(180), height.max(30))
}

#[cfg(target_os = "windows")]
fn find_primary_taskbar_window() -> Option<HWND> {
    let hwnd = unsafe { FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null()) };
    (hwnd.0 != 0).then_some(hwnd)
}

#[cfg(target_os = "windows")]
fn monitor_from_window(hwnd: HWND) -> Option<HMONITOR> {
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    (monitor.0 != 0).then_some(monitor)
}

#[cfg(target_os = "windows")]
fn main_window_monitor() -> Option<HMONITOR> {
    let title = to_wide(WINDOW_TITLE);
    let hwnd = unsafe { FindWindowW(None, PCWSTR(title.as_ptr())) };
    if hwnd.0 == 0 {
        None
    } else {
        monitor_from_window(hwnd)
    }
}

#[cfg(target_os = "windows")]
fn taskbar_anchor_from_window(hwnd: HWND) -> Option<TaskbarWidgetAnchor> {
    unsafe {
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).as_bool()
            && rect.right > rect.left
            && rect.bottom > rect.top
        {
            Some(TaskbarWidgetAnchor { rect })
        } else {
            None
        }
    }
}

#[cfg(target_os = "windows")]
fn find_taskbar_window_for_monitor(monitor: HMONITOR) -> Option<HWND> {
    if let Some(primary) = find_primary_taskbar_window() {
        if monitor_from_window(primary).is_some_and(|candidate| candidate.0 == monitor.0) {
            return Some(primary);
        }
    }

    let mut previous = HWND(0);
    loop {
        let hwnd = unsafe {
            FindWindowExW(
                HWND(0),
                previous,
                w!("Shell_SecondaryTrayWnd"),
                PCWSTR::null(),
            )
        };
        if hwnd.0 == 0 {
            break;
        }
        if monitor_from_window(hwnd).is_some_and(|candidate| candidate.0 == monitor.0) {
            return Some(hwnd);
        }
        previous = hwnd;
    }

    None
}

#[cfg(target_os = "windows")]
fn monitor_info(monitor: HMONITOR) -> Option<MONITORINFO> {
    unsafe {
        let mut info = MONITORINFO::default();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor, &mut info).as_bool() {
            Some(info)
        } else {
            None
        }
    }
}

#[cfg(target_os = "windows")]
fn anchor_from_monitor_info(info: MONITORINFO) -> Option<TaskbarWidgetAnchor> {
    let monitor = info.rcMonitor;
    let work = info.rcWork;
    let scale = current_ui_scale_factor().max(1.0);
    let fallback_height = ((TASKBAR_TRAFFIC_WIDGET_HEIGHT * scale).round() as i32).max(30);

    let rect = if work.bottom < monitor.bottom {
        RECT {
            left: monitor.left,
            top: work.bottom,
            right: monitor.right,
            bottom: monitor.bottom,
        }
    } else if work.top > monitor.top {
        RECT {
            left: monitor.left,
            top: monitor.top,
            right: monitor.right,
            bottom: work.top,
        }
    } else if work.left > monitor.left {
        RECT {
            left: monitor.left,
            top: monitor.top,
            right: work.left,
            bottom: monitor.bottom,
        }
    } else if work.right < monitor.right {
        RECT {
            left: work.right,
            top: monitor.top,
            right: monitor.right,
            bottom: monitor.bottom,
        }
    } else {
        RECT {
            left: monitor.left,
            top: monitor.bottom - fallback_height,
            right: monitor.right,
            bottom: monitor.bottom,
        }
    };

    if rect.right > rect.left && rect.bottom > rect.top {
        Some(TaskbarWidgetAnchor { rect })
    } else {
        None
    }
}

#[cfg(target_os = "windows")]
fn taskbar_widget_anchor_for_monitor(monitor: HMONITOR) -> Option<TaskbarWidgetAnchor> {
    find_taskbar_window_for_monitor(monitor)
        .and_then(taskbar_anchor_from_window)
        .or_else(|| monitor_info(monitor).and_then(anchor_from_monitor_info))
}

#[cfg(target_os = "windows")]
fn fallback_taskbar_widget_anchor() -> Option<TaskbarWidgetAnchor> {
    find_primary_taskbar_window()
        .and_then(taskbar_anchor_from_window)
        .or_else(|| main_window_monitor().and_then(taskbar_widget_anchor_for_monitor))
}

#[cfg(target_os = "windows")]
fn position_taskbar_traffic_widget(hwnd: HWND, anchor: TaskbarWidgetAnchor) {
    unsafe {
        let rect = anchor.rect;
        let taskbar_width = (rect.right - rect.left).max(1);
        let taskbar_height = (rect.bottom - rect.top).max(1);
        let (widget_width, widget_height) =
            widget_dimensions_for_taskbar(taskbar_width, taskbar_height);
        let vertical = taskbar_height > taskbar_width;
        let scale = current_ui_scale_factor().max(1.0);
        let edge_padding = (8.0 * scale).round() as i32;

        let (x, y) = if vertical {
            (
                rect.left + ((taskbar_width - widget_width) / 2).max(edge_padding),
                rect.top + edge_padding,
            )
        } else {
            (
                rect.left + edge_padding,
                rect.top + ((taskbar_height - widget_height) / 2).max(2),
            )
        };

        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            x,
            y,
            widget_width,
            widget_height,
            SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_SHOWWINDOW,
        );
    }
}

#[cfg(target_os = "windows")]
fn register_taskbar_traffic_widget_class() {
    let _ = TASKBAR_TRAFFIC_WIDGET_CLASS_REGISTERED.get_or_init(|| unsafe {
        let class_name = to_wide(TASKBAR_TRAFFIC_WIDGET_CLASS);
        if let Ok(hinstance) = GetModuleHandleW(None) {
            let wnd_class = WNDCLASSW {
                lpfnWndProc: Some(taskbar_traffic_widget_wndproc),
                hInstance: hinstance,
                hCursor: LoadCursorW(None, IDC_HAND).unwrap_or_default(),
                lpszClassName: PCWSTR(class_name.as_ptr()),
                ..Default::default()
            };
            let _ = RegisterClassW(&wnd_class);
        }
    });
}

#[cfg(target_os = "windows")]
fn draw_taskbar_text(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    text: &str,
    mut rect: RECT,
    flags: DRAW_TEXT_FORMAT,
    color: COLORREF,
    font: Option<HFONT>,
) {
    unsafe {
        let old_font = font.map(|font| SelectObject(hdc, font));
        let _ = SetTextColor(hdc, color);
        let _ = SetBkMode(hdc, TRANSPARENT);
        let mut text = to_wide(text);
        let _ = DrawTextW(
            hdc,
            &mut text,
            &mut rect,
            flags | DT_SINGLELINE | DT_VCENTER,
        );
        if let Some(old_font) = old_font {
            let _ = SelectObject(hdc, old_font);
        }
    }
}

#[cfg(target_os = "windows")]
fn measure_taskbar_text_width(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    text: &str,
    font: Option<HFONT>,
) -> i32 {
    let text_wide: Vec<u16> = text.encode_utf16().collect();
    if text_wide.is_empty() {
        return 0;
    }

    unsafe {
        let old_font = font.map(|font| SelectObject(hdc, font));
        let mut size = SIZE::default();
        let width = if GetTextExtentPoint32W(hdc, &text_wide, &mut size).as_bool() {
            size.cx.max(0)
        } else {
            0
        };
        if let Some(old_font) = old_font {
            let _ = SelectObject(hdc, old_font);
        }
        width
    }
}

#[cfg(target_os = "windows")]
fn draw_taskbar_speed_row(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    arrow: &str,
    value_text: &str,
    unit_text: &str,
    row_rect: RECT,
    color: COLORREF,
    arrow_font: Option<HFONT>,
    text_font: Option<HFONT>,
) {
    let gap = measure_taskbar_text_width(hdc, " ", text_font).max(1);
    let arrow_width = measure_taskbar_text_width(hdc, arrow, arrow_font);
    let arrow_right = (row_rect.left + arrow_width).min(row_rect.right);
    draw_taskbar_text(
        hdc,
        arrow,
        RECT {
            left: row_rect.left,
            top: row_rect.top,
            right: arrow_right,
            bottom: row_rect.bottom,
        },
        DT_LEFT,
        color,
        arrow_font,
    );

    let value_left = (arrow_right + gap).min(row_rect.right);
    let value_width = measure_taskbar_text_width(hdc, value_text, text_font);
    let value_right = (value_left + value_width).min(row_rect.right);
    if value_left < value_right {
        draw_taskbar_text(
            hdc,
            value_text,
            RECT {
                left: value_left,
                top: row_rect.top,
                right: value_right,
                bottom: row_rect.bottom,
            },
            DT_LEFT,
            color,
            text_font,
        );
    }

    let unit_left = (value_right + gap).min(row_rect.right);
    if unit_left < row_rect.right {
        draw_taskbar_text(
            hdc,
            unit_text,
            RECT {
                left: unit_left,
                top: row_rect.top,
                right: row_rect.right,
                bottom: row_rect.bottom,
            },
            DT_LEFT,
            color,
            text_font,
        );
    }
}

#[cfg(target_os = "windows")]
unsafe fn track_taskbar_widget_mouse_leave(hwnd: HWND) {
    let mut event = TRACKMOUSEEVENT {
        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
        dwFlags: TME_LEAVE,
        hwndTrack: hwnd,
        dwHoverTime: 0,
    };
    let _ = TrackMouseEvent(&mut event);
}

#[cfg(target_os = "windows")]
unsafe fn paint_taskbar_traffic_widget(hwnd: HWND) {
    let snapshot = taskbar_traffic_widget_data()
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_default();

    let mut paint = PAINTSTRUCT::default();
    let hdc = BeginPaint(hwnd, &mut paint);
    if hdc.0 == 0 {
        return;
    }

    let mut rect = RECT::default();
    let _ = GetClientRect(hwnd, &mut rect);
    let width = (rect.right - rect.left).max(1);
    let height = (rect.bottom - rect.top).max(1);
    let transparent_brush = CreateSolidBrush(taskbar_widget_transparent_color());
    let _ = FillRect(hdc, &rect, transparent_brush);

    let scale = current_ui_scale_factor().max(1.0);
    let padding = (10.0 * scale).round() as i32;
    let metrics_width = ((96.0 * scale).round() as i32).min(width / 2).max(74);
    let graph_right = (width - padding - metrics_width).max(padding + 1);
    let graph_rect = RECT {
        left: padding,
        top: (7.0 * scale).round() as i32,
        right: graph_right,
        bottom: height - (7.0 * scale).round() as i32,
    };
    let history = snapshot.history;
    let max_bps = history
        .iter()
        .map(|(up, down)| up + down)
        .fold(snapshot.upload_bps + snapshot.download_bps, f64::max)
        .max(1.0);
    let graph_width = (graph_rect.right - graph_rect.left).max(1);
    let graph_height = (graph_rect.bottom - graph_rect.top).max(1);
    let bar_slots = TASKBAR_TRAFFIC_HISTORY_CAPACITY.max(1);
    let bar_width = ((graph_width as f32 / bar_slots as f32).floor() as i32).max(2);
    let start_index = history
        .len()
        .saturating_sub(TASKBAR_TRAFFIC_HISTORY_CAPACITY);
    let visible_history = &history[start_index..];
    let slot_offset = bar_slots.saturating_sub(visible_history.len());
    let graph_brush = CreateSolidBrush(rgb(245, 247, 250));

    for (index, (upload, download)) in visible_history.iter().enumerate() {
        let slot = slot_offset + index;
        let x = graph_rect.left + ((slot as i32) * graph_width / bar_slots as i32);
        let combined = (*upload + *download).max(0.0);
        if combined <= 0.0 {
            continue;
        }

        let bar_height = ((combined / max_bps) * graph_height as f64).round() as i32;
        let bar_top = (graph_rect.bottom - bar_height).clamp(graph_rect.top, graph_rect.bottom);
        if bar_top < graph_rect.bottom {
            let bar_rect = RECT {
                left: x,
                top: bar_top,
                right: (x + bar_width - 1).min(graph_rect.right),
                bottom: graph_rect.bottom,
            };
            let _ = FillRect(hdc, &bar_rect, graph_brush);
        }
    }

    let baseline_pen = CreatePen(PS_SOLID, 1, rgb(172, 178, 188));
    let _ = SelectObject(hdc, baseline_pen);
    let _ = MoveToEx(hdc, graph_rect.left, graph_rect.bottom, None);
    let _ = LineTo(hdc, graph_rect.right, graph_rect.bottom);

    let font = create_smooth_ui_font_bold((11.0 * scale).round().max(1.0) as i32);
    let arrow_font = create_smooth_ui_font((15.0 * scale).round().max(1.0) as i32);
    let graph_text_gap = measure_taskbar_text_width(hdc, " ", font).max(1);
    let metrics_rect = RECT {
        left: (graph_rect.right + graph_text_gap).min(width - padding),
        top: 0,
        right: width - padding,
        bottom: height,
    };
    let (down_value, down_unit) = format_taskbar_speed(snapshot.download_bps);
    let (up_value, up_unit) = format_taskbar_speed(snapshot.upload_bps);
    draw_taskbar_speed_row(
        hdc,
        "\u{2191}",
        &up_value,
        up_unit,
        RECT {
            left: metrics_rect.left,
            top: 3,
            right: metrics_rect.right,
            bottom: height / 2,
        },
        rgb(232, 238, 246),
        arrow_font,
        font,
    );
    draw_taskbar_speed_row(
        hdc,
        "\u{2193}",
        &down_value,
        down_unit,
        RECT {
            left: metrics_rect.left,
            top: height / 2,
            right: metrics_rect.right,
            bottom: height - 3,
        },
        rgb(232, 238, 246),
        arrow_font,
        font,
    );

    if let Some(arrow_font) = arrow_font {
        let _ = DeleteObject(arrow_font);
    }
    if let Some(font) = font {
        let _ = DeleteObject(font);
    }
    let _ = DeleteObject(transparent_brush);
    let _ = DeleteObject(graph_brush);
    let _ = DeleteObject(baseline_pen);
    let _ = EndPaint(hwnd, &paint);
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn taskbar_traffic_widget_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCHITTEST => LRESULT(HTCLIENT as isize),
        WM_SETCURSOR => {
            if let Ok(cursor) = LoadCursorW(None, IDC_HAND) {
                let _ = SetCursor(cursor);
            }
            LRESULT(1)
        }
        WM_MOUSEMOVE => {
            track_taskbar_widget_mouse_leave(hwnd);
            if !TASKBAR_TRAFFIC_WIDGET_HOVERED.swap(true, Ordering::Relaxed) {
                let _ = InvalidateRect(hwnd, None, false);
            }
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            if TASKBAR_TRAFFIC_WIDGET_HOVERED.swap(false, Ordering::Relaxed) {
                let _ = InvalidateRect(hwnd, None, false);
            }
            LRESULT(0)
        }
        WM_PAINT => {
            paint_taskbar_traffic_widget(hwnd);
            LRESULT(0)
        }
        WM_ERASEBKGND | WM_GETOBJECT => LRESULT(1),
        WM_LBUTTONUP => {
            let title = to_wide(WINDOW_TITLE);
            let main_hwnd = FindWindowW(None, PCWSTR(title.as_ptr()));
            if main_hwnd.0 != 0 {
                let _ = ShowWindow(main_hwnd, SW_RESTORE);
                let _ = SetForegroundWindow(main_hwnd);
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

#[cfg(target_os = "windows")]
fn path_to_wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

#[cfg(target_os = "windows")]
fn string_propvariant_from_wide(value: &mut [u16]) -> PROPVARIANT {
    PROPVARIANT {
        Anonymous: PROPVARIANT_0 {
            Anonymous: ManuallyDrop::new(PROPVARIANT_0_0 {
                vt: VT_LPWSTR,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: PROPVARIANT_0_0_0 {
                    pwszVal: PWSTR(value.as_mut_ptr()),
                },
            }),
        },
    }
}

#[cfg(target_os = "windows")]
fn start_menu_shortcut_path() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    Some(
        PathBuf::from(appdata)
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs")
            .join(NOTIFICATION_SHORTCUT_NAME),
    )
}

#[cfg(target_os = "windows")]
pub(super) fn ensure_notification_shortcut_registered() -> windows::core::Result<Option<PathBuf>> {
    let Some(shortcut_path) = start_menu_shortcut_path() else {
        return Ok(None);
    };

    let exe_path = std::env::current_exe()
        .map_err(|error| windows_error(format!("current_exe failed: {}", error)))?;
    let exe_dir = exe_path
        .parent()
        .ok_or_else(|| windows_error("current_exe returned a path without a parent directory"))?;

    if let Some(parent) = shortcut_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            windows_error(format!(
                "create_dir_all for shortcut directory failed: {}",
                error
            ))
        })?;
    }

    let com_initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok() };
    let result = (|| -> windows::core::Result<Option<PathBuf>> {
        let shell_link: IShellLinkW =
            unsafe { CoCreateInstance(&CLSID_SHELL_LINK, None, CLSCTX_INPROC_SERVER)? };

        let exe_wide = path_to_wide(&exe_path);
        let exe_dir_wide = path_to_wide(exe_dir);
        let description_wide = to_wide(WINDOW_TITLE);
        let mut app_id_wide = to_wide(NOTIFICATION_APP_ID);
        let app_id_prop = string_propvariant_from_wide(&mut app_id_wide);
        let shortcut_wide = path_to_wide(&shortcut_path);

        unsafe {
            shell_link.SetPath(PCWSTR(exe_wide.as_ptr()))?;
            shell_link.SetWorkingDirectory(PCWSTR(exe_dir_wide.as_ptr()))?;
            shell_link.SetDescription(PCWSTR(description_wide.as_ptr()))?;
            shell_link.SetIconLocation(PCWSTR(exe_wide.as_ptr()), 0)?;

            let property_store: IPropertyStore = shell_link.cast()?;
            property_store.SetValue(&PKEY_AppUserModel_ID, &app_id_prop)?;
            property_store.Commit()?;

            let persist_file: IPersistFile = shell_link.cast()?;
            persist_file.Save(PCWSTR(shortcut_wide.as_ptr()), BOOL(1))?;
        }

        Ok(Some(shortcut_path))
    })();

    if com_initialized {
        unsafe {
            CoUninitialize();
        }
    }

    result
}

#[cfg(target_os = "windows")]
unsafe fn enable_file_drop_for_window(hwnd: HWND) {
    if hwnd.0 == 0 {
        return;
    }

    let _ = DragAcceptFiles(hwnd, BOOL(1));
    let _ = ChangeWindowMessageFilterEx(hwnd, WM_DROPFILES, MSGFLT_ALLOW, None);
    let _ = ChangeWindowMessageFilterEx(hwnd, WM_COPYDATA, MSGFLT_ALLOW, None);
    let _ = ChangeWindowMessageFilterEx(hwnd, WM_COPYGLOBALDATA, MSGFLT_ALLOW, None);
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn enable_file_drop_for_children(hwnd: HWND, _lparam: LPARAM) -> BOOL {
    enable_file_drop_for_window(hwnd);
    BOOL(1)
}

#[cfg(target_os = "windows")]
unsafe fn enable_file_drop(hwnd: HWND) {
    let root_hwnd = GetAncestor(hwnd, GA_ROOT);
    let target_hwnd = if root_hwnd.0 != 0 { root_hwnd } else { hwnd };

    enable_file_drop_for_window(target_hwnd);
    let _ = EnumChildWindows(target_hwnd, Some(enable_file_drop_for_children), LPARAM(0));
}

pub(super) fn open_url(url: &str) {
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

pub(super) fn show_error_dialog(title: &str, message: &str) {
    error_dialog::show(title.to_owned(), message.to_owned());
}

unsafe extern "system" fn subclass_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCLBUTTONDOWN => {
            if (wparam.0 as u32) == HTMINBUTTON {
                MINIMIZE_VIA_MINBUTTON.store(true, Ordering::SeqCst);
            }
        }
        WM_SIZE => {
            if wparam.0 as u32 == SIZE_MINIMIZED {
                let via_min_button = MINIMIZE_VIA_MINBUTTON.swap(false, Ordering::SeqCst);
                if via_min_button {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }
            }
        }
        WM_DROPFILES => {
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
                    if Path::new(&path)
                        .extension()
                        .and_then(|e| e.to_str())
                        .map_or(false, |ext| ext.eq_ignore_ascii_case("conf"))
                    {
                        let drop_storage = DROP_FILE_PATH.get_or_init(|| Mutex::new(None));
                        let mut guard = drop_storage.lock().unwrap();
                        *guard = Some(path);
                    }
                }
            }
            DragFinish(hdrop);
        }
        TRAY_CALLBACK_MESSAGE => match lparam.0 as u32 {
            WM_LBUTTONUP | WM_RBUTTONUP => {
                let _ = ShowWindow(hwnd, SW_RESTORE);
                let _ = SetForegroundWindow(hwnd);
            }
            _ => {}
        },
        _ => {}
    }

    CallWindowProcW(ORIGINAL_WNDPROC, hwnd, msg, wparam, lparam)
}

impl AppState {
    #[cfg(target_os = "windows")]
    fn taskbar_widget_anchor(&mut self) -> Option<TaskbarWidgetAnchor> {
        if let Some(monitor) = main_window_monitor() {
            self.taskbar_widget_monitor = Some(monitor);
        }

        self.taskbar_widget_monitor
            .and_then(taskbar_widget_anchor_for_monitor)
            .or_else(fallback_taskbar_widget_anchor)
    }

    #[cfg(target_os = "windows")]
    pub(super) fn update_taskbar_traffic_widget(&mut self) {
        set_taskbar_traffic_widget_worker_enabled(self.taskbar_widget_enabled);
        let anchor = self.taskbar_widget_anchor();
        let snapshot = TaskbarTrafficWidgetSnapshot {
            visible: self.service_active && self.taskbar_widget_enabled,
            upload_bps: self.last_upload_bps,
            download_bps: self.last_download_bps,
            history: self
                .traffic_history
                .iter()
                .map(|point| (point.upload_bps, point.download_bps))
                .collect(),
        };

        if let Ok(mut guard) = taskbar_traffic_widget_data().lock() {
            *guard = snapshot.clone();
        }

        let Some(existing_hwnd) = self.taskbar_widget_window else {
            if !snapshot.visible {
                return;
            }
            if let Some(anchor) = anchor {
                self.taskbar_widget_window = self.create_taskbar_traffic_widget(anchor);
            }
            if self.taskbar_widget_window.is_none() {
                return;
            }
            return self.update_taskbar_traffic_widget();
        };

        unsafe {
            if !IsWindow(existing_hwnd).as_bool() {
                self.taskbar_widget_window = None;
                TASKBAR_TRAFFIC_WIDGET_HWND.store(0, Ordering::Relaxed);
                return self.update_taskbar_traffic_widget();
            }

            if !snapshot.visible {
                TASKBAR_TRAFFIC_WIDGET_HOVERED.store(false, Ordering::Relaxed);
                let _ = ShowWindow(existing_hwnd, SW_HIDE);
                return;
            }

            let Some(anchor) = anchor else {
                TASKBAR_TRAFFIC_WIDGET_HOVERED.store(false, Ordering::Relaxed);
                let _ = ShowWindow(existing_hwnd, SW_HIDE);
                return;
            };

            position_taskbar_traffic_widget(existing_hwnd, anchor);
            let _ = ShowWindow(existing_hwnd, SW_SHOWNOACTIVATE);
            let _ = InvalidateRect(existing_hwnd, None, false);
        }
    }

    #[cfg(target_os = "windows")]
    fn create_taskbar_traffic_widget(&self, anchor: TaskbarWidgetAnchor) -> Option<HWND> {
        unsafe {
            register_taskbar_traffic_widget_class();
            let hinstance = GetModuleHandleW(None).ok()?;
            let class_name = to_wide(TASKBAR_TRAFFIC_WIDGET_CLASS);
            let title = to_wide("vpnfybot traffic");
            let rect = anchor.rect;
            let taskbar_width = (rect.right - rect.left).max(1);
            let taskbar_height = (rect.bottom - rect.top).max(1);
            let (widget_width, widget_height) =
                widget_dimensions_for_taskbar(taskbar_width, taskbar_height);

            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST | WS_EX_LAYERED,
                PCWSTR(class_name.as_ptr()),
                PCWSTR(title.as_ptr()),
                WS_POPUP | WS_VISIBLE,
                rect.left,
                rect.top,
                widget_width,
                widget_height,
                HWND(0),
                None,
                hinstance,
                None,
            );

            if hwnd.0 == 0 {
                None
            } else {
                let _ = SetLayeredWindowAttributes(
                    hwnd,
                    taskbar_widget_transparent_color(),
                    0,
                    LWA_COLORKEY,
                );
                position_taskbar_traffic_widget(hwnd, anchor);
                TASKBAR_TRAFFIC_WIDGET_HWND.store(hwnd.0, Ordering::Relaxed);
                Some(hwnd)
            }
        }
    }

    #[cfg(target_os = "windows")]
    pub(super) fn destroy_taskbar_traffic_widget(&mut self) {
        set_taskbar_traffic_widget_worker_enabled(false);
        TASKBAR_TRAFFIC_WIDGET_HOVERED.store(false, Ordering::Relaxed);
        if let Some(hwnd) = self.taskbar_widget_window.take() {
            unsafe {
                if IsWindow(hwnd).as_bool() {
                    let _ = DestroyWindow(hwnd);
                }
            }
        }
        TASKBAR_TRAFFIC_WIDGET_HWND.store(0, Ordering::Relaxed);
    }

    #[cfg(target_os = "windows")]
    #[allow(deprecated)]
    pub(super) fn ensure_tray_subclass(&mut self, frame: &mut Frame) {
        if self.tray_subclassed {
            return;
        }

        if let Ok(window_handle) = frame.window_handle() {
            if let Ok(RawWindowHandle::Win32(handle)) = window_handle.raw_window_handle() {
                let raw_hwnd = HWND(handle.hwnd.get());
                let root_hwnd = unsafe { GetAncestor(raw_hwnd, GA_ROOT) };
                let hwnd = if root_hwnd.0 != 0 {
                    root_hwnd
                } else {
                    raw_hwnd
                };
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
                        let prev = SetWindowLongPtrW(
                            hwnd,
                            GWLP_WNDPROC,
                            subclass_wndproc as *const () as isize,
                        );
                        ORIGINAL_WNDPROC = std::mem::transmute(prev);
                        enable_file_drop(hwnd);
                    }
                    self.tray_subclassed = true;
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    fn load_tray_icon(&self) -> Option<HICON> {
        let icon_data = from_png_bytes(include_bytes!("../../gifs/vpnfy.png")).ok()?;
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
    pub(super) fn add_tray_icon(&mut self, hwnd: HWND) {
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
    pub(super) fn remove_tray_icon(&mut self) {
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

    pub(super) fn show_silent_windows_notification(
        &mut self,
        title: &str,
        message: &str,
        launch: &str,
    ) {
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
                "<toast duration=\"short\" launch=\"{}\"><visual><binding template=\"ToastGeneric\">{}<text>{}</text><text>{}</text></binding></visual><audio silent=\"true\"/></toast>",
                xml_escape(launch),
                image_xml,
                xml_escape(title),
                xml_escape(message),
            );
            let xml_hstring = HSTRING::from(xml);
            toast_xml.LoadXml(&xml_hstring)?;
            let toast = ToastNotification::CreateToastNotification(&toast_xml)?;
            let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(
                NOTIFICATION_APP_ID,
            ))?;

            if let Some(existing) = self.last_notification.take() {
                let _ = notifier.Hide(&existing);
            }

            notifier.Show(&toast)?;
            self.last_notification = Some(toast);
            Ok(())
        })();

        if let Err(e) = result {
            eprintln!("⚠ Не удалось показать тихое Windows-уведомление: {}", e);
        }
    }

    #[cfg(target_os = "windows")]
    pub(super) fn apply_black_window_frame(&self, _frame: &Frame) -> bool {
        unsafe {
            let title_wide: Vec<u16> = OsStr::new(WINDOW_TITLE)
                .encode_wide()
                .chain(Some(0))
                .collect();
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
                return true;
            }
        }

        false
    }

    pub(super) fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let maybe_path = ctx
            .input(|input| {
                input.raw.dropped_files.iter().find_map(|file| {
                    let path = file.path.as_ref()?;
                    let extension = path.extension().and_then(|ext| ext.to_str())?;
                    if extension.eq_ignore_ascii_case("conf") {
                        Some(path.to_string_lossy().to_string())
                    } else {
                        None
                    }
                })
            })
            .or_else(|| {
                DROP_FILE_PATH
                    .get_or_init(|| Mutex::new(None))
                    .lock()
                    .unwrap()
                    .take()
            });

        let path = match maybe_path {
            Some(path) => path,
            None => return,
        };

        if self.service_running || self.service_active {
            self.status = self
                .language
                .translate("Отключите туннель перед импортом конфигурации")
                .to_owned();
            show_error_dialog(self.language.translate("Ошибка"), &self.status);
            return;
        }

        self.set_imported_conf_path(path);
    }
}

pub(super) fn show_silent_windows_notification_detached(
    title: &str,
    message: &str,
    launch: &str,
) -> windows::core::Result<()> {
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
        "<toast duration=\"short\" launch=\"{}\"><visual><binding template=\"ToastGeneric\">{}<text>{}</text><text>{}</text></binding></visual><audio silent=\"true\"/></toast>",
        xml_escape(launch),
        image_xml,
        xml_escape(title),
        xml_escape(message),
    );
    let xml_hstring = HSTRING::from(xml);
    toast_xml.LoadXml(&xml_hstring)?;
    let toast = ToastNotification::CreateToastNotification(&toast_xml)?;
    let notifier =
        ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(NOTIFICATION_APP_ID))?;
    notifier.Show(&toast)?;
    Ok(())
}
