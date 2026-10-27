//! Приложение-генератор зданий: окно Bevy с визуализацией и управлением.
//!
//! Собран из общих плагинов:
//! - [`GenerationControlsPlugin`] — горячие клавиши (`V`, `R`, `Shift + R`);
//! - [`CameraPlugin`](bevy_utils::cameras::CameraPlugin) — орбитальная и
//!   свободная камеры (`Tab`);
//! - [`BuildingPlugin`](buildings_generator::BuildingPlugin) — сцена и объекты.
//!
//! Для headless-использования этой же генерации обратитесь к крейту как к
//! библиотеке ([`buildings_generator::generation`]).

use bevy::prelude::*;
use bevy_utils::cameras::CameraPlugin;
use buildings_generator::BuildingPlugin;
use generation_core::GenerationControlsPlugin;

fn main() {
    use buildings_generator::generation::{BuildingConfig, BuildingPreset};
    let mut config = BuildingConfig::default();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        if args.len() != 2 || args[0] != "--preset" {
            eprintln!("Usage: buildings-generator [--preset rectangular|l-shaped|annex|apartment|skyscraper]");
            std::process::exit(2);
        }
        config.preset = match args[1].as_str() {
            "rectangular" => BuildingPreset::Rectangular,
            "l-shaped" => BuildingPreset::LShaped,
            "annex" => BuildingPreset::WithAnnex,
            "apartment" => BuildingPreset::ApartmentBlock,
            "skyscraper" => BuildingPreset::Skyscraper,
            _ => {
                eprintln!("Unknown preset: {}", args[1]);
                std::process::exit(2);
            }
        };
    }
    App::new()
        .add_plugins((
            DefaultPlugins,
            GenerationControlsPlugin::default(),
            CameraPlugin::new(Vec3::new(0.0, 3.0, 0.0), 60.0),
            BuildingPlugin { config },
        ))
        .run();
}
