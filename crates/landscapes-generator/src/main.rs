//! Приложение-генератор ландшафта: окно Bevy с визуализацией и управлением.
//!
//! Собран из общих плагинов:
//! - [`GenerationControlsPlugin`] — горячие клавиши (`V`, `R`, `Shift + R`);
//! - [`CameraPlugin`](bevy_utils::cameras::CameraPlugin) — орбитальная и
//!   свободная камеры (`Tab`);
//! - [`LandscapePlugin`](landscapes_generator::LandscapePlugin) — сцена и
//!   панель информации;
//! - [`PresetSelectorPlugin`](bevy_utils::preset_selector::PresetSelectorPlugin)
//!   — окно выбора демо-пресетов (`Ctrl+S`).
//!
//! Для headless-использования этой же генерации обратитесь к крейту как к
//! библиотеке ([`landscapes_generator::generation`]).

use std::path::PathBuf;

use bevy::prelude::*;
use bevy_utils::cameras::CameraPlugin;
use bevy_utils::preset_selector::PresetSelectorPlugin;
use generation_core::GenerationControlsPlugin;
use landscapes_generator::LandscapePlugin;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            GenerationControlsPlugin::default(),
            CameraPlugin::new(Vec3::new(0.0, 4.0, 0.0), 90.0),
            LandscapePlugin::default(),
            preset_selector_plugin("landscapes"),
        ))
        .run();
}

/// Плагин окна выбора пресетов категории `category`.
///
/// Пресеты лежат в `presets/<категория>` в корне workspace; выбор хранится
/// в `settings/<крейта>`.
fn preset_selector_plugin(category: &'static str) -> PresetSelectorPlugin {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.join("..").join("..");
    // Пресеты ищутся рядом с exe/в рабочем каталоге (переносимая сборка), а
    // путь сборочной машины остаётся запасным для `cargo run`.
    let roots = generation_core::presets::resolve_data_roots(
        category,
        workspace_root.join(generation_core::PRESETS_DIR),
        manifest_dir.join("settings"),
    );
    PresetSelectorPlugin {
        category,
        presets_root: roots.presets_root,
        settings_dir: roots.settings_dir,
    }
}
