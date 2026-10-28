//! Dump-инструмент: разрешить все архетипы [`LandscapePreset`] и записать их
//! как демо-пресеты в `presets/landscapes/<группа>/<id>/` корня workspace.
//!
//! Запуск: `cargo run -p landscapes-generator --example dump_presets`
//!
//! Пресеты сгруппированы по типу рельефа: группы `water` (водные: реки,
//! пруды, берега, водопады) и `dry` (сухие: равнины, склоны, холмы, каньоны).
//! Каждый пресет — папка с `manifest.ron` (id/имя/описание) и `config.ron`
//! (полная сериализованная [`LandscapeConfig`]). Генератор читает `config.ron`
//! и подставляет актуальный seed вместо записанного (`Seed::new(0)`).

use std::fs;
use std::path::PathBuf;

use generation_core::Seed;
use landscapes_generator::{LandscapePreset, resolve};

/// Корень каталога пресетов (workspace/presets).
fn presets_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.join("..").join("..").join("presets")
}

/// Путь каталога пресета относительно `presets/landscapes`: группа `water`
/// (водные) либо `dry` (сухие) + id.
fn preset_rel(preset: LandscapePreset) -> String {
    let group = match preset {
        LandscapePreset::River
        | LandscapePreset::Pond
        | LandscapePreset::Shore
        | LandscapePreset::SlopingRiver
        | LandscapePreset::Waterfalls => "water",
        LandscapePreset::Plain
        | LandscapePreset::Slope
        | LandscapePreset::PlainSlope
        | LandscapePreset::MultiLevel
        | LandscapePreset::CanyonLedge
        | LandscapePreset::Gorge
        | LandscapePreset::Hills => "dry",
    };
    format!("{group}/{}", preset.kind())
}

/// Записать один пресет категории (в манифест добавляется `kind` — тег,
/// по которому программы находят пресет независимо от имени папки).
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
    for preset in LandscapePreset::ALL {
        let config = resolve(preset, Seed::new(0));
        let text = ron::ser::to_string_pretty(&config, Default::default())
            .expect("serialize landscape config");
        let rel = preset_rel(preset);
        let id = rel.split('/').next_back().unwrap_or(&rel);
        write_preset("landscapes", &rel, preset.label(), "", id, &text);
    }
}
