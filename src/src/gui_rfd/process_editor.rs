use std::collections::BTreeSet;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use windows::core::{PCWSTR, w};
use windows::Win32::Foundation::{BOOL, COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DwmSetWindowAttribute};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreatePen, CreateSolidBrush, DT_CENTER, DT_SINGLELINE, DT_VCENTER,
    DeleteObject, DrawTextW, EndPaint, HDC, HFONT, InvalidateRect, PAINTSTRUCT, PS_SOLID,
    RoundRect, SelectObject, SetBkColor, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{DRAWITEMSTRUCT, ODS_DISABLED, ODS_SELECTED};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, SetFocus, VK_LBUTTON,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BS_OWNERDRAW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DestroyIcon,
    DestroyWindow, DispatchMessageW, GCLP_HBRBACKGROUND, GWLP_USERDATA, GetClassLongPtrW,
    GetClientRect, GetMessageW, GetWindowLongPtrW, HICON, HMENU, IDC_ARROW, IDC_HAND,
    LoadCursorW, MSG, MoveWindow, PostQuitMessage, RegisterClassW, SIZE_MINIMIZED, SW_SHOW,
    SendMessageW, SetCursor, SetWindowLongPtrW, ShowWindow, TranslateMessage,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_COMMAND, WM_CTLCOLORBTN, WM_DESTROY,
    WM_DRAWITEM, WM_ERASEBKGND, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
    WM_MOUSEWHEEL, WM_PAINT, WM_SETCURSOR, WM_SETICON, WM_SIZE, WNDCLASSW, WS_CHILD,
    WS_MAXIMIZEBOX, WS_OVERLAPPEDWINDOW, WS_THICKFRAME, WS_VISIBLE,
};

use super::{
    MAIN_WINDOW_CLIENT_HEIGHT, MAIN_WINDOW_CLIENT_WIDTH, PROCESS_EDITOR_GAP,
    PROCESS_EDITOR_PADDING, PROCESS_LIST_CLASS, PROCESS_LIST_ITEM_HEIGHT,
    PROCESS_LIST_SCROLLBAR_GAP, PROCESS_LIST_WHEEL_STEP, PROCESS_SAVE_BUTTON_HEIGHT,
    PROCESSES_EDITOR_CLASS, SITE_SCROLLBAR_WIDTH, adjusted_window_size,
    apply_smooth_font, create_button_ui_font, create_smooth_ui_font, external_editor_is_open,
    get_running_processes, grayscale_color, load_png_icon_handle, mouse_point_from_lparam,
    rect_contains_point, show_existing_external_editor, to_wide,
};

struct ProcessEditorState {
    tx: Option<Sender<Option<Vec<String>>>>,
    list_hwnd: HWND,
    save_hwnd: HWND,
    save_label: String,
    save_hovered: bool,
    hovered_item: Option<usize>,
    pressed_item: Option<usize>,
    scrollbar_dragging: bool,
    scrollbar_drag_offset: i32,
    scroll_index: usize,
    selected_indices: BTreeSet<usize>,
    ui_font: Option<HFONT>,
    button_font: Option<HFONT>,
    window_icon: Option<HICON>,
    items: Vec<String>,
}

pub(super) fn is_open() -> bool {
    external_editor_is_open(PROCESSES_EDITOR_CLASS)
}

pub(super) fn show_existing() -> bool {
    show_existing_external_editor(PROCESSES_EDITOR_CLASS)
}

pub(super) fn open_external(
    process_items: Vec<String>,
    selected_processes: Vec<String>,
    window_title: String,
    save_label: String,
) -> Receiver<Option<Vec<String>>> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || unsafe {
        let mut process_items = if process_items.is_empty() {
            get_running_processes()
        } else {
            process_items
        };
        process_items.sort();
        process_items.dedup();

        let class_name = to_wide(PROCESSES_EDITOR_CLASS);
        let list_class_name = to_wide(PROCESS_LIST_CLASS);
        let title_text = to_wide(&window_title);
        let save_text = to_wide(&save_label);
        let selected_processes_set: BTreeSet<String> = selected_processes.into_iter().collect();
        let selected_indices: BTreeSet<usize> = process_items
            .iter()
            .enumerate()
            .filter_map(|(index, process_name)| selected_processes_set.contains(process_name).then_some(index))
            .collect();
        let ui_font = create_smooth_ui_font(16);
        let button_font = create_button_ui_font();
        let window_icon = load_png_icon_handle(include_bytes!("../../../src/gifs/vpnfy.png"));

        let hinstance = GetModuleHandleW(None).unwrap();
        let background_brush = CreateSolidBrush(COLORREF(0));
        let list_background_brush = CreateSolidBrush(COLORREF(0));
        let wnd_class = WNDCLASSW {
            lpfnWndProc: Some(process_editor_wndproc),
            hInstance: hinstance,
            hIcon: window_icon.unwrap_or_default(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap(),
            hbrBackground: background_brush,
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        let _ = RegisterClassW(&wnd_class);

        let list_class = WNDCLASSW {
            lpfnWndProc: Some(process_list_wndproc),
            hInstance: hinstance,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap(),
            hbrBackground: list_background_brush,
            lpszClassName: PCWSTR(list_class_name.as_ptr()),
            ..Default::default()
        };
        let _ = RegisterClassW(&list_class);

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

        let list_style = WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0);
        let list_hwnd = CreateWindowExW(
            Default::default(),
            PCWSTR(list_class_name.as_ptr()),
            PCWSTR::null(),
            list_style,
            10,
            10,
            280,
            280,
            hwnd,
            HMENU(1),
            hinstance,
            None,
        );

        let _ = SetWindowLongPtrW(list_hwnd, GWLP_USERDATA, hwnd.0);

        let save_style = WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | BS_OWNERDRAW as u32);
        let save_hwnd = CreateWindowExW(
            Default::default(),
            w!("BUTTON"),
            PCWSTR(save_text.as_ptr()),
            save_style,
            10,
            312,
            280,
            28,
            hwnd,
            HMENU(2),
            hinstance,
            None,
        );

        if let Some(ui_font) = ui_font {
            apply_smooth_font(list_hwnd, ui_font);
        }
        if let Some(button_font) = button_font.or(ui_font) {
            apply_smooth_font(save_hwnd, button_font);
        }

        let state = Box::new(ProcessEditorState {
            tx: Some(tx),
            list_hwnd,
            save_hwnd,
            save_label,
            save_hovered: false,
            hovered_item: None,
            pressed_item: None,
            scrollbar_dragging: false,
            scrollbar_drag_offset: 0,
            scroll_index: 0,
            selected_indices,
            ui_font,
            button_font,
            window_icon,
            items: process_items,
        });
        let state_ptr = Box::into_raw(state);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);
        layout_process_editor_controls(hwnd, &mut *state_ptr);

        ShowWindow(hwnd, SW_SHOW);
        let _ = SetFocus(list_hwnd);
        let _ = InvalidateRect(list_hwnd, None, true);
        let _ = InvalidateRect(hwnd, None, true);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
    rx
}

unsafe fn send_selected_processes_and_close(hwnd: HWND) {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ProcessEditorState;
    if ptr.is_null() {
        DestroyWindow(hwnd);
        return;
    }

    let state = &mut *ptr;
    let mut selected_processes = Vec::new();
    for index in &state.selected_indices {
        if let Some(process_name) = state.items.get(*index) {
            selected_processes.push(process_name.clone());
        }
    }

    if let Some(tx) = state.tx.take() {
        let _ = tx.send(Some(selected_processes));
    }
    DestroyWindow(hwnd);
}

unsafe fn draw_process_save_button(hwnd: HWND, draw_item: &DRAWITEMSTRUCT) {
    if (draw_item.itemState.0 & ODS_SELECTED.0) != 0 {
        if let Ok(cursor) = LoadCursorW(None, IDC_ARROW) {
            let _ = SetCursor(cursor);
        }
    }

    let is_hovered = {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ProcessEditorState;
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
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ProcessEditorState;
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
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ProcessEditorState;
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

unsafe fn process_list_visible_count(hwnd: HWND) -> usize {
    let content_rect = process_list_content_rect(hwnd);
    let height = (content_rect.bottom - content_rect.top).max(PROCESS_LIST_ITEM_HEIGHT);
    (height / PROCESS_LIST_ITEM_HEIGHT).max(1) as usize
}

unsafe fn process_list_max_scroll(hwnd: HWND, state: &ProcessEditorState) -> usize {
    state.items.len().saturating_sub(process_list_visible_count(hwnd))
}

unsafe fn process_list_content_rect(hwnd: HWND) -> RECT {
    let mut client_rect = RECT::default();
    let _ = GetClientRect(hwnd, &mut client_rect);

    let left = PROCESS_EDITOR_PADDING;
    let top = PROCESS_EDITOR_PADDING;
    let right =
        (client_rect.right - PROCESS_EDITOR_PADDING - SITE_SCROLLBAR_WIDTH - PROCESS_LIST_SCROLLBAR_GAP)
            .max(left + 1);
    let bottom = (client_rect.bottom - PROCESS_EDITOR_PADDING).max(top + PROCESS_LIST_ITEM_HEIGHT);

    RECT {
        left,
        top,
        right,
        bottom,
    }
}

unsafe fn process_list_scrollbar_rect(hwnd: HWND) -> RECT {
    let mut client_rect = RECT::default();
    let _ = GetClientRect(hwnd, &mut client_rect);

    let top = PROCESS_EDITOR_PADDING;
    let bottom = (client_rect.bottom - PROCESS_EDITOR_PADDING).max(top + 1);
    let right = (client_rect.right - PROCESS_EDITOR_PADDING).max(SITE_SCROLLBAR_WIDTH);
    let left = (right - SITE_SCROLLBAR_WIDTH).max(0);

    RECT {
        left,
        top,
        right,
        bottom,
    }
}

unsafe fn process_list_thumb_rect(hwnd: HWND, state: &ProcessEditorState) -> RECT {
    let track_rect = process_list_scrollbar_rect(hwnd);
    let track_height = (track_rect.bottom - track_rect.top).max(1);
    let total_items = state.items.len().max(1);
    let visible_items = process_list_visible_count(hwnd).min(total_items).max(1);
    let max_scroll = process_list_max_scroll(hwnd, state);
    let thumb_height = if max_scroll == 0 {
        track_height
    } else {
        (((visible_items as i32) * track_height) / (total_items as i32)).clamp(24, track_height)
    };
    let travel = (track_height - thumb_height).max(0);
    let thumb_top = if max_scroll == 0 {
        track_rect.top
    } else {
        track_rect.top + ((state.scroll_index as i32) * travel) / (max_scroll as i32)
    };

    RECT {
        left: track_rect.left,
        top: thumb_top,
        right: track_rect.right,
        bottom: thumb_top + thumb_height,
    }
}

unsafe fn process_list_scroll_to(hwnd: HWND, state: &mut ProcessEditorState, target: usize) {
    let max_scroll = process_list_max_scroll(hwnd, state);
    let clamped = target.min(max_scroll);
    if state.scroll_index != clamped {
        state.scroll_index = clamped;
        let _ = InvalidateRect(hwnd, None, false);
    }
}

unsafe fn process_list_scroll_by_wheel(hwnd: HWND, state: &mut ProcessEditorState, delta: i32) {
    let step = ((delta.abs() / 120).max(1)) as usize * PROCESS_LIST_WHEEL_STEP;
    let target = if delta > 0 {
        state.scroll_index.saturating_sub(step)
    } else if delta < 0 {
        state.scroll_index.saturating_add(step)
    } else {
        state.scroll_index
    };
    process_list_scroll_to(hwnd, state, target);
}

unsafe fn process_list_item_from_lparam(
    hwnd: HWND,
    state: &ProcessEditorState,
    lparam: LPARAM,
) -> Option<usize> {
    let (x, y) = mouse_point_from_lparam(lparam);

    let content_rect = process_list_content_rect(hwnd);
    if !rect_contains_point(&content_rect, x, y) {
        return None;
    }

    let row = ((y - content_rect.top) / PROCESS_LIST_ITEM_HEIGHT) as usize;
    if row >= process_list_visible_count(hwnd) {
        return None;
    }
    let index = state.scroll_index + row;
    if index < state.items.len() {
        Some(index)
    } else {
        None
    }
}

unsafe fn layout_process_editor_controls(hwnd: HWND, state: &mut ProcessEditorState) {
    let mut client_rect = RECT::default();
    let _ = GetClientRect(hwnd, &mut client_rect);

    let client_width = (client_rect.right - client_rect.left).max(PROCESS_EDITOR_PADDING * 2 + 1);
    let client_height = (client_rect.bottom - client_rect.top)
        .max(PROCESS_EDITOR_PADDING * 2 + PROCESS_SAVE_BUTTON_HEIGHT + PROCESS_EDITOR_GAP + 1);
    let content_width = (client_width - PROCESS_EDITOR_PADDING * 2).max(1);
    let save_y =
        (client_height - PROCESS_EDITOR_PADDING - PROCESS_SAVE_BUTTON_HEIGHT).max(PROCESS_EDITOR_PADDING);
    let list_height = (save_y - PROCESS_EDITOR_GAP - PROCESS_EDITOR_PADDING).max(PROCESS_LIST_ITEM_HEIGHT);

    let _ = MoveWindow(
        state.list_hwnd,
        PROCESS_EDITOR_PADDING,
        PROCESS_EDITOR_PADDING,
        content_width,
        list_height,
        BOOL(1),
    );
    let _ = MoveWindow(
        state.save_hwnd,
        PROCESS_EDITOR_PADDING,
        save_y,
        content_width,
        PROCESS_SAVE_BUTTON_HEIGHT,
        BOOL(1),
    );

    state.scroll_index = state.scroll_index.min(process_list_max_scroll(state.list_hwnd, state));
}

unsafe fn draw_process_list(hwnd: HWND, hdc: HDC) {
    let parent_hwnd = HWND(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as isize);
    if parent_hwnd.0 == 0 {
        return;
    }

    let ptr = GetWindowLongPtrW(parent_hwnd, GWLP_USERDATA) as *mut ProcessEditorState;
    if ptr.is_null() {
        return;
    }

    let state = &*ptr;
    let mut client_rect = RECT::default();
    let _ = GetClientRect(hwnd, &mut client_rect);
    let content_rect = process_list_content_rect(hwnd);
    let scrollbar_rect = process_list_scrollbar_rect(hwnd);
    let thumb_rect = process_list_thumb_rect(hwnd, state);

    let background_brush = CreateSolidBrush(COLORREF(0x000000));
    let background_pen = CreatePen(PS_SOLID, 1, COLORREF(0x000000));
    let old_brush = SelectObject(hdc, background_brush);
    let old_pen = SelectObject(hdc, background_pen);
    let old_font = state.ui_font.map(|font| SelectObject(hdc, font));

    RoundRect(
        hdc,
        client_rect.left,
        client_rect.top,
        client_rect.right,
        client_rect.bottom,
        8,
        8,
    );

    let visible_count = process_list_visible_count(hwnd);
    for row in 0..visible_count {
        let item_index = state.scroll_index + row;
        if item_index >= state.items.len() {
            break;
        }

        let row_top = content_rect.top + row as i32 * PROCESS_LIST_ITEM_HEIGHT;
        let row_bottom = row_top + PROCESS_LIST_ITEM_HEIGHT;
        let is_selected = state.selected_indices.contains(&item_index);
        let is_hovered = state.hovered_item == Some(item_index);
        let is_pressed = state.pressed_item == Some(item_index);

        let fill_level = if is_pressed {
            0
        } else if is_selected {
            255
        } else if is_hovered {
            51
        } else {
            0
        };

        if fill_level > 0 {
            let fill_color = grayscale_color(fill_level as u8);
            let fill_brush = CreateSolidBrush(fill_color);
            let fill_pen = CreatePen(PS_SOLID, 1, fill_color);
            let inner_brush = SelectObject(hdc, fill_brush);
            let inner_pen = SelectObject(hdc, fill_pen);
            RoundRect(
                hdc,
                content_rect.left,
                row_top + 2,
                content_rect.right,
                row_bottom - 2,
                8,
                8,
            );
            let _ = SelectObject(hdc, inner_brush);
            let _ = SelectObject(hdc, inner_pen);
            let _ = DeleteObject(fill_brush);
            let _ = DeleteObject(fill_pen);
        }

        let _ = SetBkMode(hdc, TRANSPARENT);
        let _ = SetBkColor(hdc, COLORREF(0x000000));
        let text_color = if is_selected && !is_pressed {
            COLORREF(0x000000)
        } else {
            COLORREF(0x00FFFFFF)
        };
        let _ = SetTextColor(hdc, text_color);

        let mut text = to_wide(&state.items[item_index]);
        let mut text_rect = RECT {
            left: content_rect.left + 12,
            top: row_top,
            right: content_rect.right - 12,
            bottom: row_bottom,
        };
        let _ = DrawTextW(hdc, &mut text, &mut text_rect, DT_VCENTER | DT_SINGLELINE);
    }

    let track_brush = CreateSolidBrush(grayscale_color(51));
    let track_pen = CreatePen(PS_SOLID, 1, grayscale_color(51));
    let old_track_brush = SelectObject(hdc, track_brush);
    let old_track_pen = SelectObject(hdc, track_pen);
    RoundRect(
        hdc,
        scrollbar_rect.left,
        scrollbar_rect.top,
        scrollbar_rect.right,
        scrollbar_rect.bottom,
        SITE_SCROLLBAR_WIDTH,
        SITE_SCROLLBAR_WIDTH,
    );
    let _ = SelectObject(hdc, old_track_brush);
    let _ = SelectObject(hdc, old_track_pen);
    let _ = DeleteObject(track_brush);
    let _ = DeleteObject(track_pen);

    let thumb_brush = CreateSolidBrush(grayscale_color(255));
    let thumb_pen = CreatePen(PS_SOLID, 1, grayscale_color(255));
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

    let _ = SelectObject(hdc, old_brush);
    let _ = SelectObject(hdc, old_pen);
    if let Some(old_font) = old_font {
        let _ = SelectObject(hdc, old_font);
    }
    let _ = DeleteObject(background_brush);
    let _ = DeleteObject(background_pen);
}

unsafe fn set_process_list_cursor(state: &ProcessEditorState) {
    let cursor_id = if state.scrollbar_dragging || state.pressed_item.is_some() {
        IDC_ARROW
    } else if state.hovered_item.is_some() {
        IDC_HAND
    } else {
        IDC_ARROW
    };
    if let Ok(cursor) = LoadCursorW(None, cursor_id) {
        let _ = SetCursor(cursor);
    }
}

unsafe extern "system" fn process_list_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let parent_hwnd = HWND(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as isize);

    if parent_hwnd.0 != 0 {
        let ptr = GetWindowLongPtrW(parent_hwnd, GWLP_USERDATA) as *mut ProcessEditorState;
        if !ptr.is_null() {
            let state = &mut *ptr;
            match msg {
                WM_PAINT => {
                    let mut paint_struct = PAINTSTRUCT::default();
                    let hdc = BeginPaint(hwnd, &mut paint_struct);
                    draw_process_list(hwnd, hdc);
                    let _ = EndPaint(hwnd, &paint_struct);
                    return LRESULT(0);
                }
                WM_ERASEBKGND => {
                    return LRESULT(1);
                }
                WM_MOUSEMOVE => {
                    if state.scrollbar_dragging {
                        let (_, y) = mouse_point_from_lparam(lparam);
                        let track_rect = process_list_scrollbar_rect(hwnd);
                        let thumb_rect = process_list_thumb_rect(hwnd, state);
                        let thumb_height = (thumb_rect.bottom - thumb_rect.top).max(1);
                        let travel = ((track_rect.bottom - track_rect.top) - thumb_height).max(0);
                        if travel > 0 {
                            let thumb_top =
                                (y - state.scrollbar_drag_offset).clamp(track_rect.top, track_rect.bottom - thumb_height);
                            let max_scroll = process_list_max_scroll(hwnd, state);
                            let target = (((thumb_top - track_rect.top) as usize) * max_scroll) / (travel as usize);
                            process_list_scroll_to(hwnd, state, target);
                        }
                        return LRESULT(0);
                    }

                    let hovered_item = process_list_item_from_lparam(hwnd, state, lparam);
                    if state.hovered_item != hovered_item {
                        state.hovered_item = hovered_item;
                        set_process_list_cursor(state);
                        let _ = InvalidateRect(hwnd, None, false);
                    }
                    return LRESULT(0);
                }
                WM_LBUTTONDOWN => {
                    let (x, y) = mouse_point_from_lparam(lparam);
                    let thumb_rect = process_list_thumb_rect(hwnd, state);
                    let track_rect = process_list_scrollbar_rect(hwnd);
                    if rect_contains_point(&thumb_rect, x, y) {
                        state.scrollbar_dragging = true;
                        state.scrollbar_drag_offset = y - thumb_rect.top;
                        let _ = SetFocus(hwnd);
                        let _ = SetCapture(hwnd);
                        set_process_list_cursor(state);
                        return LRESULT(0);
                    }
                    if rect_contains_point(&track_rect, x, y) {
                        let thumb_height = (thumb_rect.bottom - thumb_rect.top).max(1);
                        let travel = ((track_rect.bottom - track_rect.top) - thumb_height).max(0);
                        if travel > 0 {
                            let thumb_top =
                                (y - thumb_height / 2).clamp(track_rect.top, track_rect.bottom - thumb_height);
                            let max_scroll = process_list_max_scroll(hwnd, state);
                            let target = (((thumb_top - track_rect.top) as usize) * max_scroll) / (travel as usize);
                            process_list_scroll_to(hwnd, state, target);
                        }
                        return LRESULT(0);
                    }

                    state.hovered_item = process_list_item_from_lparam(hwnd, state, lparam);
                    state.pressed_item = state.hovered_item;
                    let _ = SetFocus(hwnd);
                    let _ = SetCapture(hwnd);
                    set_process_list_cursor(state);
                    let _ = InvalidateRect(hwnd, None, false);
                    return LRESULT(0);
                }
                WM_LBUTTONUP => {
                    if state.scrollbar_dragging {
                        state.scrollbar_dragging = false;
                        let _ = ReleaseCapture();
                        set_process_list_cursor(state);
                        let _ = InvalidateRect(hwnd, None, false);
                        return LRESULT(0);
                    }

                    let released_item = process_list_item_from_lparam(hwnd, state, lparam);
                    if let Some(pressed_item) = state.pressed_item {
                        if Some(pressed_item) == released_item {
                            if !state.selected_indices.insert(pressed_item) {
                                state.selected_indices.remove(&pressed_item);
                            }
                        }
                    }
                    state.pressed_item = None;
                    let _ = ReleaseCapture();
                    set_process_list_cursor(state);
                    let _ = InvalidateRect(hwnd, None, false);
                    return LRESULT(0);
                }
                WM_MOUSEWHEEL => {
                    let delta = ((wparam.0 >> 16) as u16) as i16 as i32;
                    process_list_scroll_by_wheel(hwnd, state, delta);
                    return LRESULT(0);
                }
                WM_SIZE => {
                    state.scroll_index = state.scroll_index.min(process_list_max_scroll(hwnd, state));
                    let _ = InvalidateRect(hwnd, None, true);
                    return LRESULT(0);
                }
                WM_SETCURSOR => {
                    set_process_list_cursor(state);
                    return LRESULT(1);
                }
                _ => {}
            }
        }
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}

unsafe extern "system" fn process_editor_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let id = (wparam.0 & 0xffff) as i32;
            if id == 2 {
                send_selected_processes_and_close(hwnd);
            }
            LRESULT(0)
        }
        WM_SIZE => {
            if wparam.0 as u32 == SIZE_MINIMIZED {
                return LRESULT(0);
            }

            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ProcessEditorState;
            if !ptr.is_null() {
                let state = &mut *ptr;
                layout_process_editor_controls(hwnd, state);
                let _ = InvalidateRect(state.list_hwnd, None, true);
                let _ = InvalidateRect(state.save_hwnd, None, true);
            }
            LRESULT(0)
        }
        WM_DRAWITEM => {
            let draw_item = &*(lparam.0 as *const DRAWITEMSTRUCT);
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ProcessEditorState;
            if !ptr.is_null() {
                let state = &*ptr;
                if draw_item.hwndItem == state.save_hwnd {
                    draw_process_save_button(hwnd, draw_item);
                    return LRESULT(1);
                }
            }
            if draw_item.CtlID == 2 {
                draw_process_save_button(hwnd, draw_item);
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
        WM_SETCURSOR => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ProcessEditorState;
            if !ptr.is_null() {
                let state = &mut *ptr;
                let hovered_child = HWND(wparam.0 as isize);
                if hovered_child != state.list_hwnd && (state.hovered_item.is_some() || state.pressed_item.is_some()) {
                    state.hovered_item = None;
                    state.pressed_item = None;
                    let _ = InvalidateRect(state.list_hwnd, None, false);
                }
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
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut ProcessEditorState;
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