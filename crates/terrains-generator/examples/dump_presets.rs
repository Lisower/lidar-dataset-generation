//! Dump-инструмент: записать террейновые демо-пресеты в
//! `presets/terrains/<группа>/<id>/` корня workspace.
//!
//! Запуск: `cargo run -p terrains-generator --example dump_presets`
//!
//! Пресеты сгруппированы по происхождению: `nature/water` (природные водные),
//! `nature/dry` (природные сухие) и `anthropogenic` (антропогенные). Каждый
//! пресет — папка с `manifest.ron` (id/имя/описание) и `config.ron`
//! (сериализованный [`TerrainPresetData`] со ссылками на суб-пресеты других
//! категорий). Перед этим должны быть запущены dump-инструменты
//! landscapes/plants/buildings, чтобы суб-пресеты существовали.

use std::fs;
use std::path::PathBuf;

use terrains_generator::{TerrainPreset, TerrainPresetData};

/// Корень каталога пресетов (workspace/presets).
fn presets_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.join("..").join("..").join("presets")
}

/// Человекочитаемое имя террейна для манифеста.
fn terrain_name(preset: TerrainPreset) -> &'static str {
    match preset {
        TerrainPreset::Field => "Поле",
        TerrainPreset::Forest => "Лес",
        TerrainPreset::HillSlope => "Склон холма",
        TerrainPreset::TerracedSlope => "Террасированный склон",
        TerrainPreset::River => "Речка",
        TerrainPreset::Pond => "Пруд",
        TerrainPreset::Coast => "Побережье",
        TerrainPreset::SlopingRiver => "Наклонная река",
        TerrainPreset::Waterfall => "Водопад",
        TerrainPreset::Village => "Село",
        TerrainPreset::Town => "Городок",
        TerrainPreset::ForestCabin => "Лесная избушка",
    }
}

/// Путь каталога пресета относительно `presets/terrains`: группа
/// `nature/water`, `nature/dry` либо `anthropogenic` + id.
fn preset_rel(preset: TerrainPreset) -> String {
    let rel = match preset {
        TerrainPreset::Coast
        | TerrainPreset::Pond
        | TerrainPreset::River
        | TerrainPreset::SlopingRiver
        | TerrainPreset::Waterfall => "nature/water",
        TerrainPreset::Field
        | TerrainPreset::Forest
        | TerrainPreset::ForestCabin
        | TerrainPreset::HillSlope
        | TerrainPreset::TerracedSlope => "nature/dry",
        TerrainPreset::Village | TerrainPreset::Town => "anthropogenic",
    };
    format!("{rel}/{}", preset.name())
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

fn main() {
    let root = presets_root();
    for preset in TerrainPreset::ALL {
        // id суб-пресетов резолвятся по kind из фактических манифестов
        // landscapes/plants/buildings, поэтому конфиг ссылается на реальные
        // пресеты даже если папки переложены по группам.
        let data = TerrainPresetData::from_preset_with(preset, Some(&root));
        let text = ron::ser::to_string_pretty(&data, Default::default())
            .expect("serialize terrain preset data");
        let rel = preset_rel(preset);
        let kind = rel.split('/').next_back().unwrap_or(&rel);
        write_preset("terrains", &rel, terrain_name(preset), "", kind, &text);
    }
}
