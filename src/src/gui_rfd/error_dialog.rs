use std::thread;

use windows::core::{PCWSTR, w};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DwmSetWindowAttribute};
use windows::Win32::Graphics::Gdi::{CreateSolidBrush, DeleteObject, HFONT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyWindow, DispatchMessageW,
    GetClientRect, GetMessageW, GetWindowLongPtrW, HICON, HMENU, IDC_ARROW,
    LoadCursorW, MSG, MoveWindow, PostQuitMessage, RegisterClassW, SendMessageW,
    SetForegroundWindow, SetWindowLongPtrW, ShowWindow, TranslateMessage,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_COMMAND, WM_DESTROY, WM_SETICON,
    WM_SIZE, WNDCLASSW, WS_CHILD, WS_EX_CLIENTEDGE, WS_HSCROLL, WS_MAXIMIZEBOX,
    WS_OVERLAPPEDWINDOW, WS_THICKFRAME, WS_VISIBLE, WS_VSCROLL, CW_USEDEFAULT,
    GWLP_USERDATA, SW_SHOW,
};

use super::{
    adjusted_window_size, apply_smooth_font, create_smooth_ui_font, load_png_icon_handle,
    to_wide, UI_BUTTON_FONT_SIZE,
};

const ERROR_DIALOG_CLASS: &str = "vpnfy_error_dialog_class";
const ERROR_DIALOG_WIDTH: i32 = 560;
const ERROR_DIALOG_HEIGHT: i32 = 360;
const ERROR_DIALOG_PADDING: i32 = 12;
const ERROR_DIALOG_BUTTON_HEIGHT: i32 = 32;
const ERROR_DIALOG_BUTTON_WIDTH: i32 = 120;
const ERROR_DIALOG_COPY_BUTTON_ID: usize = 101;
const ERROR_DIALOG_CLOSE_BUTTON_ID: usize = 102;
const EM_SETSEL: u32 = 0x00B1;
const WM_COPY_MESSAGE: u32 = 0x0301;
const ES_MULTILINE: u32 = 0x0004;
const ES_AUTOVSCROLL: u32 = 0x0040;
const ES_AUTOHSCROLL: u32 = 0x0080;
const ES_READONLY: u32 = 0x0800;
const ES_WANTRETURN: u32 = 0x1000;

struct ErrorDialogState {
    edit_hwnd: HWND,
    copy_hwnd: HWND,
    close_hwnd: HWND,
    ui_font: Option<HFONT>,
    window_icon: Option<HICON>,
}

pub(super) fn show(title: String, message: String) {
    thread::spawn(move || unsafe {
        let class_name = to_wide(ERROR_DIALOG_CLASS);
        let title_wide = to_wide(&title);
        let message_wide = to_wide(&message);
        let copy_label = if title.eq_ignore_ascii_case("Error") { "Copy" } else { "Копировать" };
        let close_label = if title.eq_ignore_ascii_case("Error") { "Close" } else { "Закрыть" };
        let copy_label_wide = to_wide(copy_label);
        let close_label_wide = to_wide(close_label);
        let ui_font = create_smooth_ui_font(UI_BUTTON_FONT_SIZE as i32);
        let window_icon = load_png_icon_handle(include_bytes!("../../../src/gifs/vpnfy.png"));

        let hinstance = GetModuleHandleW(None).unwrap();
        let background_brush = CreateSolidBrush(COLORREF(0));
        let wnd_class = WNDCLASSW {
            lpfnWndProc: Some(error_dialog_wndproc),
            hInstance: hinstance,
            hIcon: window_icon.unwrap_or_default(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap(),
            hbrBackground: background_brush,
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        let _ = RegisterClassW(&wnd_class);

        let window_ex_style: WINDOW_EX_STYLE = Default::default();
        let window_style = WS_OVERLAPPEDWINDOW & !WS_THICKFRAME & !WS_MAXIMIZEBOX;
        let (window_width, window_height) = adjusted_window_size(
            window_style,
            window_ex_style,
            ERROR_DIALOG_WIDTH,
            ERROR_DIALOG_HEIGHT,
        );

        let hwnd = CreateWindowExW(
            window_ex_style,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title_wide.as_ptr()),
            window_style,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            window_width,
            window_height,
            None,
            None,
            hinstance,
            None,
        );

        if hwnd.0 == 0 {
            if let Some(window_icon) = window_icon {
                let _ = DestroyIcon(window_icon);
            }
            if let Some(font) = ui_font {
                let _ = DeleteObject(font);
            }
            return;
        }

        if let Some(window_icon) = window_icon {
            let _ = SendMessageW(hwnd, WM_SETICON, WPARAM(1), LPARAM(window_icon.0));
            let _ = SendMessageW(hwnd, WM_SETICON, WPARAM(0), LPARAM(window_icon.0));
        }

        let caption_color: u32 = 0x000000;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_CAPTION_COLOR,
            &caption_color as *const _ as *const _,
            std::mem::size_of::<u32>() as u32,
        );
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            &caption_color as *const _ as *const _,
            std::mem::size_of::<u32>() as u32,
        );

        let edit_style = WINDOW_STYLE(
            WS_CHILD.0
                | WS_VISIBLE.0
                | WS_VSCROLL.0
                | WS_HSCROLL.0
                | ES_MULTILINE
                | ES_AUTOVSCROLL
                | ES_AUTOHSCROLL
                | ES_READONLY
                | ES_WANTRETURN,
        );
        let edit_hwnd = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            PCWSTR(message_wide.as_ptr()),
            edit_style,
            0,
            0,
            1,
            1,
            hwnd,
            HMENU(1),
            hinstance,
            None,
        );

        let copy_hwnd = CreateWindowExW(
            Default::default(),
            w!("BUTTON"),
            PCWSTR(copy_label_wide.as_ptr()),
            WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0),
            0,
            0,
            1,
            1,
            hwnd,
            HMENU(ERROR_DIALOG_COPY_BUTTON_ID as isize),
            hinstance,
            None,
        );

        let close_hwnd = CreateWindowExW(
            Default::default(),
            w!("BUTTON"),
            PCWSTR(close_label_wide.as_ptr()),
            WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0),
            0,
            0,
            1,
            1,
            hwnd,
            HMENU(ERROR_DIALOG_CLOSE_BUTTON_ID as isize),
            hinstance,
            None,
        );

        if let Some(font) = ui_font {
            apply_smooth_font(edit_hwnd, font);
            apply_smooth_font(copy_hwnd, font);
            apply_smooth_font(close_hwnd, font);
        }

        let state = Box::new(ErrorDialogState {
            edit_hwnd,
            copy_hwnd,
            close_hwnd,
            ui_font,
            window_icon,
        });
        let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize);
        layout_error_dialog(hwnd);

        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(edit_hwnd);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            let _ = DispatchMessageW(&msg);
        }
    });
}

unsafe extern "system" fn error_dialog_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_SIZE => {
            layout_error_dialog(hwnd);
            return LRESULT(0);
        }
        WM_COMMAND => {
            let control_id = wparam.0 & 0xffff;
            match control_id {
                ERROR_DIALOG_COPY_BUTTON_ID => {
                    if let Some(state) = get_state(hwnd) {
                        let _ = SendMessageW(state.edit_hwnd, EM_SETSEL, WPARAM(0), LPARAM(-1));
                        let _ = SendMessageW(state.edit_hwnd, WM_COPY_MESSAGE, WPARAM(0), LPARAM(0));
                        let _ = SetFocus(state.edit_hwnd);
                    }
                    return LRESULT(0);
                }
                ERROR_DIALOG_CLOSE_BUTTON_ID => {
                    let _ = DestroyWindow(hwnd);
                    return LRESULT(0);
                }
                _ => {}
            }
        }
        WM_CLOSE => {
            let _ = DestroyWindow(hwnd);
            return LRESULT(0);
        }
        WM_DESTROY => {
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ErrorDialogState;
            if !state_ptr.is_null() {
                let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                let state = Box::from_raw(state_ptr);
                if let Some(font) = state.ui_font {
                    let _ = DeleteObject(font);
                }
                if let Some(window_icon) = state.window_icon {
                    let _ = DestroyIcon(window_icon);
                }
            }
            PostQuitMessage(0);
            return LRESULT(0);
        }
        _ => {}
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}

unsafe fn get_state(hwnd: HWND) -> Option<&'static mut ErrorDialogState> {
    let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ErrorDialogState;
    state_ptr.as_mut()
}

unsafe fn layout_error_dialog(hwnd: HWND) {
    let Some(state) = get_state(hwnd) else {
        return;
    };

    let mut client_rect = RECT::default();
    let _ = GetClientRect(hwnd, &mut client_rect);
    let client_width = (client_rect.right - client_rect.left).max(0);
    let client_height = (client_rect.bottom - client_rect.top).max(0);
    let buttons_top = (client_height - ERROR_DIALOG_PADDING - ERROR_DIALOG_BUTTON_HEIGHT).max(ERROR_DIALOG_PADDING);
    let edit_height = (buttons_top - ERROR_DIALOG_PADDING * 2).max(80);
    let button_y = buttons_top;
    let close_x = (client_width - ERROR_DIALOG_PADDING - ERROR_DIALOG_BUTTON_WIDTH).max(ERROR_DIALOG_PADDING);
    let copy_x = (close_x - ERROR_DIALOG_PADDING - ERROR_DIALOG_BUTTON_WIDTH).max(ERROR_DIALOG_PADDING);
    let edit_width = (client_width - ERROR_DIALOG_PADDING * 2).max(120);

    let _ = MoveWindow(
        state.edit_hwnd,
        ERROR_DIALOG_PADDING,
        ERROR_DIALOG_PADDING,
        edit_width,
        edit_height,
        true,
    );
    let _ = MoveWindow(
        state.copy_hwnd,
        copy_x,
        button_y,
        ERROR_DIALOG_BUTTON_WIDTH,
        ERROR_DIALOG_BUTTON_HEIGHT,
        true,
    );
    let _ = MoveWindow(
        state.close_hwnd,
        close_x,
        button_y,
        ERROR_DIALOG_BUTTON_WIDTH,
        ERROR_DIALOG_BUTTON_HEIGHT,
        true,
    );
}