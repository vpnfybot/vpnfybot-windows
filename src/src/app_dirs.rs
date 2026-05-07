//! Управление директориями приложения
//! Создает и управляет структурой папок рядом с установленным exe

use std::error::Error;
use std::env;
use std::fs;
use std::path::PathBuf;

/// Структура для управления всеми директориями приложения
#[derive(Debug, Clone)]
pub struct AppDirs {
    /// Корневая папка приложения: директория установленного exe
    pub root: PathBuf,
    /// Папка для логов: <install_dir>/logs
    pub logs: PathBuf,
    /// Папка для разрешений брандмауэра: <install_dir>/permissions
    pub permissions: PathBuf,
    /// Папка для конфигов: <install_dir>/configs
    pub configs: PathBuf,
    /// Папка для кэша: <install_dir>/cache
    pub cache: PathBuf,
    /// Папка для зависимостей: <install_dir>/deps
    pub deps: PathBuf,
}

impl AppDirs {
    /// Инициализировать все директории приложения
    pub fn init() -> Result<Self, Box<dyn Error>> {
        let root = get_app_root();
        
        let dirs = AppDirs {
            root: root.clone(),
            logs: root.join("logs"),
            permissions: root.join("permissions"),
            configs: root.join("configs"),
            cache: root.join("cache"),
            deps: root.join("deps"),
        };
        
        // Создаем все директории
        dirs.create_all()?;
        
        // Создаем файлы инициализации
        dirs.create_init_files()?;
        
        Ok(dirs)
    }
    
    /// Создать все необходимые директории
    fn create_all(&self) -> Result<(), Box<dyn Error>> {
        for dir in &[
            &self.root,
            &self.logs,
            &self.permissions,
            &self.configs,
            &self.cache,
            &self.deps,
        ] {
            if !dir.exists() {
                fs::create_dir_all(dir)?;
                eprintln!("✓ Создана папка: {}", dir.display());
            }
        }
        Ok(())
    }
    
    /// Создать файлы инициализации
    fn create_init_files(&self) -> Result<(), Box<dyn Error>> {
        // Создаем файл информации приложения
        let info_file = self.root.join("app.info");
        if !info_file.exists() {
            // Use compile-time package version so app.info reflects Cargo.toml
            let info = format!(
                "vpnfybot-windows Application\n\
                 Version: {}\n\
                 Created: {}\n\
                 Install Root: {}\n",
                env!("CARGO_PKG_VERSION"),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                self.root.display()
            );
            fs::write(&info_file, info)?;
            eprintln!("✓ Создана папка приложения: {}", self.root.display());
        }

        Ok(())
    }

    /// Полностью сбросить временные runtime-файлы при старте приложения.
    /// Конфиги пользователя и установленные зависимости не затрагиваются.
    pub fn reset_runtime_state(&self) -> Result<(), Box<dyn Error>> {
        for dir in [&self.logs, &self.permissions, &self.cache] {
            if dir.exists() {
                fs::remove_dir_all(dir)?;
            }
            fs::create_dir_all(dir)?;
        }

        if !self.deps.exists() {
            fs::create_dir_all(&self.deps)?;
        }

        self.remove_legacy_readmes()?;
        self.create_fresh_runtime_files()?;
        let deleted_global = self.cleanup_global_temp_artifacts()?;

        eprintln!("✓ Runtime-временные файлы очищены и пересозданы");
        if deleted_global > 0 {
            eprintln!("✓ Удалено временных файлов обновления из системного temp: {}", deleted_global);
        }

        Ok(())
    }

    fn remove_legacy_readmes(&self) -> Result<(), Box<dyn Error>> {
        for dir in [&self.logs, &self.permissions, &self.configs, &self.cache, &self.deps] {
            let readme = dir.join("README.txt");
            if readme.exists() {
                fs::remove_file(readme)?;
            }
        }
        Ok(())
    }

    fn create_fresh_runtime_files(&self) -> Result<(), Box<dyn Error>> {
        for log_name in ["proxybridge.log", "update_check.log"] {
            fs::write(self.logs.join(log_name), "")?;
        }
        Ok(())
    }

    fn cleanup_global_temp_artifacts(&self) -> Result<u32, Box<dyn Error>> {
        let mut deleted_count = 0;
        let temp_dir = env::temp_dir();
        let legacy_root = temp_dir.join("vpnfybot-windows");

        if legacy_root != self.root && legacy_root.exists() {
            if fs::remove_dir_all(&legacy_root).is_ok() {
                deleted_count += 1;
            }
        }

        if let Ok(entries) = fs::read_dir(temp_dir) {
            for entry in entries {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(_) => continue,
                };

                let path = entry.path();
                let file_name = entry.file_name().to_string_lossy().to_ascii_lowercase();
                let is_update_script = file_name.starts_with("vpnfy_update_") && file_name.ends_with(".ps1");
                let is_downloaded_update = file_name == "vpnfybot-windows.exe";

                if !(is_update_script || is_downloaded_update) {
                    continue;
                }

                if path.is_file() && fs::remove_file(&path).is_ok() {
                    deleted_count += 1;
                }
            }
        }

        Ok(deleted_count)
    }
    
    /// Получить путь к файлу лога
    #[allow(dead_code)]
    pub fn get_log_file(&self, app_name: &str) -> PathBuf {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.logs.join(format!("{}_{}.log", app_name, timestamp))
    }
    
    /// Получить путь к файлу разрешений
    #[allow(dead_code)]
    pub fn get_permission_file(&self, app_name: &str) -> PathBuf {
        self.permissions.join(format!("{}_permissions.txt", app_name))
    }
    
}

/// Получить корневую папку приложения
fn get_app_root() -> PathBuf {
    env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|dir| dir.to_path_buf()))
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

/// Получить путь к приложению
#[allow(dead_code)]
pub fn get_app_root_dir() -> PathBuf {
    get_app_root()
}

/// Получить папку логов
#[allow(dead_code)]
pub fn get_logs_dir() -> PathBuf {
    get_app_root().join("logs")
}

/// Получить папку разрешений
#[allow(dead_code)]
pub fn get_permissions_dir() -> PathBuf {
    get_app_root().join("permissions")
}

/// Получить папку конфигов
#[allow(dead_code)]
pub fn get_configs_dir() -> PathBuf {
    get_app_root().join("configs")
}

/// Получить папку кэша
#[allow(dead_code)]
pub fn get_cache_dir() -> PathBuf {
    get_app_root().join("cache")
}

/// Получить основную папку для зависимостей
#[allow(dead_code)]
pub fn get_deps_dir() -> PathBuf {
    get_app_root().join("deps")
}

/// Логирование с префиксом
#[allow(dead_code)]
pub fn log_info(message: &str) {
    eprintln!("[VPNFy] {}", message);
}

#[allow(dead_code)]
pub fn log_error(message: &str) {
    eprintln!("[VPNFy ERROR] {}", message);
}

#[allow(dead_code)]
pub fn log_warning(message: &str) {
    eprintln!("[VPNFy WARN] {}", message);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_dirs_init() {
        let dirs = AppDirs::init();
        assert!(dirs.is_ok());
    }
}
