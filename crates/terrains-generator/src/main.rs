//! Headless terrain generation and an optional Bevy preview.
//! Rendering uses the already existing renderers of the other generator crates.
mod viewer;

use std::path::PathBuf;

use bevy::prelude::*;
use bevy_utils::cameras::CameraPlugin;
use bevy_utils::preset_selector::PresetSelectorPlugin;
use generation_core::{GenerationControlsPlugin, Seed};
use terrains_generator::TerrainPreset;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|arg| arg == "--list") {
        for preset in TerrainPreset::ALL {
            println!("{}", preset.name());
        }
        return;
    }

    let positional: Vec<&str> = args
        .iter()
        .filter(|arg| !arg.starts_with("--"))
        .map(String::as_str)
        .collect();

    let name = positional.first().copied().unwrap_or("field");
    let Some(preset) = TerrainPreset::from_name(name) else {
        eprintln!("Unknown preset: {name}. Use --list to see available presets.");
        std::process::exit(2);
    };
    let seed_number: u64 = match positional.get(1) {
        Some(value) => match value.parse() {
            Ok(value) => value,
            Err(_) => {
                eprintln!("Seed must be a non-negative integer, got: {value}");
                std::process::exit(2);
            }
        },
        None => 42,
    };
    let seed = Seed::new(seed_number);

    App::new()
        .add_plugins((
            DefaultPlugins,
            GenerationControlsPlugin { initial_seed: seed },
            CameraPlugin::new(Vec3::new(0.0, 4.0, 0.0), 145.0),
            viewer::TerrainViewerPlugin {
                initial_preset: preset,
            },
            preset_selector_plugin("terrains"),
        ))
        .run();
    return;
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
