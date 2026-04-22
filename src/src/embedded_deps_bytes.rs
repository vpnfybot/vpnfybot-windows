//! Управление встроенными зависимостями с использованием include_bytes!

use std::error::Error;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use super::app_dirs;

include!(concat!(env!("OUT_DIR"), "/embedded_deps_manifest.rs"));

pub struct EmbeddedDep {
    pub file_name: &'static str,
    pub compressed: &'static [u8],
    pub original_size: usize,
}

/// Встроенные данные зависимостей
pub struct EmbeddedDeps {
    pub proxybridge_core: EmbeddedDep,
    pub proxybridge_cli: EmbeddedDep,
    pub wireproxy: EmbeddedDep,
    pub windivert: EmbeddedDep,
    pub windivert_sys: EmbeddedDep,
}

impl EmbeddedDeps {
    /// Получить встроенные зависимости
    pub fn get() -> &'static Self {
        &EMBEDDED_DEPS
    }
}

/// Глобальный экземпляр встроенных зависимостей
static EMBEDDED_DEPS: EmbeddedDeps = EmbeddedDeps {
    proxybridge_core: EmbeddedDep {
        file_name: "ProxyBridgeCore.dll",
        compressed: include_bytes!(concat!(env!("OUT_DIR"), "/ProxyBridgeCore.dll.zst")),
        original_size: PROXYBRIDGECORE_DLL_ORIGINAL_SIZE,
    },
    proxybridge_cli: EmbeddedDep {
        file_name: "ProxyBridge_CLI.exe",
        compressed: include_bytes!(concat!(env!("OUT_DIR"), "/ProxyBridge_CLI.exe.zst")),
        original_size: PROXYBRIDGE_CLI_EXE_ORIGINAL_SIZE,
    },
    wireproxy: EmbeddedDep {
        file_name: "wireproxy.exe",
        compressed: include_bytes!(concat!(env!("OUT_DIR"), "/wireproxy.exe.zst")),
        original_size: WIREPROXY_EXE_ORIGINAL_SIZE,
    },
    windivert: EmbeddedDep {
        file_name: "WinDivert.dll",
        compressed: include_bytes!(concat!(env!("OUT_DIR"), "/WinDivert.dll.zst")),
        original_size: WINDIVERT_DLL_ORIGINAL_SIZE,
    },
    windivert_sys: EmbeddedDep {
        file_name: "WinDivert64.sys",
        compressed: include_bytes!(concat!(env!("OUT_DIR"), "/WinDivert64.sys.zst")),
        original_size: WINDIVERT64_SYS_ORIGINAL_SIZE,
    },
};

static EXTRACTED_DEPS_CACHE: OnceLock<Mutex<Option<ExtractedDeps>>> = OnceLock::new();

/// Получить путь к директории для извлеченных зависимостей
pub fn embedded_deps_dir() -> PathBuf {
    app_dirs::get_deps_dir().join("@vpnfybot-windows")
}

fn embedded_dep_matches(path: &PathBuf, expected_size: usize) -> bool {
    match fs::metadata(path) {
        Ok(metadata) => metadata.len() == expected_size as u64,
        Err(_) => false,
    }
}

fn extracted_deps_match(extracted: &ExtractedDeps) -> bool {
    let deps = EmbeddedDeps::get();

    embedded_dep_matches(&extracted.proxybridge_cli, deps.proxybridge_cli.original_size)
        && embedded_dep_matches(&extracted.proxybridge_core, deps.proxybridge_core.original_size)
        && embedded_dep_matches(&extracted.wireproxy, deps.wireproxy.original_size)
        && embedded_dep_matches(&extracted.windivert, deps.windivert.original_size)
        && embedded_dep_matches(&extracted.windivert_sys, deps.windivert_sys.original_size)
}

/// Извлечь встроенную зависимость в управляемую директорию приложения
pub fn extract_embedded_dep(dep: &EmbeddedDep) -> Result<PathBuf, Box<dyn Error>> {
    let dir = embedded_deps_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }

    let file_path = dir.join(dep.file_name);
    if embedded_dep_matches(&file_path, dep.original_size) {
        return Ok(file_path);
    }

    let decoded = zstd::bulk::decompress(dep.compressed, dep.original_size)?;
    fs::write(&file_path, decoded)?;
    Ok(file_path)
}

/// Извлечь все зависимости
pub fn extract_all_dependencies() -> Result<ExtractedDeps, Box<dyn Error>> {
    let deps = EmbeddedDeps::get();

    let proxybridge_cli = extract_embedded_dep(&deps.proxybridge_cli)?;
    let proxybridge_core = extract_embedded_dep(&deps.proxybridge_core)?;
    let wireproxy = extract_embedded_dep(&deps.wireproxy)?;
    let windivert = extract_embedded_dep(&deps.windivert)?;
    let windivert_sys = extract_embedded_dep(&deps.windivert_sys)?;

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
        let cache = EXTRACTED_DEPS_CACHE.get_or_init(|| Mutex::new(None));

        let mut guard = cache.lock().map_err(|_| {
            io::Error::new(
                io::ErrorKind::Other,
                "embedded dependency cache lock poisoned",
            )
        })?;

        if let Some(existing) = guard.as_ref() {
            if extracted_deps_match(existing) {
                return Ok(existing.clone());
            }

            eprintln!(
                "⚠ Кэш встроенных зависимостей устарел или неполон, выполняется повторное извлечение"
            );
            *guard = None;
        }

        let extracted = extract_all_dependencies()?;

        if !extracted_deps_match(&extracted) {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "not all embedded dependencies were extracted successfully",
            )
            .into());
        }

        *guard = Some(extracted.clone());
        Ok(extracted)
    }
}