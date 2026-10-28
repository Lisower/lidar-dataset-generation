//! Dump-инструмент: записать демо-пресеты зданий в
//! `presets/buildings/<группа>/<id>/` корня workspace.
//!
//! Запуск: `cargo run -p buildings-generator --example dump_presets`
//!
//! Пресеты сгруппированы по типу: `houses` (частные дома) и `multi-storey`
//! (многоэтажки). Каждый пресет — папка с `manifest.ron` (id/имя/описание)
//! и `config.ron` (полная сериализованная [`BuildingConfig`]). Генератор
//! читает `config.ron` и подставляет актуальный seed вместо записанного
//! (`Seed::new(0)`).

use std::fs;
use std::path::PathBuf;

use buildings_generator::generation::{BuildingConfig, BuildingPreset};
use generation_core::Seed;

/// Корень каталога пресетов (workspace/presets).
fn presets_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.join("..").join("..").join("presets")
}

/// id папки пресета (совпадает с kind-тегом, по которому его ищут террейны).
fn building_id(preset: BuildingPreset) -> &'static str {
    match preset {
        BuildingPreset::Rectangular => "rectangular",
        BuildingPreset::LShaped => "l-shaped",
        BuildingPreset::WithAnnex => "annex",
        BuildingPreset::ApartmentBlock => "apartment-block",
        BuildingPreset::Skyscraper => "skyscraper",
    }
}

/// Группа (подпапка категории) пресета: `houses` (частные дома) либо
/// `multi-storey` (многоэтажки).
fn preset_group(preset: BuildingPreset) -> &'static str {
    match preset {
        BuildingPreset::ApartmentBlock | BuildingPreset::Skyscraper => "multi-storey",
        BuildingPreset::Rectangular | BuildingPreset::LShaped | BuildingPreset::WithAnnex => "houses",
    }
}

/// Человекочитаемое имя для манифеста.
fn building_name(preset: BuildingPreset) -> &'static str {
    preset.label()
}

/// Записать один пресет категории (в манифест добавляется `kind`).
fn write_preset(category: &str, rel: &str, name: &str, description: &str, kind: &str, config: &str) {
    let dir = presets_root().join(category).join(rel);
    fs::create_dir_all(&dir).expect("create preset dir");
    let manifest = format!(
        "(\n    id: \"{id}\",\n    name: \"{name}\",\n    description: \"{description}\",\n    kind: \"{kind}\",\n)\n",
        id = rel.split('/').next_back().unwrap_or(rel),
    );
    fs::write(dir.join("manifest.ron"), manifest).expect("write manifest");
    fs::write(dir.join("config.ron"), config).expect("write config");
    println!("wrote {}", dir.display());
}

/// Конфигурация с заданным архетипом и фиксированным «нулевым» seed.
fn with_preset(preset: BuildingPreset) -> BuildingConfig {
    let mut config = BuildingConfig::default();
    config.preset = preset;
    config.seed = Seed::new(0);
    config
}

fn main() {
    for preset in BuildingPreset::ALL {
        let text = ron::ser::to_string_pretty(&with_preset(preset), Default::default())
            .expect("serialize building config");
        let rel = format!("{}/{}", preset_group(preset), building_id(preset));
        write_preset(
            "buildings",
            &rel,
            building_name(preset),
            "",
            building_id(preset),
            &text,
        );
    }

    // Дополнительные «террейновые» суб-пресеты (на них ссылаются terrains).
    let mut cabin = with_preset(BuildingPreset::Rectangular);
    cabin.min_size = 4.0;
    cabin.max_size = 7.0;
    cabin.min_floors = 1;
    cabin.max_floors = 1;
    cabin.roof_pitch_min = 24.0;
    cabin.roof_pitch_max = 42.0;
    let text = ron::ser::to_string_pretty(&cabin, Default::default())
        .expect("serialize cabin config");
    write_preset("buildings", "houses/cabin", "Лесная избушка", "Одноэтажный домик с крутой крышей", "cabin", &text);

    let mut cottage = with_preset(BuildingPreset::WithAnnex);
    cottage.min_size = 5.0;
    cottage.max_size = 9.0;
    cottage.min_floors = 1;
    cottage.max_floors = 2;
    let text = ron::ser::to_string_pretty(&cottage, Default::default())
        .expect("serialize cottage config");
    write_preset("buildings", "houses/cottage", "Дом с пристройкой", "Сельский дом с пристройкой", "cottage", &text);
}
