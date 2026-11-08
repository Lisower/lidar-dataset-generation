//! Абсолютный каталог ассетов приложения.
//!
//! Bevy ищет ассеты относительно текущего каталога, поэтому запуск из ярлыка
//! (рабочий каталог произвольный) оставлял окно без шрифта. Приоритет:
//! переменная окружения `LIDAR_ASSETS_DIR`, затем `<каталог exe>/<relative>`
//! (упакованный вид), затем `<каталог крейта>/<relative>` (запуск из исходников).

use std::path::{Path, PathBuf};

use bevy::asset::AssetPlugin;

/// Абсолютный путь к каталогу ассетов.
///
/// `manifest_dir` — каталог крейта (`env!("CARGO_MANIFEST_DIR")`),
/// `relative` — путь к ассетам относительно него (`"assets"`),
/// `env_override` — значение `LIDAR_ASSETS_DIR`, если оно задано.
///
/// Возвращает первый существующий каталог из списка кандидатов, а если ни один
/// не существует — `manifest_dir.join(relative)`, чтобы ошибка была видна
/// в сообщении Bevy, а не превращалась в пустой путь.
pub fn assets_dir(manifest_dir: &Path, relative: &str, env_override: Option<&Path>) -> PathBuf {
    let manifest_candidate = manifest_dir.join(relative);

    if let Some(dir) = env_override {
        return dir.to_path_buf();
    }

    let exe_candidate = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .map(|dir| dir.join(relative));

    if let Some(dir) = exe_candidate.filter(|dir| dir.is_dir()) {
        return dir;
    }

    if manifest_candidate.is_dir() {
        return manifest_candidate;
    }

    manifest_candidate
}

/// Плагин ассетов с абсолютным путём к каталогу ассетов.
///
/// `LIDAR_ASSETS_DIR` читается здесь, поэтому приложение не дублирует разбор
/// переменной окружения.
pub fn asset_plugin(manifest_dir: &Path, relative: &str) -> AssetPlugin {
    let from_env = std::env::var_os("LIDAR_ASSETS_DIR").map(PathBuf::from);
    let dir = assets_dir(manifest_dir, relative, from_env.as_deref());
    AssetPlugin {
        file_path: dir.to_string_lossy().into_owned(),
        ..Default::default()
    }
}

/// Плагин ассетов для текущего крейта: `asset_plugin!("assets")`.
#[macro_export]
macro_rules! asset_plugin {
    ($relative:expr) => {
        $crate::assets::asset_plugin(::std::path::Path::new(env!("CARGO_MANIFEST_DIR")), $relative)
    };
}

#[cfg(test)]
mod tests {
    fn fixture(name: &str) -> std::path::PathBuf {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target")
            .join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("assets/fonts")).unwrap();
        std::fs::write(dir.join("assets/fonts/Roboto-Regular.ttf"), b"x").unwrap();
        dir
    }

    #[test]
    fn env_override_wins() {
        let root = fixture("assets-env");
        let override_dir = root.join("assets");
        let dir = super::assets_dir(&root, "assets", Some(&override_dir));
        assert_eq!(dir, override_dir);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn falls_back_to_manifest_dir_when_no_env() {
        let root = fixture("assets-manifest");
        let dir = super::assets_dir(&root, "assets", None);
        assert_eq!(dir, root.join("assets"));
        assert!(dir.join("fonts/Roboto-Regular.ttf").is_file());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn missing_manifest_assets_still_returns_a_path() {
        let root = fixture("assets-missing");
        let dir = super::assets_dir(&root, "does-not-exist", None);
        assert_eq!(dir, root.join("does-not-exist"));
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn plugin_uses_resolved_path() {
        let root = fixture("assets-plugin");
        let plugin = super::asset_plugin(&root, "assets");
        assert_eq!(plugin.file_path, root.join("assets").to_string_lossy());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
