//! Диагностика пресетов растений: для каждого пресета (построенного из кода и
//! прочитанного из папок `presets/plants/...`) печатает высоту, размах, число
//! веток/листьев и точек облака. Помогает подбирать параметры пресетов и
//! проверять, что папки пресетов читаются системой (`PresetStore` + RON).
//!
//! Запуск: `cargo run -p plants-generator --example inspect_presets [seed]`

use std::path::PathBuf;

use generation_core::{
    load_preset_config, PresetStore, Seed,
};
use plants_generator::{GenerationMode, PlantConfig, PlantPreset, PlantScene, resolve};

/// Корень каталога пресетов (workspace/presets).
fn presets_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.join("..").join("..").join("presets")
}

fn main() {
    let seed: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);

    println!("--- пресеты, построенные из кода ---");
    println!(
        "{:<18} {:>7} {:>7} {:>8} {:>8} {:>10} {:>8}",
        "пресет", "высота", "размах", "ветки", "листья", "точки", "объекты"
    );
    for preset in PlantPreset::ALL {
        let config = resolve(preset, Seed::new(seed));
        let scene = PlantScene::generate(&config, GenerationMode::Single);
        for p in &scene.plants {
            println!(
                "{:<18} {:>7.1} {:>7.1} {:>8} {:>8} {:>10} {:>8}",
                preset.label(),
                p.plant.height,
                p.plant.extent,
                p.plant.branch_count,
                p.plant.leaf_count,
                p.plant.point_cloud.points.len(),
                scene.plants.len(),
            );
        }
        if scene.plants.is_empty() {
            println!("{:<18} (трава/пучки, tufts={})", preset.label(), scene.tufts.len());
        }
    }

    println!("\n--- пресеты из папок presets/plants (round-trip) ---");
    let store = PresetStore::discover("plants", &presets_root());
    for meta in &store.presets {
        match load_preset_config::<PlantConfig>(meta) {
            Some(mut config) => {
                config.seed = Seed::new(seed);
                let skeleton = plants_generator::PlantSkeleton::build(&config);
                let scene = PlantScene::generate(&config, GenerationMode::Single);
                let height = scene
                    .plants
                    .iter()
                    .map(|p| p.plant.height as i32)
                    .max()
                    .unwrap_or(0);
                println!(
                    "  ok   {:<20} path={:<30} height={:>5} веток={:>7}",
                    meta.id,
                    meta.dir.display(),
                    height,
                    skeleton.segments.len(),
                );
            }
            None => println!("  FAIL {:<20} (config.ron не читается)", meta.id),
        }
    }
}
