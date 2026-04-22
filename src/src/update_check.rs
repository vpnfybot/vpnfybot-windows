use std::sync::{OnceLock, Mutex, Arc};
use std::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
use std::io::Write;

#[derive(Clone)]
pub struct UpdateAvailable {
    pub asset_name: String,
    pub download_url: String,
}

pub static UPDATE_AVAILABLE: OnceLock<Mutex<Option<UpdateAvailable>>> = OnceLock::new();
pub static UPDATE_DOWNLOAD_PROGRESS: OnceLock<Arc<AtomicUsize>> = OnceLock::new();
pub static UPDATE_DOWNLOADING: OnceLock<Arc<AtomicBool>> = OnceLock::new();

/// Prevent concurrent update checks from spawning multiple threads.
pub static UPDATE_CHECK_RUNNING: OnceLock<Arc<AtomicBool>> = OnceLock::new();

/// Spawn a background thread to check GitHub Releases for updates.
/// This function is callable both at startup and manually (e.g., button/key press).
pub fn spawn_update_check_thread() {
    // Avoid spawning multiple concurrent check threads.
    let running = UPDATE_CHECK_RUNNING.get_or_init(|| Arc::new(AtomicBool::new(false))).clone();
    // If already running, log and return.
    if running.swap(true, Ordering::SeqCst) {
        let _ = std::fs::create_dir_all(super::app_dirs::get_logs_dir());
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(super::app_dirs::get_logs_dir().join("update_check.log"))
            .and_then(|mut f| f.write_all("[update] check already in progress\n".as_bytes()));
        return;
    }
    let running_clone = running.clone();
    std::thread::spawn(move || {
        // Ensure the running flag is cleared when the thread exits.
        struct ClearFlag(Arc<AtomicBool>);
        impl Drop for ClearFlag {
            fn drop(&mut self) {
                // Clear running flag
                self.0.store(false, Ordering::SeqCst);
                // Log thread finish
                let _ = std::fs::create_dir_all(super::app_dirs::get_logs_dir());
                let _ = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(super::app_dirs::get_logs_dir().join("update_check.log"))
                    .and_then(|mut f| f.write_all("[update] thread finished\n".as_bytes()));
            }
        }
        let _clear = ClearFlag(running_clone);
        let append_log = |s: &str| {
            let log_dir = super::app_dirs::get_logs_dir();
            let _ = std::fs::create_dir_all(&log_dir);
            let _ = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_dir.join("update_check.log"))
                .and_then(|mut f| f.write_all(format!("{}\n", s).as_bytes()));
        };

        append_log("[update] thread start");

        let origin_out = {
            #[cfg(target_os = "windows")]
            {
                use std::os::windows::process::CommandExt;
                std::process::Command::new("git")
                    .args(["remote", "get-url", "origin"])
                    .creation_flags(0x08000000)
                    .output()
            }
            #[cfg(not(target_os = "windows"))]
            {
                std::process::Command::new("git")
                    .args(["remote", "get-url", "origin"])
                    .output()
            }
        };
        let mut url = match origin_out {
            Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
            Err(e) => {
                append_log(&format!("[update] git command failed: {} - using fallback repo", e));
                String::new()
            }
        };
        if url.is_empty() {
            append_log("[update] origin empty or git unavailable, using fallback repo");
            url = "https://github.com/vpnfybot/vpnfybot-windows.git".to_string();
        }
        append_log(&format!("[update] origin url='{}'", url));

        let mut owner = String::new();
        let mut repo = String::new();
        if url.contains("github.com") {
            if url.starts_with("git@") {
                if let Some(pos) = url.find(':') {
                    let path = &url[pos + 1..];
                    let path = path.strip_suffix(".git").unwrap_or(path);
                    let mut parts = path.splitn(2, '/');
                    if let (Some(o), Some(r)) = (parts.next(), parts.next()) {
                        owner = o.to_string();
                        repo = r.to_string();
                    }
                }
            } else if url.starts_with("http") {
                if let Some(pos) = url.find("github.com/") {
                    let path = &url[pos + "github.com/".len()..];
                    let path = path.strip_suffix(".git").unwrap_or(path);
                    let mut parts = path.splitn(2, '/');
                    if let (Some(o), Some(r)) = (parts.next(), parts.next()) {
                        owner = o.to_string();
                        repo = r.to_string();
                    }
                }
            }
        }
        if owner.is_empty() || repo.is_empty() {
            append_log(&format!("[update] cannot parse owner/repo from '{}'", url));
            return;
        }

        let api_url = format!("https://api.github.com/repos/{}/{}/releases/latest", owner, repo);
        append_log(&format!("[update] api_url='{}'", api_url));
        let agent = "vpnfybot-windows-update-check";

        let resp = match ureq::get(&api_url).set("User-Agent", agent).call() {
            Ok(r) => r,
            Err(e) => {
                append_log(&format!("[update] api request failed: {}", e));
                return;
            }
        };

        let json: serde_json::Value = match resp.into_string() {
            Ok(s) => match serde_json::from_str(&s) {
                Ok(j) => j,
                Err(e) => {
                    append_log(&format!("[update] json parse failed: {}", e));
                    return;
                }
            },
            Err(e) => {
                append_log(&format!("[update] api response read failed: {}", e));
                return;
            }
        };

        let latest_tag = json
            .get("tag_name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim_start_matches('v')
            .to_string();
        let current = env!("CARGO_PKG_VERSION");
        append_log(&format!("[update] latest_tag='{}' current='{}'", latest_tag, current));

        let latest_ver = semver::Version::parse(&latest_tag).ok();
        let current_ver = semver::Version::parse(current).ok();
        if latest_ver.is_none() || current_ver.is_none() {
            append_log("[update] semver parse failed");
            return;
        }

        if latest_ver.unwrap() <= current_ver.unwrap() {
            append_log("[update] already up-to-date, no action");
            return;
        }

        if let Some(assets) = json.get("assets").and_then(|a| a.as_array()) {
            for asset in assets {
                if let (Some(name), Some(dl)) = (
                    asset.get("name").and_then(|n| n.as_str()),
                    asset.get("browser_download_url").and_then(|u| u.as_str()),
                ) {
                    let lname = name.to_lowercase();
                    // Accept any Windows executable asset (.exe). Previously we only accepted
                    // names containing "setup" or "installer", which may miss plain exe builds.
                    if lname.ends_with(".exe") {
                        append_log(&format!("[update] found asset='{}' url='{}'", name, dl));
                        let info = UpdateAvailable {
                            asset_name: name.to_string(),
                            download_url: dl.to_string(),
                        };
                        let mutex = UPDATE_AVAILABLE.get_or_init(|| Mutex::new(None));
                        if let Ok(mut guard) = mutex.lock() {
                            *guard = Some(info.clone());
                        }
                        append_log(&format!("[update] queued update for version {}", latest_tag));
                        break;
                    }
                }
            }
        }
    });
}
