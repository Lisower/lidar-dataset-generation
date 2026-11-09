//! Headless terrain generation and an optional Bevy preview.
//! Rendering uses the already existing renderers of the other generator crates.
mod viewer;

use std::path::PathBuf;

use bevy::prelude::*;
use bevy_utils::cameras::CameraPlugin;
use bevy_utils::preset_selector::PresetSelectorPlugin;
use generation_core::presets::{PresetStore, load_preset_config};
use generation_core::{GenerationControlsPlugin, PointCloudRenderPlugin, Seed};
use terrains_generator::{TerrainPreset, TerrainPresetData, try_generate_from_data};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let selector = preset_selector_plugin("terrains");
    let store = PresetStore::discover("terrains", &selector.presets_root);
    if args.iter().any(|arg| arg == "--list") {
        let mut names: std::collections::BTreeSet<String> =
            TerrainPreset::ALL.iter().map(|p| p.name().into()).collect();
        names.extend(store.presets.iter().map(|p| p.id.clone()));
        for name in names {
            println!("{name}");
        }
        return;
    }
    if args.iter().any(|arg| arg == "--help") {
        println!(
            "terrains-generator [preset] [seed] [--headless] [--output cloud.xyz]\n--list: встроенные и файловые пресеты; Ctrl+S: выбор в окне"
        );
        return;
    }
    let mut positional = Vec::new();
    let mut output = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--headless" => {}
            "--output" => {
                output = Some(
                    iter.next()
                        .unwrap_or_else(|| {
                            eprintln!("После --output нужен путь .xyz");
                            std::process::exit(2)
                        })
                        .clone(),
                );
            }
            flag if flag.starts_with("--") => {
                eprintln!("Неизвестный параметр: {flag}");
                std::process::exit(2);
            }
            value => positional.push(value),
        }
    }
    let name = positional.first().copied().unwrap_or("field");
    let preset = TerrainPreset::from_name(name).unwrap_or(TerrainPreset::Custom);
    let meta = store.presets.iter().find(|p| p.id == name).or_else(|| {
        (preset != TerrainPreset::Custom)
            .then(|| store.presets.iter().find(|p| p.kind == preset.name()))
            .flatten()
    });
    if preset == TerrainPreset::Custom && meta.is_none() {
        eprintln!("Неизвестный пресет: {name}. Используйте --list");
        std::process::exit(2);
    }
    let seed_number: u64 = positional
        .get(1)
        .map_or(Ok(42), |value| value.parse())
        .unwrap_or_else(|_| {
            eprintln!("Seed должен быть неотрицательным целым числом");
            std::process::exit(2)
        });
    let seed = Seed::new(seed_number);
    if args.iter().any(|arg| arg == "--headless") || output.is_some() {
        let data = match meta {
            Some(meta) => load_preset_config::<TerrainPresetData>(meta).unwrap_or_else(|| {
                eprintln!("Ошибка конфигурации {}", meta.config_path.display());
                std::process::exit(2)
            }),
            None => TerrainPresetData::from_preset(preset),
        };
        let scene =
            try_generate_from_data(&data, seed, meta.map(|_| selector.presets_root.as_path()))
                .unwrap_or_else(|error| {
                    eprintln!("{error}");
                    std::process::exit(2)
                });
        println!(
            "{} · seed {} · деревья {} · кусты {} · трава {} (вариантов {}) · здания {} · генерация {:.1} мс",
            name,
            seed,
            scene.trees.len(),
            scene.bushes.len(),
            scene.grass.len(),
            scene.grass.variants.len(),
            scene.buildings.len(),
            scene.generation_time.as_secs_f64() * 1000.0
        );
        if let Some(path) = output {
            generation_core::point_clouds::export::write_xyz(&scene.point_cloud(), path)
                .unwrap_or_else(|error| {
                    eprintln!("Ошибка экспорта: {error}");
                    std::process::exit(1)
                });
        }
        return;
    }
    App::new()
        .add_plugins((
            DefaultPlugins.set(AssetPlugin {
                file_path: generation_core::presets::resolve_dir(
                    "assets",
                    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"),
                )
                .to_string_lossy()
                .into_owned(),
                ..default()
            }),
            PointCloudRenderPlugin,
            GenerationControlsPlugin { initial_seed: seed },
            CameraPlugin::new(Vec3::new(0.0, 4.0, 0.0), 145.0),
            viewer::TerrainViewerPlugin {
                initial_preset: preset,
                initial_id: meta.map(|meta| meta.id.clone()),
            },
            selector,
        ))
        .run();
    return;
}

/// Плагин окна выбора пресетов категории `category`.
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
