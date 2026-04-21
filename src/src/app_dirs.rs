//! Управление директориями приложения
//! Создает и управляет структурой папок рядом с установленным exe

use std::error::Error;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

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
            let info = format!(
                "@vpnfybot-windows Application\n\
                 Version: 3.2.2\n\
                 Created: {}\n\
                 Install Root: {}\n",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                self.root.display()
            );
            fs::write(&info_file, info)?;
            eprintln!("✓ Создана папка приложения: {}", self.root.display());
        }
        
        // Создаем файл .gitkeep в каждой папке (чтобы папки были видны в git)
        for dir in &[
            &self.logs,
            &self.permissions,
            &self.configs,
            &self.cache,
        ] {
            let gitkeep = dir.join(".gitkeep");
            if !gitkeep.exists() {
                fs::write(&gitkeep, "")?;
            }
        }
        
        // Создаем README файлы с описанием
        self.create_readme(&self.logs, "Логи приложения (wireproxy и ProxyBridge)")?;
        self.create_readme(&self.permissions, "Файлы разрешений брандмауэра")?;
        self.create_readme(&self.configs, "Конфиги WireGuard и приложения")?;
        self.create_readme(&self.cache, "Кэш и временные данные")?;
        
        Ok(())
    }
    
    /// Создать README файл в директории
    fn create_readme(&self, dir: &Path, description: &str) -> Result<(), Box<dyn Error>> {
        let readme = dir.join("README.txt");
        if !readme.exists() {
            let content = format!(
                "# {}\n\n\
                 {}\n\n\
                 Эта папка управляется приложением @vpnfybot-windows.\n\
                 Не удаляйте файлы вручную, если приложение работает.\n",
                dir.file_name().unwrap_or_default().to_string_lossy(),
                description
            );
            fs::write(&readme, content)?;
        }
        Ok(())
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
    
    /// Очистить старые логи (старше N дней)
    pub fn cleanup_old_logs(&self, days: u64) -> Result<u32, Box<dyn Error>> {
        let mut deleted_count = 0;
        
        if let Ok(entries) = fs::read_dir(&self.logs) {
            let now = std::time::SystemTime::now();
            let cutoff = now - std::time::Duration::from_secs(days * 86400);
            
            for entry in entries {
                if let Ok(entry) = entry {
                    if let Ok(metadata) = entry.metadata() {
                        if let Ok(modified) = metadata.modified() {
                            if modified < cutoff {
                                let _ = fs::remove_file(entry.path());
                                deleted_count += 1;
                            }
                        }
                    }
                }
            }
        }
        
        if deleted_count > 0 {
            eprintln!("✓ Удалено старых логов: {}", deleted_count);
        }
        
        Ok(deleted_count)
    }
}

/// Получить корневую папку приложения
fn get_app_root() -> PathBuf {
    // Store all application folders (logs, configs, cache, deps) inside
    // the system/user temporary directory to avoid touching installation
    // location. This uses `env::temp_dir()` which on Windows typically
    // resolves to `C:\Users\<user>\AppData\Local\Temp`.
    let mut temp_root = env::temp_dir();
    temp_root.push("vpnfybot-windows");
    temp_root
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
