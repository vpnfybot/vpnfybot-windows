use super::*;

#[cfg(target_os = "windows")]
use std::mem::ManuallyDrop;
#[cfg(target_os = "windows")]
use windows::core::{ComInterface, GUID, HRESULT, PWSTR};
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
use windows::Win32::UI::Shell::{PropertiesSystem::IPropertyStore, IShellLinkW};

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
fn windows_error(message: impl Into<String>) -> windows::core::Error {
    windows::core::Error::new(HRESULT(0x80004005u32 as i32), HSTRING::from(message.into()))
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
    let exe_dir = exe_path.parent().ok_or_else(|| {
        windows_error("current_exe returned a path without a parent directory")
    })?;

    if let Some(parent) = shortcut_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            windows_error(format!("create_dir_all for shortcut directory failed: {}", error))
        })?;
    }

    let com_initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok() };
    let result = (|| -> windows::core::Result<Option<PathBuf>> {
        let shell_link: IShellLinkW = unsafe {
            CoCreateInstance(&CLSID_SHELL_LINK, None, CLSCTX_INPROC_SERVER)?
        };

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
    let _ = EnumChildWindows(
        target_hwnd,
        Some(enable_file_drop_for_children),
        LPARAM(0),
    );
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
    #[allow(deprecated)]
    pub(super) fn ensure_tray_subclass(&mut self, frame: &mut Frame) {
        if self.tray_subclassed {
            return;
        }

        if let Ok(window_handle) = frame.window_handle() {
            if let Ok(RawWindowHandle::Win32(handle)) = window_handle.raw_window_handle() {
                let raw_hwnd = HWND(handle.hwnd.get());
                let root_hwnd = unsafe { GetAncestor(raw_hwnd, GA_ROOT) };
                let hwnd = if root_hwnd.0 != 0 { root_hwnd } else { raw_hwnd };
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
                return true;
            }
        }

        false
    }

    pub(super) fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let maybe_path = ctx.input(|input| {
            input.raw.dropped_files.iter().find_map(|file| {
                let path = file.path.as_ref()?;
                let extension = path.extension().and_then(|ext| ext.to_str())?;
                if extension.eq_ignore_ascii_case("conf") {
                    Some(path.to_string_lossy().to_string())
                } else {
                    None
                }
            })
        }).or_else(|| {
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

        self.conf_path = Some(path.clone());
        self.error_log = None;
        save_conf_path(self.conf_path.as_ref().unwrap());
        self.status.clear();
    }
}