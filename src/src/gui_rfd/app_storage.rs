use super::*;

fn ensure_managed_dir(path: PathBuf) -> PathBuf {
    let _ = fs::create_dir_all(&path);
    path
}

pub(super) fn managed_logs_dir() -> PathBuf {
    ensure_managed_dir(app_dirs::get_logs_dir())
}

fn managed_configs_dir() -> PathBuf {
    ensure_managed_dir(app_dirs::get_configs_dir())
}

pub(super) fn managed_cache_dir() -> PathBuf {
    ensure_managed_dir(app_dirs::get_cache_dir())
}

pub(super) fn managed_updates_dir() -> PathBuf {
    ensure_managed_dir(managed_cache_dir().join("updates"))
}

fn get_config_storage_path() -> Option<PathBuf> {
    let mut dir = managed_configs_dir();
    dir.push("last_conf.txt");
    Some(dir)
}

pub(super) fn load_saved_conf_path() -> Option<String> {
    let path = get_config_storage_path()?;
    let content = fs::read_to_string(path).ok()?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub(super) fn save_conf_path(conf: &str) {
    if let Some(path) = get_config_storage_path() {
        let _ = fs::write(path, conf);
    }
}

pub(super) fn save_selected_processes(processes: &[String]) {
    if let Some(path) = get_config_storage_path() {
        let mut process_file = path.clone();
        process_file.set_file_name("selected_processes.txt");
        let content = processes.join("\n");
        let _ = fs::write(process_file, content);
    }
}

pub(super) fn load_selected_processes() -> Vec<String> {
    if let Some(path) = get_config_storage_path() {
        let mut process_file = path.clone();
        process_file.set_file_name("selected_processes.txt");
        match fs::read_to_string(process_file) {
            Ok(content) => content.lines().map(|s| s.to_string()).collect(),
            Err(_) => Vec::new(),
        }
    } else {
        Vec::new()
    }
}

pub(super) fn save_selected_sites(sites: &[String]) {
    if let Some(path) = get_config_storage_path() {
        let mut site_file = path.clone();
        site_file.set_file_name("selected_sites.txt");
        let content = sites.join("\r\n");
        let _ = fs::write(site_file, content);
    }
}

pub(super) fn load_selected_sites() -> Vec<String> {
    if let Some(path) = get_config_storage_path() {
        let mut site_file = path.clone();
        site_file.set_file_name("selected_sites.txt");
        match fs::read_to_string(site_file) {
            Ok(content) => content
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(String::from)
                .collect(),
            Err(_) => Vec::new(),
        }
    } else {
        Vec::new()
    }
}

pub(super) fn save_proxy_mode(selected_apps_only: bool) {
    if let Some(path) = get_config_storage_path() {
        let mut mode_file = path.clone();
        mode_file.set_file_name("proxy_mode.txt");
        let mode = if selected_apps_only { "selected" } else { "system" };
        let _ = fs::write(mode_file, mode);
    }
}

pub(super) fn load_proxy_mode() -> bool {
    if let Some(path) = get_config_storage_path() {
        let mut mode_file = path.clone();
        mode_file.set_file_name("proxy_mode.txt");
        if let Ok(content) = fs::read_to_string(mode_file) {
            let trimmed = content.trim();
            return trimmed != "system";
        }
    }
    false
}

pub(super) fn save_language(language: Language) {
    if let Some(path) = get_config_storage_path() {
        let mut lang_file = path.clone();
        lang_file.set_file_name("language.txt");
        let lang_code = match language {
            Language::En => "en",
            Language::Ru => "ru",
        };
        let _ = fs::write(lang_file, lang_code);
    }
}

pub(super) fn load_language() -> Language {
    if let Some(path) = get_config_storage_path() {
        let mut lang_file = path.clone();
        lang_file.set_file_name("language.txt");
        if let Ok(content) = fs::read_to_string(lang_file) {
            match content.trim().to_lowercase().as_str() {
                "ru" => return Language::Ru,
                _ => return Language::En,
            }
        }
    }
    Language::En
}

pub(super) fn delete_app_storage_dirs() {
    let managed_dirs = [
        app_dirs::get_logs_dir(),
        app_dirs::get_permissions_dir(),
        app_dirs::get_configs_dir(),
        app_dirs::get_cache_dir(),
        app_dirs::get_deps_dir(),
    ];

    for dir in managed_dirs {
        let _ = fs::remove_dir_all(dir);
    }
}