//! Вспомогательные функции для тестов раннера.

use std::path::PathBuf;

/// Каталог `presets/` воркспейса (содержит папки категорий).
pub fn repo_presets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../presets")
}
