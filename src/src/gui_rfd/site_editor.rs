use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use windows::core::{PCWSTR, w};
use windows::Win32::Foundation::{BOOL, COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DwmSetWindowAttribute};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreatePen, CreateSolidBrush, DT_CENTER, DT_SINGLELINE, DT_VCENTER,
    DeleteObject, DrawTextW, EndPaint, GetDC, GetTextMetricsW, HDC, HFONT, InvalidateRect,
    PAINTSTRUCT, PS_SOLID, ReleaseDC, RoundRect, SelectObject, SetBkColor, SetBkMode,
    SetTextColor, TEXTMETRICW, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{
    DRAWITEMSTRUCT, EM_GETFIRSTVISIBLELINE, EM_GETLINECOUNT, EM_LINESCROLL, ODS_DISABLED,
    ODS_SELECTED, ShowScrollBar,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, SetFocus, VK_LBUTTON,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BS_OWNERDRAW, CW_USEDEFAULT, CallWindowProcW, CreateWindowExW, DefWindowProcW,
    DestroyIcon, DestroyWindow, DispatchMessageW, GCLP_HBRBACKGROUND, GWLP_USERDATA,
    GWLP_WNDPROC, GetClassLongPtrW, GetClientRect, GetMessageW, GetWindowLongPtrW, HICON,
    HMENU, IDC_ARROW, IDC_HAND, LoadCursorW, MSG, MoveWindow, PostQuitMessage,
    RegisterClassW, SCROLLBAR_CONSTANTS, SIZE_MINIMIZED, SW_SHOW, SendMessageW, SetCursor,
    SetWindowLongPtrW, ShowWindow, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE,
    WM_COMMAND, WM_CTLCOLORBTN, WM_CTLCOLOREDIT, WM_DESTROY, WM_DRAWITEM, WM_ERASEBKGND,
    WM_GETTEXT, WM_GETTEXTLENGTH, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
    WM_MOUSEWHEEL, WM_PAINT, WM_SETCURSOR, WM_SETFOCUS, WM_SETICON, WM_SIZE, WNDCLASSW,
    WNDPROC, WS_CHILD, WS_MAXIMIZEBOX, WS_OVERLAPPEDWINDOW, WS_THICKFRAME, WS_VISIBLE,
    WS_VSCROLL,
};

use super::{
    MAIN_WINDOW_CLIENT_HEIGHT, MAIN_WINDOW_CLIENT_WIDTH, PROCESS_EDITOR_GAP,
    PROCESS_EDITOR_PADDING, PROCESS_SAVE_BUTTON_HEIGHT, SITE_SCROLLBAR_WIDTH,
    SITE_TEXT_CONTAINER_CLASS, SITE_TEXT_LINE_HEIGHT, SITE_WHEEL_STEP, SITES_EDITOR_CLASS,
    UI_BUTTON_FONT_SIZE, adjusted_window_size, apply_smooth_font, create_button_ui_font,
    create_smooth_ui_font,
    external_editor_is_open, grayscale_color, load_png_icon_handle,
    mouse_point_from_lparam, rect_contains_point, show_existing_external_editor, to_wide,
};

struct SiteEditorState {
    tx: Option<Sender<Option<String>>>,
    container_hwnd: HWND,
    list_hwnd: HWND,
    save_hwnd: HWND,
    save_label: String,
    save_hovered: bool,
    scrollbar_dragging: bool,
    scrollbar_drag_offset: i32,
    list_original_wndproc: isize,
    ui_font: Option<HFONT>,
    button_font: Option<HFONT>,
    window_icon: Option<HICON>,
}

pub(super) fn is_open() -> bool {
    external_editor_is_open(SITES_EDITOR_CLASS)
}

pub(super) fn show_existing() -> bool {
    show_existing_external_editor(SITES_EDITOR_CLASS)
}

pub(super) fn open_external(
    initial_text: String,
    window_title: String,
    save_label: String,
) -> Receiver<Option<String>> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || unsafe {
        let class_name = to_wide(SITES_EDITOR_CLASS);
        let text_container_class_name = to_wide(SITE_TEXT_CONTAINER_CLASS);
        let title_text = to_wide(&window_title);
        let save_text = to_wide(&save_label);
        let initial_text_wide = to_wide(&initial_text);
        let ui_font = create_smooth_ui_font(UI_BUTTON_FONT_SIZE as i32);
        let button_font = create_button_ui_font();
        let window_icon = load_png_icon_handle(include_bytes!("../../../src/gifs/vpnfy.png"));

        let hinstance = GetModuleHandleW(None).unwrap();
        let background_brush = CreateSolidBrush(COLORREF(0));
        let wnd_class = WNDCLASSW {
            lpfnWndProc: Some(site_editor_wndproc),
            hInstance: hinstance,
            hIcon: window_icon.unwrap_or_default(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap(),
            hbrBackground: background_brush,
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        let _ = RegisterClassW(&wnd_class);

        let text_container_class = WNDCLASSW {
            lpfnWndProc: Some(site_text_container_wndproc),
            hInstance: hinstance,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap(),
            hbrBackground: background_brush,
            lpszClassName: PCWSTR(text_container_class_name.as_ptr()),
            ..Default::default()
        };
        let _ = RegisterClassW(&text_container_class);

        let window_ex_style: WINDOW_EX_STYLE = Default::default();
        let window_style = WS_OVERLAPPEDWINDOW & !WS_THICKFRAME & !WS_MAXIMIZEBOX;
        let (window_width, window_height) = adjusted_window_size(
            window_style,
            window_ex_style,
            MAIN_WINDOW_CLIENT_WIDTH,
            MAIN_WINDOW_CLIENT_HEIGHT,
        );

        let hwnd = CreateWindowExW(
            window_ex_style,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title_text.as_ptr()),
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
            let _ = tx.send(None);
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

        let container_hwnd = CreateWindowExW(
            Default::default(),
            PCWSTR(text_container_class_name.as_ptr()),
            PCWSTR::null(),
            WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0),
            PROCESS_EDITOR_PADDING,
            PROCESS_EDITOR_PADDING,
            1,
            1,
            hwnd,
            HMENU(1),
            hinstance,
            None,
        );

        let list_style = WINDOW_STYLE(
            WS_CHILD.0
                | WS_VISIBLE.0
                | WS_VSCROLL.0
                | 0x0004
                | 0x0040
                | 0x1000,
        );
        let list_hwnd = CreateWindowExW(
            Default::default(),
            w!("EDIT"),
            PCWSTR(initial_text_wide.as_ptr()),
            list_style,
            0,
            0,
            1,
            1,
            container_hwnd,
            HMENU(1),
            hinstance,
            None,
        );

        let save_style = WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | BS_OWNERDRAW as u32);
        let save_hwnd = CreateWindowExW(
            Default::default(),
            w!("BUTTON"),
            PCWSTR(save_text.as_ptr()),
            save_style,
            PROCESS_EDITOR_PADDING,
            PROCESS_EDITOR_PADDING,
            1,
            PROCESS_SAVE_BUTTON_HEIGHT,
            hwnd,
            HMENU(2),
            hinstance,
            None,
        );

        if let Some(button_font) = ui_font {
            apply_smooth_font(list_hwnd, button_font);
        }
        if let Some(button_font) = button_font.or(ui_font) {
            apply_smooth_font(save_hwnd, button_font);
        }

        let state = Box::new(SiteEditorState {
            tx: Some(tx),
            container_hwnd,
            list_hwnd,
            save_hwnd,
            save_label,
            save_hovered: false,
            scrollbar_dragging: false,
            scrollbar_drag_offset: 0,
            list_original_wndproc: 0,
            ui_font,
            button_font,
            window_icon,
        });
        let state_ptr = Box::into_raw(state);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);
        let _ = SetWindowLongPtrW(container_hwnd, GWLP_USERDATA, hwnd.0);
        let _ = SetWindowLongPtrW(list_hwnd, GWLP_USERDATA, hwnd.0);
        let original_wndproc = SetWindowLongPtrW(
            list_hwnd,
            GWLP_WNDPROC,
            site_editor_edit_wndproc as *const () as isize,
        );
        (*state_ptr).list_original_wndproc = original_wndproc;
        layout_site_editor_controls(hwnd, &mut *state_ptr);

        ShowWindow(hwnd, SW_SHOW);
        let _ = SetFocus(list_hwnd);
        let _ = InvalidateRect(container_hwnd, None, true);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
    rx
}

unsafe fn layout_site_editor_controls(hwnd: HWND, state: &mut SiteEditorState) {
    let mut client_rect = RECT::default();
    let _ = GetClientRect(hwnd, &mut client_rect);

    let client_width = (client_rect.right - client_rect.left).max(PROCESS_EDITOR_PADDING * 2 + 1);
    let client_height = (client_rect.bottom - client_rect.top)
        .max(PROCESS_EDITOR_PADDING * 2 + PROCESS_SAVE_BUTTON_HEIGHT + PROCESS_EDITOR_GAP + 1);
    let content_width = (client_width - PROCESS_EDITOR_PADDING * 2).max(1);
    let save_y =
        (client_height - PROCESS_EDITOR_PADDING - PROCESS_SAVE_BUTTON_HEIGHT).max(PROCESS_EDITOR_PADDING);
    let container_height = (save_y - PROCESS_EDITOR_GAP - PROCESS_EDITOR_PADDING).max(SITE_TEXT_LINE_HEIGHT);
    let container_width = content_width;

    let _ = MoveWindow(
        state.container_hwnd,
        PROCESS_EDITOR_PADDING,
        PROCESS_EDITOR_PADDING,
        container_width,
        container_height,
        BOOL(1),
    );
    let line_height = measure_site_editor_line_height(state.list_hwnd, state.ui_font);
    let inner_padding = (line_height / 3).clamp(6, 12);
    let scrollbar_gap = 6;
    let text_width = (container_width - inner_padding * 2 - SITE_SCROLLBAR_WIDTH - scrollbar_gap).max(1);
    let text_height = (container_height - inner_padding * 2).max(line_height * 2);
    let _ = MoveWindow(
        state.list_hwnd,
        inner_padding,
        inner_padding,
        text_width,
        text_height,
        BOOL(1),
    );
    let _ = ShowScrollBar(state.list_hwnd, SCROLLBAR_CONSTANTS(0), BOOL(0));
    let _ = ShowScrollBar(state.list_hwnd, SCROLLBAR_CONSTANTS(1), BOOL(0));
    let _ = MoveWindow(
        state.save_hwnd,
        PROCESS_EDITOR_PADDING,
        save_y,
        content_width,
        PROCESS_SAVE_BUTTON_HEIGHT,
        BOOL(1),
    );
}

unsafe fn measure_site_editor_line_height(edit_hwnd: HWND, ui_font: Option<HFONT>) -> i32 {
    let hdc = GetDC(edit_hwnd);
    if hdc.0 == 0 {
        return SITE_TEXT_LINE_HEIGHT;
    }

    let old_font = ui_font.map(|font| SelectObject(hdc, font));
    let mut text_metrics = TEXTMETRICW::default();
    let measured_height = if GetTextMetricsW(hdc, &mut text_metrics).as_bool() {
        (text_metrics.tmHeight + text_metrics.tmExternalLeading).max(1)
    } else {
        SITE_TEXT_LINE_HEIGHT
    };
    if let Some(old_font) = old_font {
        let _ = SelectObject(hdc, old_font);
    }
    let _ = ReleaseDC(edit_hwnd, hdc);
    measured_height.clamp(8, 64)
}

unsafe fn site_editor_scrollbar_rect(state: &SiteEditorState) -> RECT {
    let mut container_rect = RECT::default();
    let _ = GetClientRect(state.container_hwnd, &mut container_rect);
    let line_height = measure_site_editor_line_height(state.list_hwnd, state.ui_font);
    let inner_padding = (line_height / 3).clamp(6, 12);
    let top = inner_padding;
    let bottom = (container_rect.bottom - inner_padding).max(top + 1);
    let right = (container_rect.right - inner_padding).max(SITE_SCROLLBAR_WIDTH);
    let left = (right - SITE_SCROLLBAR_WIDTH).max(0);
    RECT {
        left,
        top,
        right,
        bottom,
    }
}

unsafe fn site_editor_visible_line_capacity(state: &SiteEditorState) -> i32 {
    let mut edit_rect = RECT::default();
    let _ = GetClientRect(state.list_hwnd, &mut edit_rect);
    let line_height = measure_site_editor_line_height(state.list_hwnd, state.ui_font).max(1);
    ((edit_rect.bottom - edit_rect.top) / line_height).max(1)
}

unsafe fn site_editor_max_scroll(state: &SiteEditorState) -> i32 {
    let total_lines = SendMessageW(state.list_hwnd, EM_GETLINECOUNT, WPARAM(0), LPARAM(0)).0 as i32;
    (total_lines - site_editor_visible_line_capacity(state)).max(0)
}

unsafe fn site_editor_thumb_rect(state: &SiteEditorState) -> RECT {
    let track_rect = site_editor_scrollbar_rect(state);
    let track_height = (track_rect.bottom - track_rect.top).max(1);
    let total_lines =
        (SendMessageW(state.list_hwnd, EM_GETLINECOUNT, WPARAM(0), LPARAM(0)).0 as i32).max(1);
    let visible_lines = site_editor_visible_line_capacity(state).min(total_lines).max(1);
    let max_scroll = site_editor_max_scroll(state);
    let first_visible =
        (SendMessageW(state.list_hwnd, EM_GETFIRSTVISIBLELINE, WPARAM(0), LPARAM(0)).0 as i32)
            .clamp(0, max_scroll);
    let thumb_height = if max_scroll == 0 {
        track_height
    } else {
        ((visible_lines * track_height) / total_lines).clamp(24, track_height)
    };
    let travel = (track_height - thumb_height).max(0);
    let thumb_top = if max_scroll == 0 {
        track_rect.top
    } else {
        track_rect.top + (first_visible * travel) / max_scroll.max(1)
    };

    RECT {
        left: track_rect.left,
        top: thumb_top,
        right: track_rect.right,
        bottom: thumb_top + thumb_height,
    }
}

unsafe fn scroll_site_editor_to_line(state: &mut SiteEditorState, target_first_line: i32) {
    let current =
        SendMessageW(state.list_hwnd, EM_GETFIRSTVISIBLELINE, WPARAM(0), LPARAM(0)).0 as i32;
    let max_scroll = site_editor_max_scroll(state);
    let target = target_first_line.clamp(0, max_scroll);
    let delta = target - current;
    if delta != 0 {
        let _ = SendMessageW(state.list_hwnd, EM_LINESCROLL, WPARAM(0), LPARAM(delta as isize));
    }
    let _ = InvalidateRect(state.container_hwnd, None, false);
}

unsafe fn scroll_site_editor_by_wheel(state: &mut SiteEditorState, wheel_delta: i32) -> bool {
    if wheel_delta == 0 {
        return false;
    }

    let step = ((wheel_delta.abs() / 120).max(1)) * SITE_WHEEL_STEP;
    let current =
        SendMessageW(state.list_hwnd, EM_GETFIRSTVISIBLELINE, WPARAM(0), LPARAM(0)).0 as i32;
    let target = if wheel_delta > 0 {
        current.saturating_sub(step)
    } else {
        current.saturating_add(step)
    };
    scroll_site_editor_to_line(state, target);
    true
}

unsafe fn paint_site_editor_scrollbar(state: &SiteEditorState, hdc: HDC) {
    let track_rect = site_editor_scrollbar_rect(state);
    let thumb_rect = site_editor_thumb_rect(state);

    let track_color = grayscale_color(51);
    let thumb_color = grayscale_color(255);

    let track_brush = CreateSolidBrush(track_color);
    let track_pen = CreatePen(PS_SOLID, 1, track_color);
    let old_track_brush = SelectObject(hdc, track_brush);
    let old_track_pen = SelectObject(hdc, track_pen);
    RoundRect(
        hdc,
        track_rect.left,
        track_rect.top,
        track_rect.right,
        track_rect.bottom,
        SITE_SCROLLBAR_WIDTH,
        SITE_SCROLLBAR_WIDTH,
    );
    let _ = SelectObject(hdc, old_track_brush);
    let _ = SelectObject(hdc, old_track_pen);
    let _ = DeleteObject(track_brush);
    let _ = DeleteObject(track_pen);

    let thumb_brush = CreateSolidBrush(thumb_color);
    let thumb_pen = CreatePen(PS_SOLID, 1, thumb_color);
    let old_thumb_brush = SelectObject(hdc, thumb_brush);
    let old_thumb_pen = SelectObject(hdc, thumb_pen);
    RoundRect(
        hdc,
        thumb_rect.left,
        thumb_rect.top,
        thumb_rect.right,
        thumb_rect.bottom,
        SITE_SCROLLBAR_WIDTH,
        SITE_SCROLLBAR_WIDTH,
    );
    let _ = SelectObject(hdc, old_thumb_brush);
    let _ = SelectObject(hdc, old_thumb_pen);
    let _ = DeleteObject(thumb_brush);
    let _ = DeleteObject(thumb_pen);
}

unsafe extern "system" fn site_editor_edit_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let parent_hwnd = HWND(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as isize);
    if parent_hwnd.0 != 0 {
        let state_ptr = GetWindowLongPtrW(parent_hwnd, GWLP_USERDATA) as *mut SiteEditorState;
        if !state_ptr.is_null() {
            let state = &mut *state_ptr;
            if msg == WM_MOUSEWHEEL {
                let wheel_delta = ((wparam.0 >> 16) as u16) as i16 as i32;
                if scroll_site_editor_by_wheel(state, wheel_delta) {
                    return LRESULT(0);
                }
            }

            if state.list_original_wndproc != 0 {
                let original_wndproc: WNDPROC = std::mem::transmute(state.list_original_wndproc);
                return CallWindowProcW(original_wndproc, hwnd, msg, wparam, lparam);
            }
        }
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}

unsafe fn send_text_and_close(hwnd: HWND) {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SiteEditorState;
    if ptr.is_null() {
        DestroyWindow(hwnd);
        return;
    }
    let state = &mut *ptr;
    if let Some(tx) = state.tx.take() {
        let text_len = SendMessageW(state.list_hwnd, WM_GETTEXTLENGTH, WPARAM(0), LPARAM(0)).0 as usize;
        let mut buffer = vec![0u16; text_len + 1];
        let copied = SendMessageW(
            state.list_hwnd,
            WM_GETTEXT,
            WPARAM(buffer.len()),
            LPARAM(buffer.as_mut_ptr() as isize),
        )
        .0 as usize;
        let content = OsString::from_wide(&buffer[..copied])
            .into_string()
            .unwrap_or_default();
        let _ = tx.send(Some(content));
    }
    DestroyWindow(hwnd);
}

unsafe fn draw_site_save_button(hwnd: HWND, draw_item: &DRAWITEMSTRUCT) {
    if (draw_item.itemState.0 & ODS_SELECTED.0) != 0 {
        if let Ok(cursor) = LoadCursorW(None, IDC_ARROW) {
            let _ = SetCursor(cursor);
        }
    }

    let is_hovered = {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SiteEditorState;
        !ptr.is_null() && (*ptr).save_hovered
    };
    let fill_color = if (draw_item.itemState.0 & ODS_DISABLED.0) != 0 {
        grayscale_color(128)
    } else if (draw_item.itemState.0 & ODS_SELECTED.0) != 0 {
        grayscale_color(128)
    } else if is_hovered {
        grayscale_color(204)
    } else {
        grayscale_color(255)
    };

    let brush = CreateSolidBrush(fill_color);
    let pen = CreatePen(PS_SOLID, 1, fill_color);
    let old_brush = SelectObject(draw_item.hDC, brush);
    let old_pen = SelectObject(draw_item.hDC, pen);

    RoundRect(
        draw_item.hDC,
        draw_item.rcItem.left,
        draw_item.rcItem.top,
        draw_item.rcItem.right,
        draw_item.rcItem.bottom,
        12,
        12,
    );

    let _ = SetBkMode(draw_item.hDC, TRANSPARENT);
    let _ = SetTextColor(draw_item.hDC, COLORREF(0x000000));
    let old_font = {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SiteEditorState;
        if ptr.is_null() {
            None
        } else {
            (*ptr)
                .button_font
                .or((*ptr).ui_font)
                .map(|font| SelectObject(draw_item.hDC, font))
        }
    };

    let mut text_rect = RECT {
        left: draw_item.rcItem.left + 8,
        top: draw_item.rcItem.top,
        right: draw_item.rcItem.right - 8,
        bottom: draw_item.rcItem.bottom,
    };
    let save_label = {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SiteEditorState;
        if ptr.is_null() {
            "Сохранить".to_string()
        } else {
            (*ptr).save_label.clone()
        }
    };
    let mut text = to_wide(&save_label);
    let _ = DrawTextW(
        draw_item.hDC,
        &mut text,
        &mut text_rect,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );

    if let Some(old_font) = old_font {
        let _ = SelectObject(draw_item.hDC, old_font);
    }
    let _ = SelectObject(draw_item.hDC, old_brush);
    let _ = SelectObject(draw_item.hDC, old_pen);
    let _ = DeleteObject(brush);
    let _ = DeleteObject(pen);
}

unsafe extern "system" fn site_text_container_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let parent_hwnd = HWND(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as isize);
    if parent_hwnd.0 != 0 {
        let ptr = GetWindowLongPtrW(parent_hwnd, GWLP_USERDATA) as *mut SiteEditorState;
        if !ptr.is_null() {
            let state = &mut *ptr;
            match msg {
                WM_COMMAND => {
                    return SendMessageW(parent_hwnd, WM_COMMAND, wparam, lparam);
                }
                WM_CTLCOLOREDIT => {
                    let hdc = HDC(wparam.0 as isize);
                    SetTextColor(hdc, COLORREF(0x00FFFFFF));
                    SetBkColor(hdc, COLORREF(0x000000));
                    let brush = GetClassLongPtrW(hwnd, GCLP_HBRBACKGROUND);
                    return LRESULT(brush as isize);
                }
                WM_PAINT => {
                    let mut paint_struct = PAINTSTRUCT::default();
                    let hdc = BeginPaint(hwnd, &mut paint_struct);
                    let mut client_rect = RECT::default();
                    let _ = GetClientRect(hwnd, &mut client_rect);
                    let brush = CreateSolidBrush(COLORREF(0x000000));
                    let pen = CreatePen(PS_SOLID, 1, COLORREF(0x000000));
                    let old_brush = SelectObject(hdc, brush);
                    let old_pen = SelectObject(hdc, pen);
                    RoundRect(
                        hdc,
                        client_rect.left,
                        client_rect.top,
                        client_rect.right,
                        client_rect.bottom,
                        8,
                        8,
                    );
                    let _ = SelectObject(hdc, old_brush);
                    let _ = SelectObject(hdc, old_pen);
                    let _ = DeleteObject(brush);
                    let _ = DeleteObject(pen);
                    paint_site_editor_scrollbar(state, hdc);
                    let _ = EndPaint(hwnd, &paint_struct);
                    return LRESULT(0);
                }
                WM_ERASEBKGND => {
                    return LRESULT(1);
                }
                WM_MOUSEWHEEL => {
                    let wheel_delta = ((wparam.0 >> 16) as u16) as i16 as i32;
                    if scroll_site_editor_by_wheel(state, wheel_delta) {
                        let _ = InvalidateRect(hwnd, None, false);
                        return LRESULT(0);
                    }
                }
                WM_LBUTTONDOWN => {
                    let (x, y) = mouse_point_from_lparam(lparam);
                    let thumb_rect = site_editor_thumb_rect(state);
                    let track_rect = site_editor_scrollbar_rect(state);
                    if rect_contains_point(&thumb_rect, x, y) {
                        state.scrollbar_dragging = true;
                        state.scrollbar_drag_offset = y - thumb_rect.top;
                        SetCapture(hwnd);
                        return LRESULT(0);
                    }
                    if rect_contains_point(&track_rect, x, y) {
                        let thumb_height = (thumb_rect.bottom - thumb_rect.top).max(1);
                        let travel = ((track_rect.bottom - track_rect.top) - thumb_height).max(0);
                        if travel > 0 {
                            let thumb_top =
                                (y - thumb_height / 2).clamp(track_rect.top, track_rect.bottom - thumb_height);
                            let max_scroll = site_editor_max_scroll(state);
                            let target = ((thumb_top - track_rect.top) * max_scroll) / travel;
                            scroll_site_editor_to_line(state, target);
                        }
                        return LRESULT(0);
                    }
                    let _ = SetFocus(state.list_hwnd);
                    return LRESULT(0);
                }
                WM_MOUSEMOVE => {
                    if state.scrollbar_dragging {
                        let (_, y) = mouse_point_from_lparam(lparam);
                        let track_rect = site_editor_scrollbar_rect(state);
                        let thumb_rect = site_editor_thumb_rect(state);
                        let thumb_height = (thumb_rect.bottom - thumb_rect.top).max(1);
                        let travel = ((track_rect.bottom - track_rect.top) - thumb_height).max(0);
                        if travel > 0 {
                            let thumb_top =
                                (y - state.scrollbar_drag_offset).clamp(track_rect.top, track_rect.bottom - thumb_height);
                            let max_scroll = site_editor_max_scroll(state);
                            let target = ((thumb_top - track_rect.top) * max_scroll) / travel;
                            scroll_site_editor_to_line(state, target);
                        }
                        return LRESULT(0);
                    }
                }
                WM_LBUTTONUP => {
                    if state.scrollbar_dragging {
                        state.scrollbar_dragging = false;
                        ReleaseCapture();
                        return LRESULT(0);
                    }
                }
                WM_SETFOCUS => {
                    let _ = SetFocus(state.list_hwnd);
                    return LRESULT(0);
                }
                _ => {}
            }
        }
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}

unsafe extern "system" fn site_editor_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let id = (wparam.0 & 0xffff) as i32;
            let notification = ((wparam.0 >> 16) & 0xffff) as usize;
            if id == 2 {
                send_text_and_close(hwnd);
            }
            if id == 1 && (notification == 0x0300 || notification == 0x0400 || notification == 0x0602) {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SiteEditorState;
                if !ptr.is_null() {
                    let state = &mut *ptr;
                    let _ = InvalidateRect(state.container_hwnd, None, false);
                }
            }
            LRESULT(0)
        }
        WM_DRAWITEM => {
            let draw_item = &*(lparam.0 as *const DRAWITEMSTRUCT);
            if draw_item.CtlID == 2 {
                draw_site_save_button(hwnd, draw_item);
                return LRESULT(1);
            }
            LRESULT(0)
        }
        WM_CTLCOLORBTN => {
            let hdc = HDC(wparam.0 as isize);
            SetTextColor(hdc, COLORREF(0x00FFFFFF));
            SetBkColor(hdc, COLORREF(0x000000));
            let brush = GetClassLongPtrW(hwnd, GCLP_HBRBACKGROUND);
            LRESULT(brush as isize)
        }
        WM_SIZE => {
            if wparam.0 as u32 == SIZE_MINIMIZED {
                return LRESULT(0);
            }

            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SiteEditorState;
            if !ptr.is_null() {
                let state = &mut *ptr;
                layout_site_editor_controls(hwnd, state);
                let _ = InvalidateRect(state.container_hwnd, None, true);
                let _ = InvalidateRect(state.list_hwnd, None, true);
                let _ = InvalidateRect(state.save_hwnd, None, true);
            }
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SiteEditorState;
            if !ptr.is_null() {
                let state = &mut *ptr;
                let wheel_delta = ((wparam.0 >> 16) as u16) as i16 as i32;
                if scroll_site_editor_by_wheel(state, wheel_delta) {
                    let _ = InvalidateRect(state.container_hwnd, None, false);
                    return LRESULT(0);
                }
            }
            LRESULT(0)
        }
        WM_SETCURSOR => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SiteEditorState;
            if !ptr.is_null() {
                let state = &mut *ptr;
                let hovered_save = HWND(wparam.0 as isize) == state.save_hwnd;
                if state.save_hovered != hovered_save {
                    state.save_hovered = hovered_save;
                    let _ = InvalidateRect(state.save_hwnd, None, false);
                }
                if hovered_save {
                    let cursor = if GetKeyState(VK_LBUTTON.0 as i32) < 0 {
                        LoadCursorW(None, IDC_ARROW)
                    } else {
                        LoadCursorW(None, IDC_HAND)
                    };
                    if let Ok(cursor) = cursor {
                        let _ = SetCursor(cursor);
                        return LRESULT(1);
                    }
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut SiteEditorState;
            if !ptr.is_null() {
                let mut state = Box::from_raw(ptr);
                if let Some(tx) = state.tx.take() {
                    let _ = tx.send(None);
                }
                if let Some(ui_font) = state.ui_font.take() {
                    let _ = DeleteObject(ui_font);
                }
                if let Some(button_font) = state.button_font.take() {
                    let _ = DeleteObject(button_font);
                }
                if let Some(window_icon) = state.window_icon.take() {
                    let _ = DestroyIcon(window_icon);
                }
            }
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}