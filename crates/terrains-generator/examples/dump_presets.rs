//! Reproducibly write terrain configurations, preserving old preset paths.
use std::{fs, path::PathBuf};
use terrains_generator::{TerrainPreset, TerrainPresetData};
fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../presets");
    for &preset in TerrainPreset::ALL {
        let dir = root
            .join("terrains")
            .join(preset.category())
            .join(preset.name());
        fs::create_dir_all(&dir).expect("create preset directory");
        let description = format!(
            "{}; существующие модели генераторов, детерминированная расстановка на 100×100 м",
            preset.label()
        );
        let manifest = format!(
            "(\n    id: {:?},\n    name: {:?},\n    description: {:?},\n    kind: {:?},\n)\n",
            preset.name(),
            preset.label(),
            description,
            preset.name()
        );
        fs::write(dir.join("manifest.ron"), manifest).expect("write manifest");
        let config = TerrainPresetData::from_preset_with(preset, Some(&root));
        fs::write(
            dir.join("config.ron"),
            ron::ser::to_string_pretty(&config, Default::default()).unwrap(),
        )
        .expect("write config");
        println!("{}", dir.display());
    }
}
