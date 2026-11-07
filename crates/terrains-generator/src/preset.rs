//! Data-driven composition of existing generator presets. Legacy single ids remain valid.
use crate::sources::PresetSource;
use generation_core::Seed;
use landscapes_generator::LandscapePreset;
use serde::{Deserialize, Serialize};
use std::path::Path;

macro_rules! terrain_presets {
    ($( $variant:ident => ($id:literal, $label:literal, $landscape:ident, $category:literal) ),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum TerrainPreset { $($variant,)+ Custom }
        impl TerrainPreset {
            pub const ALL: &'static [Self] = &[$(Self::$variant,)+];
            pub const fn name(self) -> &'static str { match self { $(Self::$variant => $id,)+ Self::Custom => "custom" } }
            pub const fn label(self) -> &'static str { match self { $(Self::$variant => $label,)+ Self::Custom => "Пользовательская территория" } }
            pub const fn landscape(self) -> LandscapePreset { match self { $(Self::$variant => LandscapePreset::$landscape,)+ Self::Custom => LandscapePreset::Plain } }
            pub const fn category(self) -> &'static str { match self { $(Self::$variant => $category,)+ Self::Custom => "nature/dry" } }
            pub fn from_name(name: &str) -> Option<Self> {
                let lower = name.to_lowercase();
                Self::ALL.iter().copied().find(|p| p.name() == lower || p.label().to_lowercase() == lower)
                    .or_else(|| match lower.as_str() {
                        "slope" | "склон" => Some(Self::HillSlope), "terraces" | "террасы" => Some(Self::TerracedSlope),
                        "река" | "речка" => Some(Self::River), "shore" | "побережье" => Some(Self::Coast),
                        "село" => Some(Self::Village), "избушка" => Some(Self::ForestCabin),
                        "поле" => Some(Self::Field), "лес" => Some(Self::Forest), "пруд" => Some(Self::Pond), "водопад" => Some(Self::Waterfall),
                        "наклонная-речка" => Some(Self::SlopingRiver), _ => None,
                    })
            }
        }
    }
}

terrain_presets! {
    Field => ("field", "Поле с редкими деревьями", Plain, "nature/dry"),
    OpenField => ("open-field", "Открытое поле", Plain, "nature/dry"),
    Forest => ("forest", "Лиственный лес", Plain, "nature/dry"),
    ForestSlope => ("forest-slope", "Лиственный лес на склоне", Slope, "nature/dry"),
    ForestTerraces => ("forest-terraces", "Лиственный лес на террасах", MultiLevel, "nature/dry"),
    ConiferForest => ("conifer-forest", "Хвойный лес", Plain, "nature/dry"),
    ConiferSlope => ("conifer-slope", "Хвойный лес на склоне", Slope, "nature/dry"),
    ConiferTerraces => ("conifer-terraces", "Хвойный лес на террасах", MultiLevel, "nature/dry"),
    Woodland => ("woodland", "Редколесье", Plain, "nature/dry"),
    WoodlandSlope => ("woodland-slope", "Редколесье на склоне", Slope, "nature/dry"),
    WoodlandTerraces => ("woodland-terraces", "Редколесье на террасах", MultiLevel, "nature/dry"),
    HillSlope => ("hill-slope", "Склон без деревьев", Slope, "nature/dry"),
    TerracedSlope => ("terraced-slope", "Террасы без деревьев", MultiLevel, "nature/dry"),
    River => ("river", "Речка с деревьями", River, "nature/water"),
    RiverOpen => ("river-open", "Речка без деревьев", River, "nature/water"),
    Pond => ("pond", "Пруд с деревьями", Pond, "nature/water"),
    PondOpen => ("pond-open", "Пруд без деревьев", Pond, "nature/water"),
    Coast => ("coast", "Берег реки или озера", Shore, "nature/water"),
    CoastOpen => ("coast-open", "Берег без деревьев", Shore, "nature/water"),
    Beach => ("beach", "Морской пляж", Shore, "nature/water"),
    SlopingRiver => ("sloping-river", "Наклонная река", SlopingRiver, "nature/water"),
    Waterfall => ("waterfall", "Водопад с деревьями", Waterfalls, "nature/water"),
    WaterfallOpen => ("waterfall-open", "Водопад без деревьев", Waterfalls, "nature/water"),
    Village => ("village", "Деревня", Plain, "anthropogenic"),
    Town => ("town", "Городок", Plain, "anthropogenic"),
    SkyscraperCity => ("skyscraper-city", "Город с небоскрёбами", Plain, "anthropogenic"),
    ApartmentCity => ("apartment-city", "Город с многоэтажками", Plain, "anthropogenic"),
    ResidentialCity => ("residential-city", "Город с частными домами", Plain, "anthropogenic"),
    ForestCabin => ("forest-cabin", "Лесная избушка", Plain, "nature/dry"),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub enum Settlement {
    #[default]
    None,
    Village,
    Grid,
    Cabin,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlacementSettings {
    pub settlement: Settlement,
    pub street_width: f32,
    pub setback: f32,
    pub tree_clearance: f32,
    pub plant_slope_deg: f32,
    pub cluster_probability: f32,
    pub cluster_radius: f32,
    pub clearing_radius: f32,
    pub waterfall_clearance: f32,
}
impl Default for PlacementSettings {
    fn default() -> Self {
        Self {
            settlement: Settlement::None,
            street_width: 6.0,
            setback: 2.5,
            tree_clearance: 0.6,
            plant_slope_deg: 35.0,
            cluster_probability: 0.78,
            cluster_radius: 17.0,
            clearing_radius: 0.0,
            waterfall_clearance: 4.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub enum GrassDensity {
    #[default]
    Legacy,
    None,
    Sparse,
    Medium,
    Dense,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GrassSettings {
    pub density: GrassDensity,
    pub variants_per_preset: usize,
    pub min_scale: f32,
    pub max_scale: f32,
    pub min_water_distance: f32,
}
impl Default for GrassSettings {
    fn default() -> Self {
        Self {
            density: GrassDensity::Legacy,
            variants_per_preset: 4,
            min_scale: 0.8,
            max_scale: 1.2,
            min_water_distance: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TerrainPresetData {
    #[serde(default)]
    pub seed: Seed,
    pub name: String,
    #[serde(default)]
    pub landscape_id: String,
    #[serde(default)]
    pub tree_id: String,
    #[serde(default)]
    pub bush_id: String,
    #[serde(default)]
    pub grass_id: String,
    #[serde(default)]
    pub building_id: String,
    #[serde(default)]
    pub landscape_sources: Vec<PresetSource>,
    #[serde(default)]
    pub tree_sources: Vec<PresetSource>,
    #[serde(default)]
    pub bush_sources: Vec<PresetSource>,
    #[serde(default)]
    pub grass_sources: Vec<PresetSource>,
    #[serde(default)]
    pub building_sources: Vec<PresetSource>,
    #[serde(default)]
    pub trees: usize,
    #[serde(default)]
    pub bushes: usize,
    #[serde(default)]
    pub grass: usize,
    #[serde(default)]
    pub buildings: usize,
    /// Exact target range, before rejection of unsuitable positions.
    #[serde(default)]
    pub tree_count_range: Option<(usize, usize)>,
    #[serde(default = "default_max_slope")]
    pub max_slope_deg: f32,
    #[serde(default)]
    pub placement: PlacementSettings,
    #[serde(default)]
    pub grass_settings: GrassSettings,
    #[serde(default)]
    pub sandy_ground: bool,
    /// Demo landscapes are baked archetypes; false preserves a custom file verbatim.
    #[serde(default = "default_true")]
    pub regenerate_landscape: bool,
}
const fn default_max_slope() -> f32 {
    10.0
}
const fn default_true() -> bool {
    true
}

impl TerrainPreset {
    pub fn amounts(self) -> (usize, usize, usize, usize) {
        let data = TerrainPresetData::from_preset(self);
        (data.trees, data.bushes, data.grass, data.buildings)
    }
}

impl TerrainPresetData {
    pub fn from_preset(preset: TerrainPreset) -> Self {
        Self::from_preset_with(preset, None)
    }
    pub fn from_preset_with(preset: TerrainPreset, root: Option<&Path>) -> Self {
        use TerrainPreset::*;
        let mut data = Self {
            seed: Seed::default(),
            name: preset.name().into(),
            landscape_id: preset.landscape().kind().into(),
            tree_id: "deciduous-tree".into(),
            bush_id: "bush".into(),
            grass_id: "sparse-grass".into(),
            building_id: String::new(),
            landscape_sources: Vec::new(),
            tree_sources: Vec::new(),
            bush_sources: Vec::new(),
            grass_sources: Vec::new(),
            building_sources: Vec::new(),
            trees: 16,
            bushes: 12,
            grass: 600,
            buildings: 0,
            tree_count_range: None,
            max_slope_deg: 10.0,
            placement: PlacementSettings::default(),
            grass_settings: GrassSettings {
                density: GrassDensity::Medium,
                ..Default::default()
            },
            sandy_ground: preset == Beach,
            regenerate_landscape: true,
        };
        let ids = |items: &[&str]| {
            items
                .iter()
                .map(|id| PresetSource::preset(id))
                .collect::<Vec<_>>()
        };
        match preset {
            Field => {
                data.trees = 2;
                data.tree_count_range = Some((1, 3));
                data.bushes = 4;
            }
            OpenField => {
                data.trees = 0;
                data.bushes = 3;
            }
            Forest | ForestSlope | ForestTerraces | ConiferForest | ConiferSlope
            | ConiferTerraces | ForestCabin => {
                data.trees = 40;
                data.bushes = 24;
                data.grass_settings.density = GrassDensity::Dense;
                data.grass_sources = ids(&["sparse-grass", "tall-grass"]);
            }
            Woodland | WoodlandSlope | WoodlandTerraces => {
                data.trees = 10;
                data.placement.tree_clearance = 6.0;
                data.placement.cluster_probability = 0.35;
                data.tree_sources = ids(&["deciduous-tree", "pine"]);
            }
            HillSlope | TerracedSlope => {
                data.trees = 0;
                data.bushes = 5;
            }
            SkyscraperCity => {
                data.trees = 2;
                data.bushes = 4;
                data.buildings = 4;
                data.building_id = "skyscraper".into();
                data.placement.settlement = Settlement::Grid;
                data.placement.street_width = 8.0;
                data.placement.setback = 4.0;
                data.grass_settings.density = GrassDensity::Sparse;
            }
            Town | ApartmentCity => {
                data.trees = 8;
                data.bushes = 12;
                data.buildings = 4;
                data.building_id = "apartment-block".into();
                data.placement.settlement = Settlement::Grid;
                data.grass_settings.density = GrassDensity::Sparse;
            }
            ResidentialCity | Village => {
                data.trees = if preset == Village { 16 } else { 10 };
                data.buildings = if preset == Village { 9 } else { 14 };
                data.building_id = "rectangular".into();
                data.building_sources =
                    ids(&["rectangular", "l-shaped", "annex", "cottage", "cabin"]);
                data.placement.settlement = if preset == Village {
                    Settlement::Village
                } else {
                    Settlement::Grid
                };
            }
            _ => {}
        }
        if preset == ForestCabin {
            data.buildings = 1;
            data.building_id = "cabin".into();
            data.placement.settlement = Settlement::Cabin;
            data.placement.clearing_radius = 15.0;
        }
        if matches!(preset, ConiferForest | ConiferSlope | ConiferTerraces) {
            data.tree_sources = if root.is_some() {
                vec![PresetSource::group("coniferous")]
            } else {
                ids(&["spruce", "fir", "pine"])
            };
        }
        if matches!(
            preset,
            ConiferForest
                | ConiferSlope
                | ConiferTerraces
                | Woodland
                | WoodlandSlope
                | WoodlandTerraces
                | HillSlope
                | TerracedSlope
        ) {
            data.bush_sources = ids(&["bush", "juniper"]);
        }
        if preset.category() == "nature/water" {
            let mut willow = PresetSource::preset("willow");
            willow.max_water_distance = Some(11.0);
            let mut deciduous = PresetSource::preset("deciduous-tree");
            deciduous.min_water_distance = 13.0;
            data.tree_sources = vec![willow, deciduous];
            data.grass_settings.min_water_distance = 2.0;
            data.bush_sources = vec![PresetSource {
                min_water_distance: 5.0,
                ..PresetSource::preset("bush")
            }];
            if matches!(preset, RiverOpen | PondOpen | CoastOpen | WaterfallOpen) {
                data.trees = 0;
            }
        }
        if preset == Beach {
            data.trees = 16;
            data.bushes = 3;
            data.tree_sources = ids(&["palm", "palm-coconuts"]);
            for source in &mut data.tree_sources {
                source.min_water_distance = 7.0;
            }
            data.grass_settings.density = GrassDensity::Sparse;
            data.grass_settings.min_water_distance = 16.0;
        }
        data
    }
    pub fn with_seed(mut self, seed: Seed) -> Self {
        self.seed = seed;
        self
    }
    pub fn validate(&self) -> Result<(), String> {
        let p = &self.placement;
        let g = &self.grass_settings;
        if self.name.is_empty()
            || self.tree_count_range.is_some_and(|(min, max)| min > max)
            || !self.max_slope_deg.is_finite()
            || !(0.0..90.0).contains(&self.max_slope_deg)
            || !p.plant_slope_deg.is_finite()
            || !(0.0..90.0).contains(&p.plant_slope_deg)
            || [
                p.street_width,
                p.setback,
                p.tree_clearance,
                p.cluster_radius,
                p.clearing_radius,
                p.waterfall_clearance,
                g.min_water_distance,
            ]
            .iter()
            .any(|v| !v.is_finite() || *v < 0.0)
            || p.street_width <= 0.0
            || !p.cluster_probability.is_finite()
            || !(0.0..=1.0).contains(&p.cluster_probability)
            || !(1..=32).contains(&g.variants_per_preset)
            || !g.min_scale.is_finite()
            || !g.max_scale.is_finite()
            || g.min_scale <= 0.0
            || g.max_scale < g.min_scale
        {
            return Err(format!("Некорректные параметры территории '{}'", self.name));
        }
        Ok(())
    }
}
