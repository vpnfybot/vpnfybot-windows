use super::*;

pub(super) fn button_font_id() -> egui::FontId {
    egui::FontId::new(
        UI_BUTTON_FONT_SIZE,
        egui::FontFamily::Name(BUTTON_FONT_FAMILY_NAME.into()),
    )
}

#[allow(dead_code)]
pub(super) fn button_font_small_id() -> egui::FontId {
    egui::FontId::new(
        UI_BUTTON_FONT_SIZE - 4.0,
        egui::FontFamily::Name(BUTTON_FONT_FAMILY_NAME.into()),
    )
}

fn load_windows_button_font_bytes() -> Option<Vec<u8>> {
    let windows_dir = env::var_os("WINDIR").unwrap_or_else(|| OsString::from("C:\\Windows"));
    let primary_path = PathBuf::from(&windows_dir).join("Fonts").join("segoeui.ttf");
    fs::read(primary_path).ok()
}

pub(super) fn configure_egui_button_font(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let button_family = egui::FontFamily::Name(BUTTON_FONT_FAMILY_NAME.into());
    let mut button_fonts = fonts
        .families
        .get(&egui::FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();

    if let Some(font_bytes) = load_windows_button_font_bytes() {
        let font_name = "vpnfy_button_font_segoe_ui".to_string();
        fonts
            .font_data
            .insert(font_name.clone(), egui::FontData::from_owned(font_bytes).into());
        button_fonts.insert(0, font_name);
    }

    fonts.families.insert(button_family, button_fonts);
    ctx.set_fonts(fonts);

    let mut style = (*ctx.style()).clone();
    style
        .text_styles
        .insert(egui::TextStyle::Button, button_font_id());
    ctx.set_style(style);
}

pub(super) fn xml_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn notification_icon_path() -> Option<PathBuf> {
    let icon_bytes = include_bytes!("../../../src/gifs/vpnfy.png");
    let icon_path = super::managed_cache_dir().join("vpnfy-notification-icon.png");
    if let Some(parent) = icon_path.parent() {
        fs::create_dir_all(parent).ok()?;
    }

    let needs_write = match fs::read(&icon_path) {
        Ok(existing) => existing.as_slice() != &icon_bytes[..],
        Err(_) => true,
    };

    if needs_write {
        fs::write(&icon_path, icon_bytes).ok()?;
    }

    Some(icon_path)
}

fn file_uri_from_path(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let mut uri = String::from("file:///");
    for byte in normalized.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b':' | b'-' | b'_' | b'.' | b'~' => {
                uri.push(byte as char);
            }
            _ => uri.push_str(&format!("%{:02X}", byte)),
        }
    }
    uri
}

pub(super) fn notification_icon_uri() -> Option<String> {
    let path = notification_icon_path()?;
    Some(file_uri_from_path(&path))
}

pub(super) fn load_texture(ctx: &egui::Context, id: &str, bytes: &[u8]) -> egui::TextureHandle {
    let image = image::load_from_memory(bytes).expect("failed to load embedded image");
    let image = image.to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let pixels = image.into_vec();
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
    ctx.load_texture(id, color_image, Default::default())
}

pub(super) fn load_svg_texture(ctx: &egui::Context, id: &str, bytes: &[u8]) -> Option<egui::TextureHandle> {
    let svg_source = std::str::from_utf8(bytes).ok()?.replace("currentColor", "#ffffff");
    let options = usvg::Options::default();
    let tree = usvg::Tree::from_str(&svg_source, &options).ok()?;
    let size = tree.size().to_int_size();
    let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height())?;
    let mut pixmap_mut = pixmap.as_mut();
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap_mut);
    let color_image = egui::ColorImage::from_rgba_unmultiplied(
        [size.width() as usize, size.height() as usize],
        pixmap.data(),
    );
    Some(ctx.load_texture(id, color_image, Default::default()))
}

pub(super) fn load_gif_frames(ctx: &egui::Context, bytes: &[u8]) -> image::ImageResult<(Vec<egui::TextureHandle>, Vec<u64>)> {
    let decoder = GifDecoder::new(Cursor::new(bytes))?;
    let frames = decoder.into_frames();
    let frames = frames.collect_frames()?;
    let mut textures = Vec::new();
    let mut durations = Vec::new();

    for (index, frame) in frames.into_iter().enumerate() {
        let delay = frame.delay().numer_denom_ms().0 as u64;
        let buffer = frame.into_buffer();
        let size = [buffer.width() as usize, buffer.height() as usize];
        let pixels = buffer.into_vec();
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
        textures.push(ctx.load_texture(&format!("animated_frame_{}", index), color_image, Default::default()));
        durations.push(delay);
    }

    Ok((textures, durations))
}

#[cfg(target_os = "windows")]
pub(super) fn win_text_to_texture(
    ctx: &egui::Context,
    id: &str,
    text: &str,
    hfont: Option<HFONT>,
    color: egui::Color32,
    width: usize,
    height: usize,
) -> Option<egui::TextureHandle> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Foundation::HWND;

    unsafe {
        if width == 0 || height == 0 {
            return None;
        }

        let screen_dc = GetDC(HWND(0));
        if screen_dc.0 == 0 {
            return None;
        }

        let mem_dc = CreateCompatibleDC(screen_dc);
        if mem_dc.0 == 0 {
            let _ = ReleaseDC(HWND(0), screen_dc);
            return None;
        }

        let hbmp = CreateCompatibleBitmap(screen_dc, width as i32, height as i32);
        if hbmp.0 == 0 {
            let _ = DeleteDC(mem_dc);
            let _ = ReleaseDC(HWND(0), screen_dc);
            return None;
        }

        let old_bmp = SelectObject(mem_dc, hbmp);

        let brush = CreateSolidBrush(COLORREF(0x00FFFFFF));
        let clear_rect = RECT { left: 0, top: 0, right: width as i32, bottom: height as i32 };
        let _ = FillRect(mem_dc, &clear_rect, brush);
        let _ = DeleteObject(brush);

        let old_font = if let Some(f) = hfont { Some(SelectObject(mem_dc, f)) } else { None };

        let _ = SetTextColor(mem_dc, COLORREF(0x000000));
        let _ = SetBkMode(mem_dc, TRANSPARENT);

        let mut wtext: Vec<u16> = OsStr::new(text).encode_wide().chain(Some(0)).collect();
        let mut rect = RECT { left: 0, top: 0, right: width as i32, bottom: height as i32 };
        let _ = DrawTextW(
            mem_dc,
            &mut wtext[..],
            &mut rect,
            DT_SINGLELINE | DT_CENTER | DT_VCENTER,
        );

        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = width as i32;
        bmi.bmiHeader.biHeight = -(height as i32);
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = 0;

        let mut pixels: Vec<u8> = vec![0u8; width * height * 4];
        let lines = GetDIBits(mem_dc, hbmp, 0, height as u32, Some(pixels.as_mut_ptr() as *mut _), &mut bmi, DIB_RGB_COLORS);

        if let Some(of) = old_font {
            let _ = SelectObject(mem_dc, of);
        }
        let _ = SelectObject(mem_dc, old_bmp);
        let _ = DeleteObject(hbmp);
        let _ = DeleteDC(mem_dc);
        let _ = ReleaseDC(HWND(0), screen_dc);

        if lines == 0 {
            return None;
        }

        let desired_r = color.r();
        let desired_g = color.g();
        let desired_b = color.b();
        for i in 0..(width * height) {
            let b = pixels[i * 4] as u32;
            let g = pixels[i * 4 + 1] as u32;
            let r = pixels[i * 4 + 2] as u32;
            let lum = ((r + g + b) / 3) as u8;
            let mask = 255u8.saturating_sub(lum);
            pixels[i * 4] = desired_r;
            pixels[i * 4 + 1] = desired_g;
            pixels[i * 4 + 2] = desired_b;
            pixels[i * 4 + 3] = mask;
        }

        let size = [width as usize, height as usize];
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
        Some(ctx.load_texture(id, color_image, Default::default()))
    }
}

pub(super) fn load_png_icon_handle(png_bytes: &[u8]) -> Option<HICON> {
    let icon_data = from_png_bytes(png_bytes).ok()?;
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

pub(super) fn show_existing_external_editor(window_class: &str) -> bool {
    let class_text = to_wide(window_class);
    unsafe {
        let existing_window = FindWindowW(PCWSTR(class_text.as_ptr()), PCWSTR::null());
        if existing_window.0 != 0 {
            let _ = ShowWindow(existing_window, SW_SHOWNORMAL);
            let _ = ShowWindow(existing_window, SW_RESTORE);
            let _ = SetForegroundWindow(existing_window);
            return true;
        }
    }
    false
}

pub(super) fn external_editor_is_open(window_class: &str) -> bool {
    let class_text = to_wide(window_class);
    unsafe { FindWindowW(PCWSTR(class_text.as_ptr()), PCWSTR::null()).0 != 0 }
}

pub(super) fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

pub(super) fn copy_wide_truncated(dst: &mut [u16], text: &str) {
    let wide = to_wide(text);
    let max_len = dst.len().saturating_sub(1);
    let copy_len = wide.len().saturating_sub(1).min(max_len);
    if copy_len > 0 {
        dst[..copy_len].copy_from_slice(&wide[..copy_len]);
    }
    if !dst.is_empty() {
        dst[copy_len] = 0;
    }
}

pub(super) fn mouse_point_from_lparam(lparam: LPARAM) -> (i32, i32) {
    let raw = lparam.0 as u32;
    let x = (raw & 0xffff) as i16 as i32;
    let y = ((raw >> 16) & 0xffff) as i16 as i32;
    (x, y)
}

pub(super) fn rect_contains_point(rect: &RECT, x: i32, y: i32) -> bool {
    x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom
}

pub(super) fn grayscale_color(level: u8) -> COLORREF {
    let value = level as u32;
    COLORREF(value | (value << 8) | (value << 16))
}

pub(super) fn adjusted_window_size(window_style: WINDOW_STYLE, window_ex_style: WINDOW_EX_STYLE, client_width: i32, client_height: i32) -> (i32, i32) {
    unsafe {
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: client_width,
            bottom: client_height,
        };
        if AdjustWindowRectEx(&mut rect, window_style, false, window_ex_style).as_bool() {
            (rect.right - rect.left, rect.bottom - rect.top)
        } else {
            (client_width, client_height)
        }
    }
}

fn create_smooth_ui_font_with_weight(size_px: i32, weight: i32) -> Option<HFONT> {
    unsafe {
        let font = CreateFontW(-size_px, 0, 0, 0, weight, 0, 0, 0, 1, 0, 0, 5, 0, windows::core::w!("Segoe UI"));
        if font.0 == 0 {
            None
        } else {
            Some(font)
        }
    }
}

#[allow(dead_code)]
fn create_nonsmooth_ui_font_with_weight(size_px: i32, weight: i32) -> Option<HFONT> {
    unsafe {
        let font = CreateFontW(-size_px, 0, 0, 0, weight, 0, 0, 0, 1, 0, 0, 3, 0, windows::core::w!("Segoe UI"));
        if font.0 == 0 {
            None
        } else {
            Some(font)
        }
    }
}

#[allow(dead_code)]
fn create_nonsmooth_ui_font(size_px: i32) -> Option<HFONT> {
    create_nonsmooth_ui_font_with_weight(size_px, 400)
}

#[allow(dead_code)]
fn create_nonsmooth_ui_font_light(size_px: i32) -> Option<HFONT> {
    create_nonsmooth_ui_font_with_weight(size_px, 300)
}

pub(super) fn create_smooth_ui_font(size_px: i32) -> Option<HFONT> {
    create_smooth_ui_font_with_weight(size_px, 400)
}

pub(super) fn current_ui_scale_factor() -> f32 {
    unsafe {
        let screen_hwnd = HWND(0);
        let screen_dc = GetDC(screen_hwnd);
        if screen_dc.0 == 0 {
            return 1.0;
        }

        let dpi_y = GetDeviceCaps(screen_dc, LOGPIXELSY);
        let _ = ReleaseDC(screen_hwnd, screen_dc);

        if dpi_y <= 0 {
            1.0
        } else {
            (dpi_y as f32 / 96.0).max(1.0)
        }
    }
}

pub(super) fn create_button_ui_font() -> Option<HFONT> {
    let scaled_size = (UI_BUTTON_FONT_SIZE * current_ui_scale_factor())
        .round()
        .max(1.0) as i32;
    create_smooth_ui_font(scaled_size)
}

pub(super) fn create_button_ui_font_light() -> Option<HFONT> {
    let scaled_size = (UI_BUTTON_FONT_SIZE * current_ui_scale_factor())
        .round()
        .max(1.0) as i32;
    create_smooth_ui_font_with_weight(scaled_size, 300)
}

pub(super) unsafe fn apply_smooth_font(control: HWND, font: HFONT) {
    let _ = SendMessageW(control, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
}