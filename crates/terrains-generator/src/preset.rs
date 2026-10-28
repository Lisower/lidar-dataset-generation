//! Presets for composite 100 x 100 m environments.
use std::path::Path;

use generation_core::presets::PresetStore;
use landscapes_generator::LandscapePreset;
use serde::{Deserialize, Serialize};

use generation_core::Seed;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TerrainPreset {
    Field,
    Forest,
    HillSlope,
    TerracedSlope,
    River,
    Pond,
    Coast,
    SlopingRiver,
    Waterfall,
    Village,
    Town,
    ForestCabin,
}

impl TerrainPreset {
    pub const ALL: [Self; 12] = [
        Self::Field,
        Self::Forest,
        Self::HillSlope,
        Self::TerracedSlope,
        Self::River,
        Self::Pond,
        Self::Coast,
        Self::SlopingRiver,
        Self::Waterfall,
        Self::Village,
        Self::Town,
        Self::ForestCabin,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Field => "field",
            Self::Forest => "forest",
            Self::HillSlope => "hill-slope",
            Self::TerracedSlope => "terraced-slope",
            Self::River => "river",
            Self::Pond => "pond",
            Self::Coast => "coast",
            Self::SlopingRiver => "sloping-river",
            Self::Waterfall => "waterfall",
            Self::Village => "village",
            Self::Town => "town",
            Self::ForestCabin => "forest-cabin",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "field" | "поле" => Some(Self::Field),
            "forest" | "лес" => Some(Self::Forest),
            "hill-slope" | "slope" | "склон" => Some(Self::HillSlope),
            "terraced-slope" | "terraces" | "террасы" => Some(Self::TerracedSlope),
            "river" | "река" | "речка" => Some(Self::River),
            "pond" | "пруд" => Some(Self::Pond),
            "coast" | "shore" | "побережье" => Some(Self::Coast),
            "sloping-river" | "наклонная-речка" => Some(Self::SlopingRiver),
            "waterfall" | "водопад" => Some(Self::Waterfall),
            "village" | "село" => Some(Self::Village),
            "town" | "городок" => Some(Self::Town),
            "forest-cabin" | "избушка" => Some(Self::ForestCabin),
            _ => None,
        }
    }

    pub const fn landscape(self) -> LandscapePreset {
        match self {
            Self::Field | Self::Forest | Self::Village | Self::Town | Self::ForestCabin => {
                LandscapePreset::Plain
            }
            Self::HillSlope => LandscapePreset::Slope,
            Self::TerracedSlope => LandscapePreset::MultiLevel,
            Self::River => LandscapePreset::River,
            Self::Pond => LandscapePreset::Pond,
            Self::Coast => LandscapePreset::Shore,
            Self::SlopingRiver => LandscapePreset::SlopingRiver,
            Self::Waterfall => LandscapePreset::Waterfalls,
        }
    }

    pub const fn amounts(self) -> (usize, usize, usize, usize) {
        // (trees, bushes, grass tufts, buildings) — target counts, not guaranteed.
        match self {
            Self::Field => (6, 16, 260, 0),
            Self::Forest => (35, 38, 200, 0),
            Self::HillSlope => (10, 18, 120, 0),
            Self::TerracedSlope => (14, 15, 100, 0),
            Self::River => (14, 24, 190, 0),
            Self::Pond => (12, 23, 190, 0),
            Self::Coast => (10, 18, 160, 0),
            Self::SlopingRiver => (12, 20, 125, 0),
            Self::Waterfall => (11, 18, 90, 0),
            Self::Village => (8, 14, 130, 8),
            Self::Town => (5, 9, 70, 14),
            Self::ForestCabin => (31, 30, 160, 1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn presets_are_complete_and_parseable() {
        assert_eq!(TerrainPreset::ALL.len(), 12);
        for preset in TerrainPreset::ALL {
            assert_eq!(TerrainPreset::from_name(preset.name()), Some(preset));
        }
    }
    #[test]
    fn data_roundtrip_and_defaults_consistent() {
        for preset in TerrainPreset::ALL {
            let data = TerrainPresetData::from_preset(preset);
            // Без каталога пресетов id — канонические kind-слаги (запасной
            // вариант; при наличии `presets/` они резолвятся из манифестов).
            assert_eq!(data.landscape_id, preset.landscape().kind());
            assert_eq!(data.name, preset.name());
            assert_eq!(data.trees > 0, preset.amounts().0 > 0);
            assert_eq!(data.bushes > 0, preset.amounts().1 > 0);
            assert_eq!(data.buildings > 0, preset.amounts().3 > 0);
            // Здание ссылается на kind только когда его количество > 0.
            assert_eq!(data.building_id.is_empty(), preset.amounts().3 == 0);
            // Круговая сериализация в RON читается обратно.
            let text = ron::ser::to_string_pretty(&data, Default::default()).unwrap();
            let back: TerrainPresetData = ron::from_str(&text).unwrap();
            assert_eq!(back.name, data.name);
            assert_eq!(back.landscape_id, data.landscape_id);
            assert_eq!(back.tree_id, data.tree_id);
            assert_eq!(back.max_slope_deg, data.max_slope_deg);
        }
    }
}

/// Данные террейнового пресета для демо — сериализуются в
/// `presets/terrains/<папка>/config.ron`.
///
/// Хранит id суб-пресетов других категорий (`presets/landscapes/...`,
/// `presets/plants/...`, `presets/buildings/...`) и целевые счётчики;
/// генерация подгружает конфиги суб-пресетов из файлов, а при отсутствии
/// (`presets_root = None` или файла нет) — откатывается на архетипы.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TerrainPresetData {
    /// Пометка seed для dump-инструмента; при генерации заменяется на актуальный.
    #[serde(default)]
    pub seed: Seed,
    /// Имя пресета (`field`, `village`, …).
    pub name: String,
    /// id суб-пресета ландшафта в `presets/landscapes/`.
    pub landscape_id: String,
    /// id суб-пресета дерева в `presets/plants/`.
    pub tree_id: String,
    /// id суб-пресета куста в `presets/plants/`.
    pub bush_id: String,
    /// id суб-пресета травы в `presets/plants/`.
    pub grass_id: String,
    /// id суб-пресета здания в `presets/buildings/` (пусто — зданий нет).
    pub building_id: String,
    /// Целевое число деревьев.
    pub trees: usize,
    /// Целевое число кустов.
    pub bushes: usize,
    /// Целевое число пучков травы.
    pub grass: usize,
    /// Целевое число зданий.
    pub buildings: usize,
    /// Максимальный уклон под фундамент, градусы.
    #[serde(default = "default_max_slope")]
    pub max_slope_deg: f32,
}

const fn default_max_slope() -> f32 {
    10.0
}

impl TerrainPresetData {
    /// Составить данные пресета из встроенного архетипа без каталога пресетов
    /// (headless-режим и обратная совместимость): id суб-пресетов — канонические
    /// `kind`-слаги. При генерации без пресетов они не используются для поиска
    /// файлов (всё откатывается на архетипы).
    pub fn from_preset(preset: TerrainPreset) -> Self {
        Self::from_preset_with(preset, None)
    }

    /// Составить данные пресета, разрешив id суб-пресетов по фактическому
    /// каталогу `presets/` (если `presets_root` задан): для каждого вида
    /// берётся пресет с нужным `kind` из манифестов (см. [`PresetStore::by_kind`]),
    /// поэтому папки можно переименовывать и перекладывать по группам — id
    /// подхватится из манифеста. Если каталога нет или пресета такого вида нет —
    /// запасной вариант: канонический kind-слаг.
    pub fn from_preset_with(preset: TerrainPreset, presets_root: Option<&Path>) -> Self {
        let (trees, bushes, grass, buildings) = preset.amounts();
        // Вид травы: на застроенной/полевой местности — редкая, иначе густая.
        let grass_kind = if matches!(
            preset,
            TerrainPreset::Field | TerrainPreset::Village | TerrainPreset::Town
        ) {
            "sparse-grass"
        } else {
            "tall-grass"
        };
        // Вид здания по архетипу (пусто — зданий нет).
        let building_kind = match preset {
            TerrainPreset::Village => Some("cottage"),
            TerrainPreset::Town => Some("apartment-block"),
            TerrainPreset::ForestCabin => Some("cabin"),
            _ => None,
        };
        let landscape_kind = preset.landscape().kind();

        Self {
            seed: Seed::default(),
            name: preset.name().to_string(),
            landscape_id: resolve_sub_id("landscapes", landscape_kind, presets_root)
                .unwrap_or_else(|| landscape_kind.to_string()),
            tree_id: resolve_sub_id("plants", "deciduous-tree", presets_root)
                .unwrap_or_else(|| "deciduous-tree".to_string()),
            bush_id: resolve_sub_id("plants", "bush", presets_root)
                .unwrap_or_else(|| "bush".to_string()),
            grass_id: resolve_sub_id("plants", grass_kind, presets_root)
                .unwrap_or_else(|| grass_kind.to_string()),
            building_id: match building_kind {
                Some(kind) => resolve_sub_id("buildings", kind, presets_root)
                    .unwrap_or_else(|| kind.to_string()),
                None => String::new(),
            },
            trees,
            bushes,
            grass,
            buildings,
            max_slope_deg: default_max_slope(),
        }
    }

    /// Подставить актуальный seed (метка «ландшафта» ставится во время
    /// генерации самими суб-пресетами).
    pub fn with_seed(mut self, seed: Seed) -> Self {
        self.seed = seed;
        self
    }
}

/// Найти id пресета с нужным `kind` в категории `presets/<категория>/…`,
/// используя манифесты. `None` — каталог недоступен или пресета нет.
fn resolve_sub_id(
    category: &str,
    kind: &str,
    presets_root: Option<&Path>,
) -> Option<String> {
    let root = presets_root?;
    PresetStore::discover(category, root)
        .by_kind(kind)
        .map(|meta| meta.id.clone())
}
