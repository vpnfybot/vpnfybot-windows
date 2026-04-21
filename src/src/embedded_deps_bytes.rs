//! Управление встроенными зависимостями с использованием include_bytes!

use std::error::Error;
use std::fs;
use std::path::PathBuf;

use super::app_dirs;

/// Встроенные данные зависимостей
pub struct EmbeddedDeps {
    pub proxybridge_core: &'static [u8],
    pub proxybridge_cli: &'static [u8],
    pub wireproxy: &'static [u8],
    pub windivert: &'static [u8],
    pub windivert_sys: &'static [u8],
}

impl EmbeddedDeps {
    /// Получить встроенные зависимости
    pub fn get() -> &'static Self {
        &EMBEDDED_DEPS
    }
}

/// Глобальный экземпляр встроенных зависимостей
static EMBEDDED_DEPS: EmbeddedDeps = EmbeddedDeps {
    proxybridge_core: include_bytes!("../embedded_deps/ProxyBridgeCore.dll"),
    proxybridge_cli: include_bytes!("../embedded_deps/ProxyBridge_CLI.exe"),
    wireproxy: include_bytes!("../embedded_deps/wireproxy.exe"),
    windivert: include_bytes!("../embedded_deps/WinDivert.dll"),
    windivert_sys: include_bytes!("../embedded_deps/WinDivert64.sys"),
};

/// Получить путь к директории для извлеченных зависимостей
pub fn embedded_deps_dir() -> PathBuf {
    app_dirs::get_deps_dir().join("@vpnfybot-windows")
}

fn embedded_dep_matches(path: &PathBuf, data: &[u8]) -> bool {
    match fs::metadata(path) {
        Ok(metadata) => metadata.len() == data.len() as u64,
        Err(_) => false,
    }
}

/// Извлечь встроенную зависимость в управляемую директорию приложения
pub fn extract_embedded_dep(name: &str, data: &[u8]) -> Result<PathBuf, Box<dyn Error>> {
    let dir = embedded_deps_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }

    let file_path = dir.join(name);
    
    fs::write(&file_path, data)?;
    Ok(file_path)
}

/// Извлечь все зависимости
pub fn extract_all_dependencies() -> Result<ExtractedDeps, Box<dyn Error>> {
    let deps = EmbeddedDeps::get();
    
    let proxybridge_cli = extract_embedded_dep("ProxyBridge_CLI.exe", deps.proxybridge_cli)?;
    let proxybridge_core = extract_embedded_dep("ProxyBridgeCore.dll", deps.proxybridge_core)?;
    let wireproxy = extract_embedded_dep("wireproxy.exe", deps.wireproxy)?;
    let windivert = extract_embedded_dep("WinDivert.dll", deps.windivert)?;
    let windivert_sys = extract_embedded_dep("WinDivert64.sys", deps.windivert_sys)?;

    Ok(ExtractedDeps {
        proxybridge_cli,
        proxybridge_core,
        wireproxy,
        windivert,
        windivert_sys,
    })
}

/// Структура с путями к извлеченным зависимостям
#[derive(Debug, Clone)]
pub struct ExtractedDeps {
    pub proxybridge_cli: PathBuf,
    #[allow(dead_code)]
    pub proxybridge_core: PathBuf,
    pub wireproxy: PathBuf,
    #[allow(dead_code)]
    pub windivert: PathBuf,
    #[allow(dead_code)]
    pub windivert_sys: PathBuf,
}

impl ExtractedDeps {
    /// Получить пути к зависимостям, извлекая их при необходимости
    pub fn get() -> Result<Self, Box<dyn Error>> {
        let deps = EmbeddedDeps::get();
        let dir = embedded_deps_dir();
        if !dir.exists() {
            return extract_all_dependencies();
        }

        // Проверяем, что все файлы существуют
        let proxybridge_cli = dir.join("ProxyBridge_CLI.exe");
        let proxybridge_core = dir.join("ProxyBridgeCore.dll");
        let wireproxy = dir.join("wireproxy.exe");
        let windivert = dir.join("WinDivert.dll");
        let windivert_sys = dir.join("WinDivert64.sys");

        let all_exist = embedded_dep_matches(&proxybridge_cli, deps.proxybridge_cli)
            && embedded_dep_matches(&proxybridge_core, deps.proxybridge_core)
            && embedded_dep_matches(&wireproxy, deps.wireproxy)
            && embedded_dep_matches(&windivert, deps.windivert)
            && embedded_dep_matches(&windivert_sys, deps.windivert_sys);

        if all_exist {
            Ok(ExtractedDeps {
                proxybridge_cli,
                proxybridge_core,
                wireproxy,
                windivert,
                windivert_sys,
            })
        } else {
            // Если какие-то файлы отсутствуют, переизвлекаем все
            extract_all_dependencies()
        }
    }
}