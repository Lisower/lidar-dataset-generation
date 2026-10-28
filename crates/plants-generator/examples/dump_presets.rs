//! Dump-инструмент: разрешить все архетипы [`PlantPreset`] и записать их как
//! демо-пресеты в группированные папки `presets/plants/<группа>/<id>/` корня
//! workspace.
//!
//! Запуск: `cargo run -p plants-generator --example dump_presets`
//!
//! Пресеты сгруппированы по типу растения: деревья (`high`) — с подгруппами
//! `coniferous` (ель, ель высокая, сосна), `decidious` (лиственное дерево,
//! ива) и `palms` (пальмы); кусты (`medium`); трава (`low`). Группа — папка
//! с `manifest.ron` (`is_group: true`), пресет — папка с `manifest.ron`
//! (id/имя/описание/kind) и `config.ron` (полная сериализованная
//! [`PlantConfig`]).
//!
//! По умолчанию уже существующие пресеты **не перезаписываются** (только
//! добавляются недостающие и досоздаются манифесты групп). Чтобы перегенериро-
//! вать все пресеты заново, запустите с флагом `--force`.
//!
//! Программы ищут пресет по `kind` из манифеста (`"tree"`, `"bush"`,
//! `"grass"`, либо уточняющий слаг), а не по имени папки, поэтому папки можно
//! переименовывать и перекладывать по группам вручную — приложение увидит
//! изменения сразу (главное, чтобы RON были оформлены правильно).

use std::fs;
use std::path::{Path, PathBuf};

use generation_core::Seed;
use plants_generator::{PlantPreset, PlantType, resolve};

/// Корень каталога пресетов (workspace/presets).
fn presets_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.join("..").join("..").join("presets")
}

/// Верхняя группа пресетов по типу растения: папка, её id и имя для манифеста.
fn group_of(preset: PlantPreset) -> (&'static str, &'static str, &'static str) {
    match preset.plant_type() {
        PlantType::Tree => ("high", "high", "Деревья"),
        PlantType::Bush => ("medium", "medium", "Кустарники"),
        PlantType::Grass => ("low", "low", "Трава"),
    }
}

/// Подгруппа дерева (папка, её id и имя), либо `None` для кустов/травы.
fn subgroup_of(preset: PlantPreset) -> Option<(&'static str, &'static str, &'static str)> {
    match preset {
        PlantPreset::Spruce | PlantPreset::Fir | PlantPreset::Pine => {
            Some(("coniferous", "coniferous", "Хвойные"))
        }
        PlantPreset::DeciduousTree | PlantPreset::Willow => {
            Some(("decidious", "decidious", "Лиственные"))
        }
        PlantPreset::Palm | PlantPreset::PalmCoconuts => {
            Some(("palms", "palms", "Пальмовые"))
        }
        _ => None,
    }
}

/// Путь каталога пресета относительно `presets/plants`: `<группа>/<id>`
/// либо `<группа>/<подгруппа>/<id>` для деревьев с подгруппой.
fn preset_dir(preset: PlantPreset) -> PathBuf {
    let (group, _, _) = group_of(preset);
    let id = preset.ron_id();
    match subgroup_of(preset) {
        Some((subgroup, _, _)) => PathBuf::from(group).join(subgroup).join(id),
        None => PathBuf::from(group).join(id),
    }
}

/// Записать манифест группы (`is_group: true`) в `presets/plants/<путь>/`.
/// Досоздаёт недостающие (вложенные) группы.
fn write_group(rel: &Path, id: &str, name: &str) {
    let dir = presets_root().join("plants").join(rel);
    fs::create_dir_all(&dir).expect("create group dir");
    let manifest = format!(
        "(\n    id: \"{id}\",\n    name: \"{name}\",\n    description: \"\",\n    is_group: true,\n)\n"
    );
    let path = dir.join("manifest.ron");
    if path.exists() {
        return;
    }
    fs::write(path, manifest).expect("write group manifest");
}

/// Записать пресет категории `plants` внутри группы (или подгруппы).
fn write_preset_in_group(rel: &Path, id: &str, name: &str, kind: &str, config: &str) {
    let dir = presets_root().join("plants").join(rel);
    fs::create_dir_all(&dir).expect("create preset dir");
    let manifest = format!(
        "(\n    id: \"{id}\",\n    name: \"{name}\",\n    description: \"\",\n    kind: \"{kind}\",\n)\n"
    );
    fs::write(dir.join("manifest.ron"), manifest).expect("write manifest");
    fs::write(dir.join("config.ron"), config).expect("write config");
    println!("wrote {}", dir.display());
}

fn main() {
    let force = std::env::args().any(|a| a == "--force");

    for preset in PlantPreset::ALL {
        let rel = preset_dir(preset);
        let dir = presets_root().join("plants").join(&rel);
        if dir.exists() && !force {
            println!("skipped {} (already exists)", dir.display());
            continue;
        }

        let config = resolve(preset, Seed::new(0));
        let text = ron::ser::to_string_pretty(&config, Default::default())
            .expect("serialize plant config");

        // Манифесты групп досоздаём при необходимости (идемпотентно).
        let (top, top_id, top_name) = group_of(preset);
        write_group(Path::new(top), top_id, top_name);
        match subgroup_of(preset) {
            Some((sub, sub_id, sub_name)) => {
                write_group(&Path::new(top).join(sub), sub_id, sub_name);
            }
            None => {}
        }

        write_preset_in_group(
            &rel,
            preset.ron_id(),
            preset.label(),
            preset.ron_id(),
            &text,
        );
    }
}
