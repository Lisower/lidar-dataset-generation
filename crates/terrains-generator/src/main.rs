//! Headless terrain generation and an optional Bevy preview.
//! Rendering uses the already existing renderers of the other generator crates.
mod viewer;

use bevy::prelude::*;
use bevy_utils::cameras::CameraPlugin;
use generation_core::{GenerationControlsPlugin, Seed};
use terrains_generator::{TerrainPreset, generate};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|arg| arg == "--list") {
        for preset in TerrainPreset::ALL {
            println!("{}", preset.name());
        }
        return;
    }

    let preview = args.iter().any(|arg| arg == "--preview");
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

    if preview {
        println!(
            "Preview: {} | seed: {} | [P] preset [R] new [Shift+R] same [V] view [Tab] camera",
            preset.name(),
            seed
        );
        App::new()
            .add_plugins((
                DefaultPlugins,
                GenerationControlsPlugin { initial_seed: seed },
                CameraPlugin::new(Vec3::new(0.0, 4.0, 0.0), 145.0),
                viewer::TerrainViewerPlugin {
                    initial_preset: preset,
                },
            ))
            .run();
        return;
    }

    let scene = generate(preset, seed);
    println!(
        "Preset: {} | seed: {} | {} x {} m",
        preset.name(),
        seed,
        scene.landscape.heightfield.size_m,
        scene.landscape.heightfield.size_m
    );
    println!(
        "Trees: {} | Bushes: {} | Grass tufts: {} | Buildings: {} | Road corridors: {}",
        scene.trees.len(),
        scene.bushes.len(),
        scene.grass.len(),
        scene.buildings.len(),
        scene.roads.len()
    );
    println!(
        "Total objects: {} | Water: {}",
        scene.object_count(),
        scene.landscape.has_water
    );
}
