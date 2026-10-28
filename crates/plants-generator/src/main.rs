//! Приложение-генератор растений: окно Bevy с визуализацией и управлением.
//!
//! Собран из общих плагинов:
//! - [`GenerationControlsPlugin`] — горячие клавиши (`V`, `R`, `Shift + R`);
//! - [`CameraPlugin`](bevy_utils::cameras::CameraPlugin) — орбитальная и
//!   свободная камеры (`Tab`);
//! - [`PlantsPlugin`](plants_generator::PlantsPlugin) — сцена и объекты,
//!   а также клавиша режима генерации (`M`);
//! - [`PresetSelectorPlugin`](bevy_utils::preset_selector::PresetSelectorPlugin)
//!   — окно выбора демо-пресетов (`Ctrl+S`).
//!
//! Для headless-использования этой же генерации обратитесь к крейту как к
//! библиотеке ([`plants_generator::generation`]).

use std::path::PathBuf;

use bevy::prelude::*;
use bevy_utils::cameras::CameraPlugin;
use bevy_utils::preset_selector::PresetSelectorPlugin;
use generation_core::GenerationControlsPlugin;
use plants_generator::PlantsPlugin;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            GenerationControlsPlugin::default(),
            CameraPlugin::new(Vec3::new(0.0, 3.0, 0.0), 60.0),
            PlantsPlugin::default(),
            preset_selector_plugin("plants"),
        ))
        .run();
}

/// Плагин окна выбора пресетов категории `category`.
fn preset_selector_plugin(category: &'static str) -> PresetSelectorPlugin {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.join("..").join("..");
    PresetSelectorPlugin {
        category,
        presets_root: workspace_root.join("presets"),
        settings_dir: manifest_dir.join("settings"),
    }
}
