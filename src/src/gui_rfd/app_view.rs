use super::*;
use super::app_windows::{open_url, show_error_dialog};

impl App for AppState {
    fn update(&mut self, ctx: &egui::Context, frame: &mut Frame) {
        if ctx.input(|i| i.key_pressed(egui::Key::Num4)) {
            self.reset_app_settings();
        }

        #[cfg(target_os = "windows")]
        {
            if !self.window_frame_styled && self.window_frame_attempts < 10 {
                self.window_frame_attempts += 1;
                self.window_frame_styled = self.apply_black_window_frame(frame);
                if !self.window_frame_styled {
                    ctx.request_repaint_after(Duration::from_millis(250));
                }
            }
        }

        if self.update_pending.is_none() {
            if let Some(mutex) = update_check::UPDATE_AVAILABLE.get() {
                if let Ok(mut guard) = mutex.lock() {
                    if let Some(info) = guard.take() {
                        self.update_pending = Some(info);
                    }
                }
            }
        }

        if let Some(info) = &self.update_pending {
            let info = info.clone();
            let available = ctx.available_rect();
            egui::Area::new("update_modal_full".into())
                .fixed_pos(available.min)
                .show(ctx, |ui| {
                    ui.set_min_size(available.size());
                    let _bg_resp = ui.allocate_rect(available, egui::Sense::click());

                    let overlay_alpha = (0.80_f32 * 255.0_f32).round() as u8;
                    let overlay_color =
                        egui::Color32::from_rgba_unmultiplied(0, 0, 0, overlay_alpha);
                    ui.painter().rect_filled(available, 0.0, overlay_color);

                    let max_content_w = (available.width() - 40.0).max(320.0);
                    let content_w = (available.width() * 0.7).clamp(320.0, max_content_w);
                    let content_h = 260.0_f32;
                    let content_rect = egui::Rect::from_center_size(
                        available.center() + egui::vec2(0.0, 20.0),
                        egui::vec2(content_w, content_h),
                    );
                    let mut content_ui =
                        ui.child_ui(content_rect, egui::Layout::top_down(egui::Align::Center));
                    content_ui.add_space(80.0);

                    let downloading = update_check::UPDATE_DOWNLOADING
                        .get()
                        .map(|a| a.load(std::sync::atomic::Ordering::Relaxed))
                        .unwrap_or(false);
                    let progress_percent = update_check::UPDATE_DOWNLOAD_PROGRESS
                        .get()
                        .map(|p| p.load(std::sync::atomic::Ordering::Relaxed))
                        .unwrap_or(0usize);

                    let button_width = 220.0f32;
                    let bar_size = egui::vec2(button_width, 18.0);

                    let label_text = if downloading {
                        self.language.translate("Загрузка")
                    } else {
                        self.language.translate("Доступна новая версия")
                    };

                    let (bar_rect, _) =
                        content_ui.allocate_exact_size(bar_size, egui::Sense::hover());

                    if downloading {
                        let bar_bg = egui::Color32::from_rgba_unmultiplied(
                            255,
                            255,
                            255,
                            (0.20_f32 * 255.0_f32).round() as u8,
                        );
                        content_ui.painter().rect_filled(bar_rect, 9.0, bar_bg);

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
                            let fill_rect = egui::Rect::from_min_max(
                                bar_rect.min,
                                egui::pos2(bar_rect.min.x + fill_w, bar_rect.max.y),
                            );
                            content_ui
                                .painter()
                                .rect_filled(fill_rect, 9.0, egui::Color32::WHITE);
                        }
                        content_ui.painter().text(
                            bar_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            format!("{}%", progress_percent),
                            egui::FontId::proportional(14.0),
                            egui::Color32::BLACK,
                        );
                    } else {
                        content_ui.painter().text(
                            bar_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            label_text,
                            egui::FontId::proportional(UI_BUTTON_FONT_SIZE),
                            egui::Color32::WHITE,
                        );
                    }

                    content_ui.add_space(14.0);

                    content_ui.vertical_centered(|ui| {
                        let install_size = egui::vec2(220.0, 40.0);
                        let (install_rect, install_resp) =
                            ui.allocate_exact_size(install_size, egui::Sense::click());
                        let install_hover_alpha = if install_resp.is_pointer_button_down_on() {
                            (255f32 * 0.50).round() as u8
                        } else if install_resp.hovered() {
                            (255f32 * 0.80).round() as u8
                        } else {
                            255u8
                        };
                        let enabled = !downloading;
                        let install_alpha = if enabled {
                            install_hover_alpha
                        } else {
                            (255f32 * 0.45).round() as u8
                        };
                        let install_fill =
                            egui::Color32::from_rgba_unmultiplied(220, 220, 220, install_alpha);
                        ui.painter().rect_filled(install_rect, 6.0, install_fill);
                        ui.painter().rect_stroke(
                            install_rect,
                            6.0,
                            egui::Stroke::new(
                                1.0,
                                egui::Color32::from_rgba_unmultiplied(
                                    0,
                                    0,
                                    0,
                                    install_hover_alpha,
                                ),
                            ),
                        );
                        #[cfg(target_os = "windows")]
                        {
                            let label = self.language.translate("Установить");
                            let ppp = ctx.pixels_per_point();
                            let w_px = (install_rect.width() * ppp).ceil() as usize;
                            let h_px = (install_rect.height() * ppp).ceil() as usize;
                            let key = format!("install_button:{}:{}:{}", label, w_px, h_px);
                            let text_color = egui::Color32::from_rgba_unmultiplied(
                                0,
                                0,
                                0,
                                install_hover_alpha,
                            );
                            if let Some(tex) = self.win_text_cache.get(&key) {
                                ui.painter().image(
                                    tex.id(),
                                    install_rect,
                                    egui::Rect::from_min_max(
                                        egui::pos2(0.0, 0.0),
                                        egui::pos2(1.0, 1.0),
                                    ),
                                    egui::Color32::WHITE,
                                );
                            } else if let Some(tex) = win_text_to_texture(
                                ctx,
                                &key,
                                &label,
                                self.button_hfont,
                                text_color,
                                w_px,
                                h_px,
                            ) {
                                self.win_text_cache.insert(key.clone(), tex.clone());
                                ui.painter().image(
                                    tex.id(),
                                    install_rect,
                                    egui::Rect::from_min_max(
                                        egui::pos2(0.0, 0.0),
                                        egui::pos2(1.0, 1.0),
                                    ),
                                    egui::Color32::WHITE,
                                );
                            } else {
                                ui.painter().text(
                                    install_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "Установить",
                                    egui::FontId::proportional(UI_BUTTON_FONT_SIZE),
                                    egui::Color32::BLACK,
                                );
                            }
                        }
                        #[cfg(not(target_os = "windows"))]
                        {
                            let label = self.language.translate("Установить");
                            ui.painter().text(
                                install_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                &label,
                                egui::FontId::proportional(UI_BUTTON_FONT_SIZE),
                                egui::Color32::BLACK,
                            );
                        }
                        if install_resp.is_pointer_button_down_on() {
                            ctx.set_cursor_icon(egui::CursorIcon::Default);
                        } else if install_resp.hovered() {
                            ctx.set_cursor_icon(if !downloading {
                                egui::CursorIcon::PointingHand
                            } else {
                                egui::CursorIcon::NotAllowed
                            });
                        }
                        if install_resp.clicked() && !downloading {
                            let dl_url = info.download_url.clone();
                            let fname = info.asset_name.clone();
                            let progress_atomic = update_check::UPDATE_DOWNLOAD_PROGRESS
                                .get_or_init(|| {
                                    std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0))
                                })
                                .clone();
                            let downloading_flag = update_check::UPDATE_DOWNLOADING
                                .get_or_init(|| {
                                    std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false))
                                })
                                .clone();
                            progress_atomic.store(0, std::sync::atomic::Ordering::Relaxed);
                            downloading_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                            std::thread::spawn(move || {
                                let agent = "vpnfybot-windows-update-install";
                                if let Ok(resp) = ureq::get(&dl_url).set("User-Agent", agent).call()
                                {
                                    let total_opt = resp
                                        .header("Content-Length")
                                        .and_then(|s| s.parse::<usize>().ok());
                                    let exe_dir = std::env::current_exe()
                                        .ok()
                                        .and_then(|p| p.parent().map(|pp| pp.to_path_buf()))
                                        .unwrap_or_else(|| {
                                            std::env::current_dir().unwrap_or(std::env::temp_dir())
                                        });

                                    let asset_stem = std::path::Path::new(&fname)
                                        .file_stem()
                                        .and_then(|s| s.to_str())
                                        .unwrap_or_default()
                                        .to_lowercase();
                                    let cur_no_ext_opt = std::env::current_exe().ok().and_then(|p| {
                                        p.file_name().and_then(|n| n.to_str()).map(|s| {
                                            s.trim_end_matches(".exe").to_string().to_lowercase()
                                        })
                                    });
                                    let replace_candidate = match &cur_no_ext_opt {
                                        Some(cur_no_ext) => {
                                            asset_stem == *cur_no_ext
                                                || asset_stem.contains("vpnfy")
                                                || cur_no_ext.contains(&asset_stem)
                                                || asset_stem.contains(cur_no_ext)
                                        }
                                        None => asset_stem.contains("vpnfy"),
                                    };

                                    let downloaded_basename = if replace_candidate {
                                        "vpnfybot-windows.exe".to_string()
                                    } else {
                                        std::path::Path::new(&fname)
                                            .file_name()
                                            .and_then(|n| n.to_str())
                                            .unwrap_or("update_installer.exe")
                                            .to_string()
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
                                                    if file.write_all(&buf[..n]).is_err() {
                                                        break;
                                                    }
                                                    downloaded += n;
                                                    if let Some(total) = total_opt {
                                                        let pct = ((downloaded as f64 / total as f64)
                                                            * 100.0)
                                                            .round() as usize;
                                                        progress_atomic.store(
                                                            pct.min(100),
                                                            std::sync::atomic::Ordering::Relaxed,
                                                        );
                                                    } else {
                                                        let prev = progress_atomic.load(
                                                            std::sync::atomic::Ordering::Relaxed,
                                                        );
                                                        let next = (prev + 1).min(99);
                                                        progress_atomic.store(
                                                            next,
                                                            std::sync::atomic::Ordering::Relaxed,
                                                        );
                                                    }
                                                }
                                                Err(_) => break,
                                            }
                                        }
                                        progress_atomic
                                            .store(100, std::sync::atomic::Ordering::Relaxed);

                                        #[cfg(target_os = "windows")]
                                        {
                                            let downloaded_path = download_path.clone();
                                            if let Ok(current_exe) = std::env::current_exe() {
                                                let current_name = current_exe
                                                    .file_name()
                                                    .and_then(|n| n.to_str())
                                                    .unwrap_or_default()
                                                    .to_lowercase();
                                                let cur_no_ext =
                                                    current_name.trim_end_matches(".exe").to_string();
                                                let fname_no_ext =
                                                    std::path::Path::new(&downloaded_basename)
                                                        .file_stem()
                                                        .and_then(|s| s.to_str())
                                                        .unwrap_or_default()
                                                        .to_lowercase();

                                                let replace_candidate_after = fname_no_ext
                                                    == cur_no_ext
                                                    || fname_no_ext.contains("vpnfy")
                                                    || cur_no_ext.contains(&fname_no_ext)
                                                    || fname_no_ext.contains(&cur_no_ext);

                                                if replace_candidate_after {
                                                    let script_name = format!(
                                                        "vpnfy_update_{}.ps1",
                                                        std::time::SystemTime::now()
                                                            .duration_since(
                                                                std::time::UNIX_EPOCH,
                                                            )
                                                            .map(|d| d.as_millis())
                                                            .unwrap_or(0u128)
                                                    );
                                                    let script_path =
                                                        std::env::temp_dir().join(&script_name);
                                                    let src = downloaded_path
                                                        .display()
                                                        .to_string()
                                                        .replace("'", "''");
                                                    let dst = current_exe
                                                        .display()
                                                        .to_string()
                                                        .replace("'", "''");
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
    "#,
                                                        src = src,
                                                        dst = dst,
                                                        proc = procname
                                                    );

                                                    let _ = std::fs::write(
                                                        &script_path,
                                                        script.as_bytes(),
                                                    );

                                                    use std::os::windows::process::CommandExt;
                                                    const CREATE_NO_WINDOW: u32 = 0x08000000;
                                                    let mut cmd =
                                                        std::process::Command::new("powershell");
                                                    cmd.args([
                                                        "-NoProfile",
                                                        "-ExecutionPolicy",
                                                        "Bypass",
                                                        "-File",
                                                        script_path
                                                            .to_str()
                                                            .unwrap_or_default(),
                                                    ]);
                                                    cmd.creation_flags(CREATE_NO_WINDOW);
                                                    if cmd.spawn().is_ok() {
                                                        std::process::exit(0);
                                                    } else {
                                                        let _ = std::process::Command::new(
                                                            &downloaded_path,
                                                        )
                                                        .spawn();
                                                        std::process::exit(0);
                                                    }
                                                } else {
                                                    let _ =
                                                        std::process::Command::new(downloaded_path)
                                                            .spawn();
                                                    std::process::exit(0);
                                                }
                                            } else {
                                                let _ = std::process::Command::new(download_path)
                                                    .spawn();
                                                std::process::exit(0);
                                            }
                                        }
                                    }
                                }
                                downloading_flag
                                    .store(false, std::sync::atomic::Ordering::Relaxed);
                            });
                        }

                        ui.add_space(8.0);
                        let later_size = install_size;
                        let (later_rect, later_resp) =
                            ui.allocate_exact_size(later_size, egui::Sense::click());
                        let later_hover_alpha = if later_resp.is_pointer_button_down_on() {
                            (255f32 * 0.50).round() as u8
                        } else if later_resp.hovered() {
                            (255f32 * 0.80).round() as u8
                        } else {
                            255u8
                        };
                        let later_alpha = if enabled {
                            later_hover_alpha
                        } else {
                            (255f32 * 0.45).round() as u8
                        };
                        let later_fill =
                            egui::Color32::from_rgba_unmultiplied(180, 80, 80, later_alpha);
                        ui.painter().rect_filled(later_rect, 6.0, later_fill);
                        ui.painter().rect_stroke(
                            later_rect,
                            6.0,
                            egui::Stroke::new(
                                1.0,
                                egui::Color32::from_rgba_unmultiplied(
                                    0,
                                    0,
                                    0,
                                    later_hover_alpha,
                                ),
                            ),
                        );
                        #[cfg(target_os = "windows")]
                        {
                            let label = self.language.translate("Позже");
                            let ppp = ctx.pixels_per_point();
                            let w_px = (later_rect.width() * ppp).ceil() as usize;
                            let h_px = (later_rect.height() * ppp).ceil() as usize;
                            let key = format!("later_button:{}:{}:{}", label, w_px, h_px);
                            let text_color = egui::Color32::from_rgba_unmultiplied(
                                0,
                                0,
                                0,
                                later_hover_alpha,
                            );
                            if let Some(tex) = self.win_text_cache.get(&key) {
                                ui.painter().image(
                                    tex.id(),
                                    later_rect,
                                    egui::Rect::from_min_max(
                                        egui::pos2(0.0, 0.0),
                                        egui::pos2(1.0, 1.0),
                                    ),
                                    egui::Color32::WHITE,
                                );
                            } else if let Some(tex) = win_text_to_texture(
                                ctx,
                                &key,
                                &label,
                                self.button_hfont,
                                text_color,
                                w_px,
                                h_px,
                            ) {
                                self.win_text_cache.insert(key.clone(), tex.clone());
                                ui.painter().image(
                                    tex.id(),
                                    later_rect,
                                    egui::Rect::from_min_max(
                                        egui::pos2(0.0, 0.0),
                                        egui::pos2(1.0, 1.0),
                                    ),
                                    egui::Color32::WHITE,
                                );
                            } else {
                                ui.painter().text(
                                    later_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    &label,
                                    egui::FontId::proportional(UI_BUTTON_FONT_SIZE),
                                    egui::Color32::BLACK,
                                );
                            }
                        }
                        #[cfg(not(target_os = "windows"))]
                        {
                            let label = self.language.translate("Позже");
                            ui.painter().text(
                                later_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                &label,
                                egui::FontId::proportional(UI_BUTTON_FONT_SIZE),
                                egui::Color32::BLACK,
                            );
                        }
                        if later_resp.is_pointer_button_down_on() {
                            ctx.set_cursor_icon(egui::CursorIcon::Default);
                        } else if later_resp.hovered() {
                            ctx.set_cursor_icon(if !downloading {
                                egui::CursorIcon::PointingHand
                            } else {
                                egui::CursorIcon::NotAllowed
                            });
                        }
                        if later_resp.clicked() && !downloading {
                            self.update_pending = None;
                        }
                    });

                    if update_check::UPDATE_DOWNLOADING
                        .get()
                        .map(|d| d.load(std::sync::atomic::Ordering::Relaxed))
                        .unwrap_or(false)
                    {
                        ctx.request_repaint_after(Duration::from_millis(100));
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
            self.top_image = Some(load_texture(
                ctx,
                "vpnfy",
                include_bytes!("../../gifs/vpnfy.png"),
            ));
            if let Ok((frames, durations)) =
                load_gif_frames(ctx, include_bytes!("../../gifs/animated.gif"))
            {
                self.animated_frames = Some(frames);
                self.animated_frame_durations = durations;
                self.animated_frame_index = 0;
                self.animated_last_frame = Instant::now();
            }
        }

        if self.settings_icon.is_none() {
            self.settings_icon =
                load_svg_texture(ctx, "settings_icon", include_bytes!("../icons/settings.svg"));
        }
        if self.settings_close_icon.is_none() {
            self.settings_close_icon = load_svg_texture(
                ctx,
                "settings_close_icon",
                include_bytes!("../icons/settings-close.svg"),
            );
        }
        if self.language_icon.is_none() {
            self.language_icon = load_svg_texture(
                ctx,
                "language_icon",
                include_bytes!("../icons/language.svg"),
            );
        }
        if self.upload_icon.is_none() {
            self.upload_icon = load_svg_texture(
                ctx,
                "upload_icon",
                include_bytes!("../icons/arrow-up.svg"),
            );
        }
        if self.download_icon.is_none() {
            self.download_icon = load_svg_texture(
                ctx,
                "download_icon",
                include_bytes!("../icons/arrow-down.svg"),
            );
        }

        if let Some(frames) = &self.animated_frames {
            if !frames.is_empty() {
                let frame_delay =
                    Duration::from_millis(self.animated_frame_durations[self.animated_frame_index].max(50));
                if self.animated_last_frame.elapsed() >= frame_delay {
                    self.animated_frame_index = (self.animated_frame_index + 1) % frames.len();
                    self.animated_last_frame = Instant::now();
                }
                let next_frame_in = frame_delay.saturating_sub(self.animated_last_frame.elapsed());
                ctx.request_repaint_after(next_frame_in.max(Duration::from_millis(16)));
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
        let apply_button_cursor = |ctx: &egui::Context,
                                   response: &egui::Response,
                                   enabled: bool| {
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
        if connect_effect_progress > 0.0 && connect_effect_progress < 1.0 {
            is_animating = true;
        }

        let edge_pad = 12.0 / ctx.pixels_per_point();

        egui::Area::new("settings_button".into())
            .anchor(egui::Align2::LEFT_TOP, [edge_pad, edge_pad])
            .movable(false)
            .show(ctx, |ui| {
                let button_size = egui::vec2(26.0, 26.0);
                let (button_rect, response) =
                    ui.allocate_exact_size(button_size, egui::Sense::click());
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
                        let text_color = egui::Color32::from_rgba_unmultiplied(
                            255, 255, 255, icon_alpha,
                        );
                        if let Some(tex) = self.win_text_cache.get(&key) {
                            ui.painter().image(
                                tex.id(),
                                button_rect,
                                egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                ),
                                egui::Color32::WHITE,
                            );
                        } else if let Some(tex) = win_text_to_texture(
                            ctx,
                            &key,
                            "\u{2699}",
                            self.button_hfont,
                            text_color,
                            w_px,
                            h_px,
                        ) {
                            self.win_text_cache.insert(key.clone(), tex.clone());
                            ui.painter().image(
                                tex.id(),
                                button_rect,
                                egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                ),
                                egui::Color32::WHITE,
                            );
                        } else {
                            ui.painter().text(
                                button_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "\u{2699}",
                                egui::FontId::proportional(24.0),
                                egui::Color32::from_rgba_unmultiplied(
                                    255, 255, 255, icon_alpha,
                                ),
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
                    refresh_running_processes_async();
                    self.last_process_refresh = Some(Instant::now());
                }
            });

        egui::Area::new("language_button".into())
            .anchor(egui::Align2::RIGHT_TOP, [-edge_pad, edge_pad])
            .movable(false)
            .show(ctx, |ui| {
                let button_size = egui::vec2(38.0, 26.0);
                let (button_rect, response) =
                    ui.allocate_exact_size(button_size, egui::Sense::click());
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
                    let lang_color =
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, icon_alpha);
                    #[cfg(target_os = "windows")]
                    {
                        let ppp = ctx.pixels_per_point();
                        let w_px = (button_rect.width() * ppp).ceil() as usize;
                        let h_px = (button_rect.height() * ppp).ceil() as usize;
                        let key =
                            format!("language_button:{}:{}:{}", lang_text, w_px, h_px);
                        if let Some(tex) = self.win_text_cache.get(&key) {
                            ui.painter().image(
                                tex.id(),
                                button_rect,
                                egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                ),
                                egui::Color32::WHITE,
                            );
                        } else if let Some(tex) = win_text_to_texture(
                            ctx,
                            &key,
                            lang_text,
                            self.button_hfont,
                            lang_color,
                            w_px,
                            h_px,
                        ) {
                            self.win_text_cache.insert(key.clone(), tex.clone());
                            ui.painter().image(
                                tex.id(),
                                button_rect,
                                egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                ),
                                egui::Color32::WHITE,
                            );
                        } else {
                            let lang_font = button_font.clone();
                            for offset in [
                                egui::vec2(-0.35, 0.0),
                                egui::vec2(0.35, 0.0),
                                egui::Vec2::ZERO,
                            ] {
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
                        for offset in [
                            egui::vec2(-0.35, 0.0),
                            egui::vec2(0.35, 0.0),
                            egui::Vec2::ZERO,
                        ] {
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
                        let (rect, _) =
                            ui.allocate_exact_size(image_base, egui::Sense::hover());
                        let image_center = rect.center() + egui::vec2(0.0, connect_shift);
                        if let Some(frames) = &self.animated_frames {
                            if let Some(frame_texture) = frames.get(self.animated_frame_index) {
                                let gif_rect = egui::Rect::from_center_size(
                                    image_center,
                                    gif_size * pulse_scale,
                                );
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
                        self.language.translate("Импорт").to_owned()
                    };
                    let import_button_enabled = !(self.service_running || self.service_active);
                    let import_button_interactive =
                        import_button_enabled && !controls_locked_by_settings;
                    let import_button_alpha = (self.import_button_opacity * 255.0)
                        .round()
                        .clamp(0.0, 255.0) as u8;
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
                            (import_button_alpha as f32 * 0.20).round().clamp(0.0, 255.0)
                                as u8
                        } else {
                            0
                        };
                        ui.painter().rect_filled(
                            inner_rect,
                            6.0,
                            egui::Color32::from_rgba_unmultiplied(
                                255,
                                255,
                                255,
                                import_fill_alpha,
                            ),
                        );
                        ui.painter().rect_stroke(
                            inner_rect,
                            6.0,
                            egui::Stroke::new(
                                stroke_width,
                                egui::Color32::from_rgba_unmultiplied(
                                    255,
                                    255,
                                    255,
                                    render_import_alpha,
                                ),
                            ),
                        );
                        #[cfg(target_os = "windows")]
                        {
                            let ppp = ctx.pixels_per_point();
                            let w_px = (button_rect.width() * ppp).ceil() as usize;
                            let h_px = (button_rect.height() * ppp).ceil() as usize;
                            let key = format!(
                                "import_button:{}:{}:{}",
                                import_button_text, w_px, h_px
                            );
                            let text_rgb = if import_fill_alpha > 128 {
                                (0u8, 0u8, 0u8)
                            } else {
                                (255u8, 255u8, 255u8)
                            };
                            let text_color = egui::Color32::from_rgba_unmultiplied(
                                text_rgb.0,
                                text_rgb.1,
                                text_rgb.2,
                                render_import_alpha,
                            );
                            if let Some(tex) = self.win_text_cache.get(&key) {
                                ui.painter().image(
                                    tex.id(),
                                    button_rect,
                                    egui::Rect::from_min_max(
                                        egui::pos2(0.0, 0.0),
                                        egui::pos2(1.0, 1.0),
                                    ),
                                    egui::Color32::WHITE,
                                );
                            } else if let Some(tex) = win_text_to_texture(
                                ctx,
                                &key,
                                &import_button_text,
                                self.button_hfont,
                                text_color,
                                w_px,
                                h_px,
                            ) {
                                self.win_text_cache.insert(key.clone(), tex.clone());
                                ui.painter().image(
                                    tex.id(),
                                    button_rect,
                                    egui::Rect::from_min_max(
                                        egui::pos2(0.0, 0.0),
                                        egui::pos2(1.0, 1.0),
                                    ),
                                    egui::Color32::WHITE,
                                );
                            } else {
                                ui.painter().text(
                                    button_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    import_button_text,
                                    button_font.clone(),
                                    egui::Color32::from_rgba_unmultiplied(
                                        text_color.r(),
                                        text_color.g(),
                                        text_color.b(),
                                        text_color.a(),
                                    ),
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
                                egui::Color32::from_rgba_unmultiplied(
                                    255,
                                    255,
                                    255,
                                    render_import_alpha,
                                ),
                            );
                        }
                    }
                    if controls_locked_by_settings && import_button_response.hovered() {
                        ctx.set_cursor_icon(egui::CursorIcon::Default);
                    } else {
                        apply_button_cursor(ctx, &import_button_response, import_button_enabled);
                    }
                    if import_button_interactive && import_button_response.clicked() {
                        if let Some(path) = FileDialog::new()
                            .add_filter("WireGuard config", &["conf"])
                            .pick_file()
                        {
                            let selected_path = path.display().to_string();
                            self.conf_path = Some(selected_path.clone());
                            self.error_log = None;
                            save_conf_path(self.conf_path.as_ref().unwrap());
                        }
                    }

                    let gap = 8.0 / ctx.pixels_per_point();
                    let ppp = ctx.pixels_per_point();
                    let gap_connect_text = 8.0 / ppp;
                    ui.add_space(gap_connect_text);

                    let connect_label = if self.service_active {
                        self.language.translate("Отключиться")
                    } else {
                        self.language.translate("Подключиться")
                    };
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
                        egui::Color32::from_rgba_unmultiplied(
                            220,
                            220,
                            220,
                            connect_hover_alpha,
                        )
                    } else {
                        egui::Color32::from_rgba_unmultiplied(
                            220,
                            220,
                            220,
                            connect_hover_alpha,
                        )
                    };
                    ui.painter().rect_filled(connect_rect, 6.0, connect_fill);
                    ui.painter().rect_stroke(
                        connect_rect,
                        6.0,
                        egui::Stroke::new(
                            1.0,
                            egui::Color32::from_rgba_unmultiplied(0, 0, 0, connect_hover_alpha),
                        ),
                    );
                    #[cfg(target_os = "windows")]
                    {
                        let ppp = ctx.pixels_per_point();
                        let w_px = (connect_rect.width() * ppp).ceil() as usize;
                        let h_px = (connect_rect.height() * ppp).ceil() as usize;
                        let key = format!("connect_button:{}:{}:{}", connect_label, w_px, h_px);
                        let text_color = egui::Color32::from_rgba_unmultiplied(
                            0,
                            0,
                            0,
                            connect_hover_alpha,
                        );
                        if let Some(tex) = self.win_text_cache.get(&key) {
                            ui.painter().image(
                                tex.id(),
                                connect_rect,
                                egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                ),
                                egui::Color32::WHITE,
                            );
                        } else if let Some(tex) = win_text_to_texture(
                            ctx,
                            &key,
                            &connect_label,
                            self.button_hfont,
                            text_color,
                            w_px,
                            h_px,
                        ) {
                            self.win_text_cache.insert(key.clone(), tex.clone());
                            ui.painter().image(
                                tex.id(),
                                connect_rect,
                                egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                ),
                                egui::Color32::WHITE,
                            );
                        } else {
                            ui.painter().text(
                                connect_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                connect_label,
                                egui::FontId::proportional(UI_BUTTON_FONT_SIZE),
                                egui::Color32::from_rgba_unmultiplied(
                                    0,
                                    0,
                                    0,
                                    connect_hover_alpha,
                                ),
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
                                let conf_path = conf.clone();
                                let (tx, rx) = mpsc::channel();
                                self.status_rx = Some(rx);
                                self.service_running = true;
                                self.error_log = None;
                                self.import_button_opacity = 1.0;
                                self.disconnect_animation_start = Some(Instant::now());

                                thread::spawn(move || {
                                    let result = stop_and_delete_service(&conf_path);
                                    let _ = tx.send(result);
                                });
                            } else {
                                self.import_button_opacity = 0.0;
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
                                self.last_tunnel_totals = None;
                                self.startup_animation_frame = 0;

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
                            self.status = self
                                .language
                                .translate("Сначала импортируйте .conf файл")
                                .to_owned();
                            show_error_dialog("Ошибка", &self.status);
                        }
                    }

                    ui.add_space(gap);
                    let _traffic_alpha =
                        (self.traffic_opacity * 255.0).round().clamp(0.0, 255.0) as u8;
                    let text_alpha: u8 = 255u8;
                    let text_width = connect_rect.width().min(ui.available_width());
                    let (text_rect, _) = ui.allocate_exact_size(
                        egui::vec2(text_width, connect_rect.height()),
                        egui::Sense::hover(),
                    );

                    let ppp = ctx.pixels_per_point();
                    let text_nudge = 16.0 / ppp;
                    let shifted_rect = text_rect.translate(egui::vec2(0.0, -text_nudge));
                    let text_position =
                        shifted_rect.center() + egui::vec2(0.0, -(4.0 + 2.0 / ppp));

                    if self
                        .last_time_display_update
                        .map_or(true, |t| t.elapsed() >= Duration::from_secs(1))
                    {
                        let mb = self.session_traffic_bytes as f64 / 1024.0 / 1024.0;
                        let traffic_text = if mb > 1000.0 {
                            format!("{:.2} GB", mb / 1024.0)
                        } else {
                            format!("{:.2} MB", mb)
                        };
                        self.cached_time_display =
                            format!("{} / {}", self.format_connection_time(), traffic_text);

                        let up_mbps = self.last_upload_bps / 1024.0 / 1024.0;
                        let down_mbps = self.last_download_bps / 1024.0 / 1024.0;
                        self.cached_up_display = format!("{:.2}", up_mbps);
                        self.cached_down_display = format!("{:.2}", down_mbps);

                        self.last_time_display_update = Some(Instant::now());
                    }
                    let display_text = &self.cached_time_display;

                    #[cfg(target_os = "windows")]
                    {
                        let ppp = ctx.pixels_per_point();
                        let w_px = (text_rect.width() * ppp).ceil() as usize;
                        let h_px = (text_rect.height() * ppp).ceil() as usize;
                        let key = format!("center_mode_display:{}:{}:{}", display_text, w_px, h_px);
                        let text_color = egui::Color32::from_white_alpha(text_alpha);
                        if let Some(tex) = self.win_text_cache.get(&key) {
                            ui.painter().image(
                                tex.id(),
                                shifted_rect,
                                egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                ),
                                egui::Color32::WHITE,
                            );
                        } else if let Some(tex) = win_text_to_texture(
                            ctx,
                            &key,
                            &display_text,
                            self.button_hfont,
                            text_color,
                            w_px,
                            h_px,
                        ) {
                            self.win_text_cache.insert(key.clone(), tex.clone());
                            ui.painter().image(
                                tex.id(),
                                shifted_rect,
                                egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                ),
                                egui::Color32::WHITE,
                            );
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

                    let pad_points = 12.0 / ppp;
                    let speed_alpha = if self.service_active { 255u8 } else { 0u8 };

                    egui::Area::new("upload_speed_area".into())
                        .anchor(egui::Align2::LEFT_BOTTOM, [pad_points, -pad_points])
                        .movable(false)
                        .show(ctx, |ui_area| {
                            let ppp_local = ctx.pixels_per_point();
                            let icon_size_points = 18.0 / ppp_local;
                            let spacing_points = 6.0 / ppp_local;
                            let added_width_points = 20.0 / ppp_local;
                            let text_str = self.cached_up_display.clone();
                            let font_id = button_font_id();
                            let galley = ui_area.fonts(|f| {
                                f.layout_no_wrap(
                                    text_str.clone(),
                                    font_id.clone(),
                                    egui::Color32::WHITE,
                                )
                            });
                            let text_size = galley.size();
                            let text_px = (text_size.x * ppp_local).ceil() as usize;
                            let pixel_margin = 8usize;
                            let w_px = (text_px + pixel_margin).max(1usize);
                            let h_px = (((icon_size_points
                                + spacing_points
                                + (w_px as f32) / ppp_local
                                + added_width_points)
                                .max(icon_size_points))
                                * ppp_local)
                                .ceil() as usize;
                            let text_points_with_margin = (w_px as f32) / ppp_local;
                            let total_width =
                                icon_size_points + spacing_points + text_points_with_margin + added_width_points;
                            let total_height = (h_px as f32 / ppp_local).max(icon_size_points);
                            let (rect, _) = ui_area.allocate_exact_size(
                                egui::vec2(total_width, total_height),
                                egui::Sense::hover(),
                            );
                            let painter = ui_area.painter();

                            let icon_x = (rect.min.x * ppp_local).round() / ppp_local;
                            let icon_y =
                                ((rect.max.y - icon_size_points) * ppp_local).round() / ppp_local;
                            let icon_rect = egui::Rect::from_min_size(
                                egui::pos2(icon_x, icon_y),
                                egui::vec2(icon_size_points, icon_size_points),
                            );
                            if let Some(tex) = &self.upload_icon {
                                painter.image(
                                    tex.id(),
                                    icon_rect,
                                    egui::Rect::from_min_max(
                                        egui::pos2(0.0, 0.0),
                                        egui::pos2(1.0, 1.0),
                                    ),
                                    egui::Color32::from_white_alpha(speed_alpha),
                                );
                            }

                            let mut text_left = rect.center().x - text_size.x * 0.5;
                            if text_left < rect.min.x {
                                text_left = rect.min.x;
                            }
                            if text_left > rect.max.x - text_size.x {
                                text_left = rect.max.x - text_size.x;
                            }
                            let text_y = rect.min.y + (rect.height() - text_size.y) * 0.5;
                            let snapped_x = (text_left * ppp_local).round() / ppp_local;
                            let snapped_y = (text_y * ppp_local).round() / ppp_local;
                            let text_rect = egui::Rect::from_min_size(
                                egui::pos2(snapped_x, snapped_y),
                                egui::vec2((w_px as f32) / ppp_local, rect.height()),
                            );
                            let text_color = egui::Color32::from_white_alpha(speed_alpha);
                            #[cfg(target_os = "windows")]
                            {
                                let key = format!("speed_up:{}:{}:{}", text_str, w_px, h_px);
                                if let Some(tex) = self.win_text_cache.get(&key) {
                                    painter.image(
                                        tex.id(),
                                        text_rect,
                                        egui::Rect::from_min_max(
                                            egui::pos2(0.0, 0.0),
                                            egui::pos2(1.0, 1.0),
                                        ),
                                        text_color,
                                    );
                                } else if let Some(tex) = win_text_to_texture(
                                    ctx,
                                    &key,
                                    &text_str,
                                    self.button_hfont,
                                    text_color,
                                    w_px,
                                    h_px,
                                ) {
                                    self.win_text_cache.insert(key.clone(), tex.clone());
                                    painter.image(
                                        tex.id(),
                                        text_rect,
                                        egui::Rect::from_min_max(
                                            egui::pos2(0.0, 0.0),
                                            egui::pos2(1.0, 1.0),
                                        ),
                                        text_color,
                                    );
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
                            let icon_size_points = 18.0 / ppp_local;
                            let spacing_points = 6.0 / ppp_local;
                            let added_width_points = 20.0 / ppp_local;
                            let text_str = self.cached_down_display.clone();
                            let font_id = button_font_id();
                            let galley = ui_area.fonts(|f| {
                                f.layout_no_wrap(
                                    text_str.clone(),
                                    font_id.clone(),
                                    egui::Color32::WHITE,
                                )
                            });
                            let text_size = galley.size();
                            let text_px = (text_size.x * ppp_local).ceil() as usize;
                            let pixel_margin = 8usize;
                            let w_px = (text_px + pixel_margin).max(1usize);
                            let text_points_with_margin = (w_px as f32) / ppp_local;
                            let font_px_est =
                                (UI_BUTTON_FONT_SIZE * current_ui_scale_factor()).round() as usize;
                            let text_height_points_est = ((font_px_est + 4usize) as f32) / ppp_local;
                            let total_width =
                                text_points_with_margin + spacing_points + icon_size_points + added_width_points;
                            let total_height = text_height_points_est.max(icon_size_points);
                            let (rect, _) = ui_area.allocate_exact_size(
                                egui::vec2(total_width, total_height),
                                egui::Sense::hover(),
                            );
                            let painter = ui_area.painter();
                            let h_px = (rect.height() * ppp_local).ceil() as usize;

                            let mut text_left = rect.center().x - text_size.x * 0.5;
                            if text_left < rect.min.x {
                                text_left = rect.min.x;
                            }
                            if text_left > rect.max.x - text_size.x {
                                text_left = rect.max.x - text_size.x;
                            }
                            let text_y = rect.min.y + (rect.height() - text_size.y) * 0.5;
                            let snapped_x = (text_left * ppp_local).round() / ppp_local;
                            let snapped_y = (text_y * ppp_local).round() / ppp_local;
                            let text_rect = egui::Rect::from_min_size(
                                egui::pos2(snapped_x, snapped_y),
                                egui::vec2((w_px as f32) / ppp_local, rect.height()),
                            );
                            let text_color = egui::Color32::from_white_alpha(speed_alpha);
                            #[cfg(target_os = "windows")]
                            {
                                let key = format!("speed_down:{}:{}:{}", text_str, w_px, h_px);
                                if let Some(tex) = self.win_text_cache.get(&key) {
                                    painter.image(
                                        tex.id(),
                                        text_rect,
                                        egui::Rect::from_min_max(
                                            egui::pos2(0.0, 0.0),
                                            egui::pos2(1.0, 1.0),
                                        ),
                                        text_color,
                                    );
                                } else if let Some(tex) = win_text_to_texture(
                                    ctx,
                                    &key,
                                    &text_str,
                                    self.button_hfont,
                                    text_color,
                                    w_px,
                                    h_px,
                                ) {
                                    self.win_text_cache.insert(key.clone(), tex.clone());
                                    painter.image(
                                        tex.id(),
                                        text_rect,
                                        egui::Rect::from_min_max(
                                            egui::pos2(0.0, 0.0),
                                            egui::pos2(1.0, 1.0),
                                        ),
                                        text_color,
                                    );
                                } else {
                                    painter.galley(text_rect.min, galley.clone(), text_color);
                                }
                            }
                            #[cfg(not(target_os = "windows"))]
                            {
                                painter.galley(text_rect.min, galley.clone(), text_color);
                            }

                            let icon_x =
                                ((rect.max.x - icon_size_points) * ppp_local).round() / ppp_local;
                            let icon_y =
                                ((rect.max.y - icon_size_points) * ppp_local).round() / ppp_local;
                            let icon_rect = egui::Rect::from_min_size(
                                egui::pos2(icon_x, icon_y),
                                egui::vec2(icon_size_points, icon_size_points),
                            );
                            if let Some(tex) = &self.download_icon {
                                painter.image(
                                    tex.id(),
                                    icon_rect,
                                    egui::Rect::from_min_max(
                                        egui::pos2(0.0, 0.0),
                                        egui::pos2(1.0, 1.0),
                                    ),
                                    egui::Color32::from_white_alpha(speed_alpha),
                                );
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
                            self.wireproxy_info_addr =
                                service_result.wireproxy_info_addr.clone();
                            if self.service_active {
                                self.import_button_opacity = 0.0;
                                self.start_tunnel_traffic_worker();
                                if !was_active {
                                    self.connected_at = Some(Instant::now());
                                    self.session_traffic_bytes = 0;
                                    self.session_base_traffic_bytes = None;
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
                                    self.animated_frame_index = 0;
                                    self.animated_last_frame = Instant::now();
                                    let notification_conf_name = self
                                        .conf_path
                                        .as_deref()
                                        .map(|conf| {
                                            Path::new(conf)
                                                .file_name()
                                                .and_then(|name| name.to_str())
                                                .unwrap_or(conf)
                                                .to_string()
                                        })
                                        .unwrap_or_else(|| {
                                            self.language.translate("Туннель подключен").to_owned()
                                        });
                                    self.show_silent_windows_notification(
                                        self.language.translate("Подключен"),
                                        &notification_conf_name,
                                        "vpnfybot-windows/connected",
                                    );
                                    let selected_processes = load_selected_processes();
                                    let proxy_mode = load_proxy_mode();
                                    let selected_sites = self.selected_sites.clone();
                                    let should_run_proxybridge = !proxy_mode
                                        || !selected_processes.is_empty()
                                        || !selected_sites.is_empty();

                                    if should_run_proxybridge {
                                        let status_text = format_proxybridge_status(
                                            selected_processes.len(),
                                            selected_sites.len(),
                                            proxy_mode,
                                            false,
                                        );
                                        self.status = status_text;
                                        ui.ctx().request_repaint();

                                        match start_proxybridge(
                                            &selected_processes,
                                            &selected_sites,
                                            proxy_mode,
                                            self.wireproxy_info_addr.as_deref(),
                                        ) {
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
                                                self.status =
                                                    format!("❌ ProxyBridge ошибка: {}", e);
                                                show_error_dialog(
                                                    self.language.translate("Ошибка"),
                                                    &self.status,
                                                );
                                            }
                                        }
                                    } else {
                                        self.status = format!(
                                            "✅ {}: {}",
                                            self.language.translate("Туннель подключен"),
                                            self.language
                                                .translate("Выберите процессы для маршрутизации")
                                        );
                                    }
                                }
                                self.status.clear();
                            } else {
                                self.connected_at = None;
                                self.reset_tunnel_traffic_state();
                                self.import_button_opacity = 1.0;
                                self.connect_animation_start = None;

                                let had_error = self.error_log.is_some();
                                if had_error {
                                    self.status = service_result.message.clone();
                                } else {
                                    self.status.clear();
                                }

                                if self.proxybridge_running {
                                    match stop_proxybridge() {
                                        Ok(_) => {
                                            if !had_error {
                                                self.status.clear();
                                            }
                                        }
                                        Err(e) => {
                                            self.status = format!(
                                                "{}: {}",
                                                self.language
                                                    .translate("Ошибка остановки ProxyBridge"),
                                                e,
                                            );
                                            show_error_dialog(
                                                self.language.translate("Ошибка"),
                                                &self.status,
                                            );
                                        }
                                    }
                                    self.proxybridge_running = false;
                                }
                            }
                            if !self.service_active {
                                if let Some(ref error_log) = self.error_log {
                                    show_error_dialog(self.language.translate("Ошибка"), error_log);
                                } else if was_active {
                                    let notification_conf_name = self
                                        .conf_path
                                        .as_deref()
                                        .map(|conf| {
                                            Path::new(conf)
                                                .file_name()
                                                .and_then(|name| name.to_str())
                                                .unwrap_or(conf)
                                                .to_string()
                                        })
                                        .unwrap_or_else(|| {
                                            self.language.translate("Туннель отключен").to_owned()
                                        });
                                    self.show_silent_windows_notification(
                                        self.language.translate("Отключен"),
                                        &notification_conf_name,
                                        "vpnfybot-windows/disconnected",
                                    );
                                }
                            }
                            self.status_rx = None;
                        }
                    }

                    if self.service_running {
                        self.startup_animation_frame =
                            self.startup_animation_frame.wrapping_add(1);
                    }

                    if self.service_active {
                        ctx.request_repaint_after(TUNNEL_TRAFFIC_POLL_INTERVAL);

                        if self.apply_pending_tunnel_traffic_samples() {
                            self.last_time_display_update = None;
                            is_animating = true;
                        }
                    }

                    let target_traffic_opacity = if self.service_active { 1.0 } else { 0.0 };
                    let traffic_delta = target_traffic_opacity - self.traffic_opacity;
                    if traffic_delta.abs() > 0.001 {
                        self.traffic_opacity += traffic_delta * 0.154;
                        self.traffic_opacity = self.traffic_opacity.clamp(0.0, 1.0);
                        is_animating = true;
                    }

                    if self.show_settings {
                        let app_rect = ctx.available_rect();

                        let painter = ctx.layer_painter(egui::LayerId::new(
                            egui::Order::Middle,
                            egui::Id::new("settings_overlay_bg"),
                        ));
                        painter.rect_filled(
                            app_rect,
                            0.0,
                            egui::Color32::from_rgba_unmultiplied(0, 0, 0, 204),
                        );

                        let content_rect = app_rect.shrink2(egui::vec2(edge_pad, edge_pad));
                        let settings_header_left = content_rect.min.x;
                        let settings_header_top = content_rect.min.y;
                        let settings_header_width = content_rect.width().max(0.0);
                        let settings_close_size = egui::vec2(36.0, 28.0);

                        let close_response = egui::Area::new(egui::Id::new("settings_close_button"))
                            .fixed_pos(egui::pos2(
                                settings_header_left + settings_header_width - settings_close_size.x,
                                settings_header_top,
                            ))
                            .movable(false)
                            .order(egui::Order::Debug)
                            .interactable(true)
                            .show(ctx, |ui| {
                                let (button_rect, response) = ui.allocate_exact_size(
                                    settings_close_size,
                                    egui::Sense::click(),
                                );
                                let close_alpha = button_alpha(&response, 255);
                                if let Some(settings_close_icon) = &self.settings_close_icon {
                                    ui.painter().image(
                                        settings_close_icon.id(),
                                        button_rect.shrink2(egui::vec2(6.0, 2.0)),
                                        egui::Rect::from_min_max(
                                            egui::pos2(0.0, 0.0),
                                            egui::pos2(1.0, 1.0),
                                        ),
                                        egui::Color32::from_rgba_unmultiplied(
                                            255,
                                            255,
                                            255,
                                            close_alpha,
                                        ),
                                    );
                                } else {
                                    #[cfg(target_os = "windows")]
                                    {
                                        let ppp = ctx.pixels_per_point();
                                        let w_px = (button_rect.width() * ppp).ceil() as usize;
                                        let h_px = (button_rect.height() * ppp).ceil() as usize;
                                        let key = format!("settings_close:{}:{}:{}", "❌", w_px, h_px);
                                        let text_color = egui::Color32::from_rgba_unmultiplied(
                                            255,
                                            255,
                                            255,
                                            close_alpha,
                                        );
                                        if let Some(tex) = self.win_text_cache.get(&key) {
                                            ui.painter().image(
                                                tex.id(),
                                                button_rect,
                                                egui::Rect::from_min_max(
                                                    egui::pos2(0.0, 0.0),
                                                    egui::pos2(1.0, 1.0),
                                                ),
                                                egui::Color32::WHITE,
                                            );
                                        } else if let Some(tex) = win_text_to_texture(
                                            ctx,
                                            &key,
                                            "❌",
                                            self.button_hfont,
                                            text_color,
                                            w_px,
                                            h_px,
                                        ) {
                                            self.win_text_cache.insert(key.clone(), tex.clone());
                                            ui.painter().image(
                                                tex.id(),
                                                button_rect,
                                                egui::Rect::from_min_max(
                                                    egui::pos2(0.0, 0.0),
                                                    egui::pos2(1.0, 1.0),
                                                ),
                                                egui::Color32::WHITE,
                                            );
                                        } else {
                                            ui.painter().text(
                                                button_rect.center(),
                                                egui::Align2::CENTER_CENTER,
                                                "❌",
                                                egui::FontId::proportional(24.0),
                                                egui::Color32::from_rgba_unmultiplied(
                                                    255,
                                                    255,
                                                    255,
                                                    close_alpha,
                                                ),
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
                                            egui::Color32::from_rgba_unmultiplied(
                                                255,
                                                255,
                                                255,
                                                close_alpha,
                                            ),
                                        );
                                    }
                                }
                                response
                            })
                            .inner;

                        apply_button_cursor(ctx, &close_response, true);
                        if close_response.clicked() {
                            self.show_settings = false;
                        }
                        egui::Area::new(egui::Id::new("settings_content_area"))
                            .fixed_pos(content_rect.min)
                            .order(egui::Order::Foreground)
                            .show(ctx, |ui| {
                                ui.visuals_mut().override_text_color = Some(egui::Color32::WHITE);
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

                                        let sites_window_open =
                                            self.site_window_receiver.is_some()
                                                || site_editor::is_open();
                                        let sites_button_enabled =
                                            !self.service_active && !sites_window_open;
                                        let sites_label_key = if self.proxy_mode_toggle {
                                            "Сайты через VPN"
                                        } else {
                                            "Исключенные сайты"
                                        };
                                        let sites_button_text = format!(
                                            "{} [{}]",
                                            self.language.translate(sites_label_key),
                                            self.selected_sites.len()
                                        );

                                        let process_window_open =
                                            self.process_window_receiver.is_some()
                                                || process_editor::is_open();
                                        let process_button_enabled =
                                            !self.service_active && !process_window_open;
                                        let process_label_key = if self.proxy_mode_toggle {
                                            "Приложения через VPN"
                                        } else {
                                            "Исключенные приложения"
                                        };
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
                                        let bottom_padding = 8.0 / ctx.pixels_per_point();

                                        let shift_points = 8.0 / ctx.pixels_per_point();
                                        let mut mode_rect = egui::Rect::from_min_size(
                                            egui::pos2(
                                                settings_rect.left(),
                                                settings_rect.bottom()
                                                    - bottom_padding
                                                    - button_height,
                                            ),
                                            egui::vec2(button_width, button_height),
                                        );
                                        mode_rect = mode_rect.translate(egui::vec2(0.0, shift_points));
                                        let process_rect = mode_rect
                                            .translate(egui::vec2(0.0, -(button_height + button_spacing)));
                                        let sites_rect = process_rect
                                            .translate(egui::vec2(0.0, -(button_height + button_spacing)));

                                        let description_width = ((settings_rect.width() * 0.7)
                                            + 40.0)
                                            .max(160.0)
                                            .min(settings_rect.width());
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
                                                fonts.layout_no_wrap(
                                                    candidate_line.clone(),
                                                    button_font.clone(),
                                                    description_color,
                                                )
                                            });
                                            if !current_line.is_empty()
                                                && candidate_galley.size().x > description_width
                                            {
                                                description_lines.push(ui.fonts(|fonts| {
                                                    fonts.layout_no_wrap(
                                                        current_line.clone(),
                                                        button_font.clone(),
                                                        description_color,
                                                    )
                                                }));
                                                current_line = word.to_string();
                                            } else {
                                                current_line = candidate_line;
                                            }
                                        }
                                        if !current_line.is_empty() {
                                            description_lines.push(ui.fonts(|fonts| {
                                                fonts.layout_no_wrap(
                                                    current_line.clone(),
                                                    button_font.clone(),
                                                    description_color,
                                                )
                                            }));
                                        }
                                        let description_top = settings_rect.top();
                                        let description_bottom =
                                            (sites_rect.top() - button_spacing).max(description_top);
                                        let description_center_y = description_top
                                            + ((description_bottom - description_top) * 0.5);
                                        let line_spacing = 2.0;
                                        let total_description_height = description_lines
                                            .iter()
                                            .map(|galley| galley.size().y)
                                            .sum::<f32>()
                                            + line_spacing
                                                * description_lines.len().saturating_sub(1) as f32;
                                        let mut description_y =
                                            description_center_y - total_description_height * 0.5;

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
                                            egui::Color32::from_rgba_unmultiplied(
                                                255,
                                                255,
                                                255,
                                                button_alpha_val,
                                            )
                                        } else {
                                            egui::Color32::from_rgba_unmultiplied(
                                                180,
                                                80,
                                                80,
                                                button_alpha_val,
                                            )
                                        };
                                        let text_color = if self.proxy_mode_toggle {
                                            egui::Color32::from_rgba_unmultiplied(
                                                0,
                                                0,
                                                0,
                                                button_alpha_val,
                                            )
                                        } else {
                                            egui::Color32::from_rgba_unmultiplied(
                                                255,
                                                255,
                                                255,
                                                button_alpha_val,
                                            )
                                        };
                                        ui.painter().rect_filled(mode_rect, 6.0, button_fill);
                                        #[cfg(target_os = "windows")]
                                        {
                                            let ppp = ctx.pixels_per_point();
                                            let w_px = (mode_rect.width() * ppp).ceil() as usize;
                                            let h_px = (mode_rect.height() * ppp).ceil() as usize;
                                            let key =
                                                format!("settings_mode:{}:{}:{}", mode_text, w_px, h_px);
                                            let chosen_font = self.button_hfont;

                                            if let Some(tex) = self.win_text_cache.get(&key) {
                                                ui.painter().image(
                                                    tex.id(),
                                                    mode_rect,
                                                    egui::Rect::from_min_max(
                                                        egui::pos2(0.0, 0.0),
                                                        egui::pos2(1.0, 1.0),
                                                    ),
                                                    egui::Color32::WHITE,
                                                );
                                            } else if let Some(tex) = win_text_to_texture(
                                                ctx,
                                                &key,
                                                &mode_text,
                                                chosen_font,
                                                text_color,
                                                w_px,
                                                h_px,
                                            ) {
                                                self.win_text_cache.insert(key.clone(), tex.clone());
                                                ui.painter().image(
                                                    tex.id(),
                                                    mode_rect,
                                                    egui::Rect::from_min_max(
                                                        egui::pos2(0.0, 0.0),
                                                        egui::pos2(1.0, 1.0),
                                                    ),
                                                    egui::Color32::WHITE,
                                                );
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
                                            egui::Color32::from_rgba_unmultiplied(
                                                255,
                                                255,
                                                255,
                                                process_alpha_val,
                                            ),
                                        );
                                        #[cfg(target_os = "windows")]
                                        {
                                            let ppp = ctx.pixels_per_point();
                                            let w_px = (process_rect.width() * ppp).ceil() as usize;
                                            let h_px = (process_rect.height() * ppp).ceil() as usize;
                                            let key = format!(
                                                "settings_process:{}:{}:{}",
                                                process_button_text, w_px, h_px
                                            );
                                            let text_color = egui::Color32::from_rgba_unmultiplied(
                                                0,
                                                0,
                                                0,
                                                process_alpha_val,
                                            );
                                            if let Some(tex) = self.win_text_cache.get(&key) {
                                                ui.painter().image(
                                                    tex.id(),
                                                    process_rect,
                                                    egui::Rect::from_min_max(
                                                        egui::pos2(0.0, 0.0),
                                                        egui::pos2(1.0, 1.0),
                                                    ),
                                                    egui::Color32::WHITE,
                                                );
                                            } else if let Some(tex) = win_text_to_texture(
                                                ctx,
                                                &key,
                                                &process_button_text,
                                                self.button_hfont,
                                                text_color,
                                                w_px,
                                                h_px,
                                            ) {
                                                self.win_text_cache.insert(key.clone(), tex.clone());
                                                ui.painter().image(
                                                    tex.id(),
                                                    process_rect,
                                                    egui::Rect::from_min_max(
                                                        egui::pos2(0.0, 0.0),
                                                        egui::pos2(1.0, 1.0),
                                                    ),
                                                    egui::Color32::WHITE,
                                                );
                                            } else {
                                                ui.painter().text(
                                                    process_rect.center(),
                                                    egui::Align2::CENTER_CENTER,
                                                    process_button_text,
                                                    button_font.clone(),
                                                    egui::Color32::from_rgba_unmultiplied(
                                                        0,
                                                        0,
                                                        0,
                                                        process_alpha_val,
                                                    ),
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
                                                egui::Color32::from_rgba_unmultiplied(
                                                    0,
                                                    0,
                                                    0,
                                                    process_alpha_val,
                                                ),
                                            );
                                        }
                                        apply_button_cursor(
                                            ctx,
                                            &process_response,
                                            process_button_enabled,
                                        );
                                        if process_response.clicked() && process_button_enabled {
                                            self.cached_processes = get_running_processes();
                                            refresh_running_processes_async();
                                            self.last_process_refresh = Some(Instant::now());

                                            if process_editor::show_existing() {
                                            } else if self.process_window_receiver.is_none() {
                                                let process_window_title = if self.proxy_mode_toggle {
                                                    self.language.translate("Приложения через VPN").to_owned()
                                                } else {
                                                    self.language
                                                        .translate("Исключенные приложения")
                                                        .to_owned()
                                                };
                                                self.process_window_receiver = Some(
                                                    process_editor::open_external(
                                                        self.cached_processes.clone(),
                                                        self.selected_processes.clone(),
                                                        process_window_title,
                                                        self.language.translate("Сохранить").to_owned(),
                                                    ),
                                                );
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
                                            egui::Color32::from_rgba_unmultiplied(
                                                255,
                                                255,
                                                255,
                                                sites_alpha_val,
                                            ),
                                        );
                                        #[cfg(target_os = "windows")]
                                        {
                                            let ppp = ctx.pixels_per_point();
                                            let w_px = (sites_rect.width() * ppp).ceil() as usize;
                                            let h_px = (sites_rect.height() * ppp).ceil() as usize;
                                            let key = format!(
                                                "settings_sites:{}:{}:{}",
                                                sites_button_text, w_px, h_px
                                            );
                                            let text_color = egui::Color32::from_rgba_unmultiplied(
                                                0,
                                                0,
                                                0,
                                                sites_alpha_val,
                                            );
                                            if let Some(tex) = self.win_text_cache.get(&key) {
                                                ui.painter().image(
                                                    tex.id(),
                                                    sites_rect,
                                                    egui::Rect::from_min_max(
                                                        egui::pos2(0.0, 0.0),
                                                        egui::pos2(1.0, 1.0),
                                                    ),
                                                    egui::Color32::WHITE,
                                                );
                                            } else if let Some(tex) = win_text_to_texture(
                                                ctx,
                                                &key,
                                                &sites_button_text,
                                                self.button_hfont,
                                                text_color,
                                                w_px,
                                                h_px,
                                            ) {
                                                self.win_text_cache.insert(key.clone(), tex.clone());
                                                ui.painter().image(
                                                    tex.id(),
                                                    sites_rect,
                                                    egui::Rect::from_min_max(
                                                        egui::pos2(0.0, 0.0),
                                                        egui::pos2(1.0, 1.0),
                                                    ),
                                                    egui::Color32::WHITE,
                                                );
                                            } else {
                                                ui.painter().text(
                                                    sites_rect.center(),
                                                    egui::Align2::CENTER_CENTER,
                                                    sites_button_text,
                                                    button_font.clone(),
                                                    egui::Color32::from_rgba_unmultiplied(
                                                        0,
                                                        0,
                                                        0,
                                                        sites_alpha_val,
                                                    ),
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
                                                egui::Color32::from_rgba_unmultiplied(
                                                    0,
                                                    0,
                                                    0,
                                                    sites_alpha_val,
                                                ),
                                            );
                                        }
                                        apply_button_cursor(
                                            ctx,
                                            &sites_response,
                                            sites_button_enabled,
                                        );
                                        if sites_response.clicked() && sites_button_enabled {
                                            let sites_window_title = if self.proxy_mode_toggle {
                                                self.language.translate("Сайты через VPN").to_owned()
                                            } else {
                                                self.language.translate("Исключенные сайты").to_owned()
                                            };
                                            if site_editor::show_existing() {
                                            } else if self.site_window_receiver.is_none() {
                                                self.site_window_receiver = Some(
                                                    site_editor::open_external(
                                                        self.selected_sites.join("\r\n"),
                                                        sites_window_title,
                                                        self.language.translate("Сохранить").to_owned(),
                                                    ),
                                                );
                                            }
                                        }

                                        for description_line in description_lines {
                                            let line_height = description_line.size().y;
                                            let description_pos = egui::pos2(
                                                settings_rect.center().x
                                                    - description_line.size().x * 0.5,
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
                    }

                    ui.add_space(0.0);

                    ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                        ui.add_space(10.0);
                        {
                            let version_text = if update_check::UPDATE_CHECK_RUNNING
                                .get()
                                .map(|b| b.load(std::sync::atomic::Ordering::Relaxed))
                                .unwrap_or(false)
                            {
                                self.language.translate("Проверка обновлений").to_owned()
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
                        let galley = ui.fonts(|fonts| {
                            fonts.layout_no_wrap(link_text.to_string(), button_font.clone(), link_color)
                        });
                        let ppp = ctx.pixels_per_point();
                        let extra_px = 20.0f32;
                        let extra_points = extra_px / ppp;
                        let extra_y_px = 8.0f32;
                        let extra_y_points = extra_y_px / ppp;
                        let widget_size = egui::vec2(galley.size().x + extra_points, galley.size().y);
                        let (link_rect, response) = ui.allocate_exact_size(
                            widget_size,
                            if link_enabled {
                                egui::Sense::click()
                            } else {
                                egui::Sense::hover()
                            },
                        );

                        #[cfg(target_os = "windows")]
                        {
                            let ppp = ctx.pixels_per_point();
                            let w_px = (link_rect.width() * ppp).ceil() as usize;
                            let h_px = (link_rect.height() * ppp).ceil() as usize;
                            let key = format!("link:{}:{}:{}", link_text, w_px, h_px);
                            if let Some(tex) = self.win_text_cache.get(&key) {
                                ui.painter().image(
                                    tex.id(),
                                    link_rect.translate(egui::vec2(0.0, extra_y_points)),
                                    egui::Rect::from_min_max(
                                        egui::pos2(0.0, 0.0),
                                        egui::pos2(1.0, 1.0),
                                    ),
                                    egui::Color32::WHITE,
                                );
                            } else if let Some(tex) = win_text_to_texture(
                                ctx,
                                &key,
                                link_text,
                                self.button_hfont,
                                link_color,
                                w_px,
                                h_px,
                            ) {
                                self.win_text_cache.insert(key.clone(), tex.clone());
                                ui.painter().image(
                                    tex.id(),
                                    link_rect.translate(egui::vec2(0.0, extra_y_points)),
                                    egui::Rect::from_min_max(
                                        egui::pos2(0.0, 0.0),
                                        egui::pos2(1.0, 1.0),
                                    ),
                                    egui::Color32::WHITE,
                                );
                            } else {
                                ui.painter().galley(
                                    link_rect.min + egui::vec2(0.0, extra_y_points),
                                    galley.clone(),
                                    link_color,
                                );
                            }
                        }
                        #[cfg(not(target_os = "windows"))]
                        {
                            ui.painter().galley(
                                link_rect.min + egui::vec2(0.0, extra_y_points),
                                galley.clone(),
                                link_color,
                            );
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
        self.stop_tunnel_traffic_worker();

        if let Some(ref conf) = self.conf_path {
            let _ = stop_and_delete_service(conf);
        }

        if self.proxybridge_running {
            if let Some(mut child) = self.proxybridge_child.take() {
                if let Err(e) = child.kill() {
                    log::warn!(
                        "Не удалось убить дочерний процесс ProxyBridge напрямую: {}",
                        e
                    );
                }
                let _ = child.wait();
            }

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