//! Манифест созданных ярлыков: что именно можем удалить.
//!
//! `--remove` удаляет только то, что xtask сам записал: наугад чистить каталог
//! приложений нельзя, там лежат чужие `.desktop`-файлы.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Платформа, под которую создаются ярлыки.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// Linux: `.desktop` в `$XDG_DATA_HOME/applications`.
    Linux,
    /// Windows: `.lnk` в меню «Пуск».
    Windows,
    /// macOS: `.command` в `~/Applications`.
    MacOs,
}

impl Platform {
    /// Платформа текущего процесса.
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Linux
        }
    }

    /// Имя платформы для манифеста.
    pub fn name(self) -> &'static str {
        match self {
            Self::Linux => "linux",
            Self::Windows => "windows",
            Self::MacOs => "macos",
        }
    }
}

/// Источники переменных окружения для выбора каталога данных.
#[derive(Debug, Clone, Default)]
pub struct DataDirInputs {
    /// Платформа.
    pub os: Platform,
    /// `XDG_DATA_HOME`.
    pub xdg_data_home: Option<PathBuf>,
    /// `HOME` или `USERPROFILE`.
    pub home: Option<PathBuf>,
    /// `APPDATA`.
    pub appdata: Option<PathBuf>,
}

impl Default for Platform {
    fn default() -> Self {
        Self::Linux
    }
}

/// Один созданный ярлык.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestEntry {
    /// Ключ приложения (`runner`, `plants`, …).
    pub app: String,
    /// Абсолютный путь к файлу ярлыка.
    pub path: String,
}

/// Манифест созданных ярлыков.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// Платформа, на которой создавались ярлыки.
    #[serde(default)]
    pub platform: String,
    /// Созданные файлы.
    #[serde(default)]
    pub entries: Vec<ManifestEntry>,
}

/// Каталог данных пользователя для манифеста.
///
/// `None`, если ни одна переменная не задана: писать в угаданный домашний
/// каталог нельзя.
pub fn data_dir(inputs: DataDirInputs) -> Option<PathBuf> {
    match inputs.os {
        Platform::Linux => {
            let base = inputs
                .xdg_data_home
                .or_else(|| inputs.home.map(|home| home.join(".local/share")))?;
            Some(base.join("lidar-dataset-generator"))
        }
        Platform::Windows => {
            let base = inputs
                .appdata
                .or_else(|| inputs.home.map(|home| home.join("AppData/Roaming")))?;
            Some(base.join("LiDAR"))
        }
        Platform::MacOs => {
            let home = inputs.home?;
            Some(home.join("Library/Application Support/LiDAR"))
        }
    }
}

/// Прочитать манифест.
pub fn load_manifest(path: &Path) -> Result<Manifest, String> {
    let text = std::fs::read_to_string(path).map_err(|error| {
        format!(
            "файл манифеста ярлыков не найден: {}; сначала создайте ярлыки ({error})",
            path.display()
        )
    })?;
    serde_json::from_str(&text).map_err(|error| {
        format!(
            "манифест ярлыков повреждён: {}; сначала создайте ярлыки ({error})",
            path.display()
        )
    })
}

/// Записать манифест, создав каталоги.
pub fn save_manifest(path: &Path, manifest: &Manifest) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "не удалось создать каталог манифеста ярлыков {}: {error}",
                parent.display()
            )
        })?;
    }
    let text = serde_json::to_string_pretty(manifest)
        .map_err(|error| format!("не удалось сериализовать манифест ярлыков: {error}"))?;
    std::fs::write(path, text).map_err(|error| {
        format!(
            "не удалось записать манифест ярлыков {}: {error}",
            path.display()
        )
    })
}

#[cfg(test)]
mod tests {
    fn fixture(name: &str) -> std::path::PathBuf {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target")
            .join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn manifest_roundtrip() {
        let root = fixture("xtask-manifest");
        let path = root.join("shortcuts.json");
        let manifest = super::Manifest {
            platform: "linux".into(),
            entries: vec![super::ManifestEntry {
                app: "runner".into(),
                path: root.join("a.desktop").to_string_lossy().into_owned(),
            }],
        };
        super::save_manifest(&path, &manifest).unwrap();
        let loaded = super::load_manifest(&path).unwrap();
        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.entries[0].app, "runner");
        assert_eq!(loaded.platform, "linux");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn load_fails_with_readable_message() {
        let root = fixture("xtask-manifest-missing");
        let error = super::load_manifest(&root.join("nope.json")).unwrap_err();
        assert!(error.contains("ярлык"), "{error}");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn save_creates_parent_directories() {
        let root = fixture("xtask-manifest-nested");
        let path = root.join("a/b/shortcuts.json");
        super::save_manifest(&path, &super::Manifest::default()).unwrap();
        assert!(path.is_file());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn data_dir_prefers_xdg_data_home() {
        let inputs = super::DataDirInputs {
            os: super::Platform::Linux,
            xdg_data_home: Some("/xdg".into()),
            home: Some("/home/u".into()),
            appdata: None,
        };
        assert_eq!(
            super::data_dir(inputs),
            Some(std::path::PathBuf::from("/xdg/lidar-dataset-generator"))
        );
    }

    #[test]
    fn data_dir_falls_back_to_appdata_on_windows() {
        let inputs = super::DataDirInputs {
            os: super::Platform::Windows,
            xdg_data_home: None,
            home: Some("C:/Users/u".into()),
            appdata: Some("C:/Users/u/AppData/Roaming".into()),
        };
        assert_eq!(
            super::data_dir(inputs),
            Some(std::path::PathBuf::from(
                "C:/Users/u/AppData/Roaming/LiDAR"
            ))
        );
    }

    #[test]
    fn data_dir_uses_home_on_every_platform_without_env() {
        let inputs = super::DataDirInputs {
            os: super::Platform::Linux,
            xdg_data_home: None,
            home: Some("/home/u".into()),
            appdata: None,
        };
        assert_eq!(
            super::data_dir(inputs),
            Some(std::path::PathBuf::from(
                "/home/u/.local/share/lidar-dataset-generator"
            ))
        );
    }

    #[test]
    fn data_dir_sits_in_application_support_on_macos() {
        let inputs = super::DataDirInputs {
            os: super::Platform::MacOs,
            xdg_data_home: None,
            home: Some("/Users/u".into()),
            appdata: None,
        };
        assert_eq!(
            super::data_dir(inputs),
            Some(std::path::PathBuf::from(
                "/Users/u/Library/Application Support/LiDAR"
            ))
        );
    }

    #[test]
    fn data_dir_is_none_without_any_home() {
        let inputs = super::DataDirInputs {
            os: super::Platform::Linux,
            xdg_data_home: None,
            home: None,
            appdata: None,
        };
        assert_eq!(super::data_dir(inputs), None);
    }
}
