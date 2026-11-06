//! Каталог пресетов для интерфейса, валидации рецепта и разрешения конфигов.
//!
//! Источник истины — каталог `presets/` в репозитории.
//! Здания и растения разрешаются только через [`PresetStore`] +
//! [`load_preset_config`]: у `plants_generator::resolve` есть пресеты, которые
//! паникуют при отсутствии RON-файла, а у зданий нет обратного отображения
//! из manifest-`kind` в перечисление. Поэтому здесь нет ни одного `unwrap`
//! на файлах: нечитаемый конфиг просто оставляет поле пустым, а решение
//! «так нельзя генерировать» принимает валидация рецепта.
//!
//! Каталог хранит свой корень, поэтому вся дальнейшая работа (композиция
//! сцены, статистика `run.json`) идёт от него и не зависит от переменных
//! окружения или глобального состояния.

use std::path::{Path, PathBuf};

use generation_core::presets::{PresetMeta, PresetStore, load_preset_config};
use plants_generator::{PlantConfig, PlantType};
use serde::Serialize;

/// Один пресет в виде, пригодном для UI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetDto {
    /// id из `manifest.ron`.
    pub id: String,
    /// `kind` из `manifest.ron`; для ландшафтов — вход выбора архетипа.
    pub kind: String,
    /// Русское имя из манифеста.
    pub name: String,
    /// Описание из манифеста.
    pub description: String,
    /// Только для растений: `"tree" | "bush" | "grass"`.
    pub plant_type: Option<String>,
}

/// Каталог пресетов по категориям вместе с корнем, из которого он прочитан.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    /// Пресеты ландшафтов.
    pub landscapes: Vec<PresetDto>,
    /// Пресеты зданий.
    pub buildings: Vec<PresetDto>,
    /// Пресеты растений (деревья, кусты, трава).
    pub plants: Vec<PresetDto>,
    /// Корень каталога (каталог, содержащий папки категорий).
    ///
    /// В JSON не пишется: путь нужен только коду, а UI получает его отдельно
    /// в `CatalogInfo`.
    #[serde(skip)]
    pub presets_root: PathBuf,
}

impl Catalog {
    /// Пресеты категории по её имени (`"landscapes" | "buildings" | "plants"`).
    pub fn category(&self, category: &str) -> &[PresetDto] {
        match category {
            "landscapes" => &self.landscapes,
            "buildings" => &self.buildings,
            "plants" => &self.plants,
            _ => &[],
        }
    }

    /// Индекс пресетов категории по её имени.
    pub fn store(&self, category: &str) -> PresetStore {
        PresetStore::discover(category, &self.presets_root)
    }

    /// Метаданные пресета категории (с путём к `config.ron`).
    ///
    /// Пресет считается доступным, только если он есть в категории каталога:
    /// так отфильтрованный или очищенный каталог действительно перестаёт
    /// разрешать пресеты, а не обходит собственный список.
    pub fn meta(&self, category: &str, id: &str) -> Option<PresetMeta> {
        let known = self.category(category).iter().any(|p| p.id == id);
        if !known {
            return None;
        }
        self.store(category).meta(id).cloned()
    }
}

/// Тип растения строкой для UI.
fn plant_type_name(kind: PlantType) -> String {
    match kind {
        PlantType::Tree => "tree",
        PlantType::Bush => "bush",
        PlantType::Grass => "grass",
    }
    .to_string()
}

/// Прочитать каталог `presets_root` (каталог, содержащий папки категорий).
///
/// Отсутствующий или пустой каталог даёт пустой [`Catalog`], а не панику:
/// диагностику показывает UI, а запуск генерации отсекает валидация рецепта.
pub fn load_catalog(presets_root: &Path) -> Catalog {
    Catalog {
        landscapes: category_presets(presets_root, "landscapes"),
        buildings: category_presets(presets_root, "buildings"),
        plants: category_presets(presets_root, "plants"),
        presets_root: presets_root.to_path_buf(),
    }
}

/// Пресеты одной категории; для растений дополнительно читается тип.
fn category_presets(presets_root: &Path, category: &str) -> Vec<PresetDto> {
    let store = PresetStore::discover(category, presets_root);
    store
        .presets
        .iter()
        .map(|meta| PresetDto {
            id: meta.id.clone(),
            kind: meta.kind.clone(),
            name: meta.name.clone(),
            description: meta.description.clone(),
            plant_type: if category == "plants" {
                plant_type_of(meta)
            } else {
                None
            },
        })
        .collect()
}

/// Тип растения из `config.ron` пресета; `None` — конфиг не читается.
fn plant_type_of(meta: &PresetMeta) -> Option<String> {
    load_preset_config::<PlantConfig>(meta).map(|cfg| plant_type_name(cfg.preset.plant_type()))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::dataset::test_support::repo_presets;

    #[test]
    fn catalog_lists_real_presets_with_plant_types() {
        let catalog = load_catalog(&repo_presets());
        assert!(catalog.landscapes.iter().any(|p| p.id == "plain"));
        assert!(catalog.buildings.iter().any(|p| p.id == "rectangular"));
        let tree = catalog
            .plants
            .iter()
            .find(|p| p.id == "deciduous-tree")
            .unwrap();
        assert_eq!(tree.plant_type.as_deref(), Some("tree"));
        let grass = catalog
            .plants
            .iter()
            .find(|p| p.id == "sparse-grass")
            .unwrap();
        assert_eq!(grass.plant_type.as_deref(), Some("grass"));
        assert!(
            catalog
                .plants
                .iter()
                .find(|p| p.id == "bush")
                .unwrap()
                .plant_type
                .is_some()
        );
    }

    #[test]
    fn missing_presets_dir_yields_empty_catalog_without_panic() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/catalog-missing");
        let _ = std::fs::remove_dir_all(&dir);
        let catalog = load_catalog(&dir);
        assert!(
            catalog.landscapes.is_empty()
                && catalog.buildings.is_empty()
                && catalog.plants.is_empty()
        );
    }

    #[test]
    fn catalogue_carries_its_root_and_resolves_preset_paths() {
        let root = repo_presets();
        let catalog = load_catalog(&root);
        assert_eq!(catalog.presets_root, root);
        let meta = catalog.meta("landscapes", "plain").unwrap();
        assert!(meta.config_path.is_file(), "{}", meta.config_path.display());
    }
}
