//! Composition of existing landscape, vegetation and building generators.
//! No Bevy rendering is duplicated in this crate.
use std::f32::consts::TAU;
use std::path::Path;

use bevy::math::{Vec2, Vec3};
use buildings_generator::{Building, BuildingConfig};
use generation_core::presets::{load_preset_config, resolve_cross_category};
use generation_core::{Occupancy, Seed};
use landscapes_generator::{
    Landscape, LandscapeConfig, LandscapePreset, generation::landscape,
};
use plants_generator::generation::grass::generate_tuft;
use plants_generator::{PlacedPlant, PlacedTuft, Plant, PlantConfig, PlantPreset};
use rand::RngExt;

use crate::placement::{PlacementMap, PlacementRules};
use crate::preset::{TerrainPreset, TerrainPresetData};

/// Building + placement on the landscape. Building geometry is in local coordinates.
#[derive(Debug, Clone)]
pub struct PlacedBuilding {
    pub position: Vec3,
    pub rotation: f32,
    pub building: Building,
    /// Conservative collision circle containing the rotated building footprint.
    pub footprint: Occupancy,
}

/// Road corridor reserved for layout; it is data, not a rendered road mesh.
#[derive(Debug, Clone, Copy)]
pub struct RoadCorridor {
    pub start: Vec2,
    pub end: Vec2,
    pub width: f32,
}

/// One complete 100 x 100 meter scene of generation data.
#[derive(Debug)]
pub struct TerrainScene {
    pub preset: TerrainPreset,
    pub seed: Seed,
    pub landscape: Landscape,
    pub trees: Vec<PlacedPlant>,
    pub bushes: Vec<PlacedPlant>,
    pub grass: Vec<PlacedTuft>,
    pub buildings: Vec<PlacedBuilding>,
    pub roads: Vec<RoadCorridor>,
    pub footprints: Vec<Occupancy>,
}

impl TerrainScene {
    pub fn object_count(&self) -> usize {
        self.trees.len() + self.bushes.len() + self.grass.len() + self.buildings.len()
    }
}

fn clusters(seed: Seed, count: usize) -> Vec<Vec2> {
    let mut rng = seed.with_label("clusters").rng();
    (0..count)
        .map(|_| Vec2::new(rng.random_range(-38.0..38.0), rng.random_range(-38.0..38.0)))
        .collect()
}

fn vegetation_candidate(rng: &mut impl rand::Rng, anchors: &[Vec2], preset: TerrainPreset) -> Vec2 {
    if matches!(
        preset,
        TerrainPreset::Field | TerrainPreset::Forest | TerrainPreset::ForestCabin
    ) && !anchors.is_empty()
        && rng.random::<f32>() < 0.78
    {
        let center = anchors[rng.random_range(0..anchors.len())];
        let radius = if preset == TerrainPreset::Field {
            11.0
        } else {
            17.0
        };
        let r = rng.random::<f32>().sqrt() * radius;
        let angle = rng.random_range(0.0..TAU);
        center + Vec2::new(angle.cos() * r, angle.sin() * r)
    } else {
        Vec2::new(rng.random_range(-49.0..49.0), rng.random_range(-49.0..49.0))
    }
}

fn house_candidate(rng: &mut impl rand::Rng, preset: TerrainPreset) -> Vec2 {
    match preset {
        TerrainPreset::ForestCabin => {
            Vec2::new(rng.random_range(-6.0..6.0), rng.random_range(-6.0..6.0))
        }
TerrainPreset::Village => Vec2::new(
    rng.random_range(-40.0..40.0),
    rng.random_range(-40.0..40.0),
),
        TerrainPreset::Town => {
            // Houses approximately line the streets, with offset from the reserved roadway.
            if rng.random::<bool>() {
                let lane = if rng.random::<bool>() { -20.0 } else { 20.0 };
                Vec2::new(
                    lane + if rng.random::<bool>() { 11.0 } else { -11.0 },
                    rng.random_range(-43.0..43.0),
                )
            } else {
                let lane = if rng.random::<bool>() { -20.0 } else { 20.0 };
                Vec2::new(
                    rng.random_range(-43.0..43.0),
                    lane + if rng.random::<bool>() { 11.0 } else { -11.0 },
                )
            }
        }
        _ => Vec2::ZERO,
    }
}

/// Generate a composite scene, using only the existing generators.
/// The number of objects can be lower than requested if the free terrain is insufficient.
///
/// Без `presets_root` (и в headless-режиме) суб-конфиги берутся из архетипов
/// `TerrainPreset` — поведение полностью совпадает с прежним.
pub fn generate(preset: TerrainPreset, seed: Seed) -> TerrainScene {
    let data = TerrainPresetData::from_preset(preset);
    generate_impl(Some(preset), &data, seed, None)
}

/// Generate from preset data (loaded from `presets/terrains/.../config.ron`).
///
/// `presets_root` — корень каталога пресетов (для cross-category подгрузки
/// ландшафта, растений и зданий по `resolve_cross_category`); `None` —
/// всё откатывается на архетипы.
pub fn generate_from_data(
    data: &TerrainPresetData,
    seed: Seed,
    presets_root: Option<&Path>,
) -> TerrainScene {
    let preset = TerrainPreset::from_name(&data.name).unwrap_or(TerrainPreset::Field);
    generate_impl(Some(preset), data, seed, presets_root)
}

/// Общая реализация: `preset_hint` нужен только для fallback-решений
/// (тип травы, твики зданий по архетипу) и для функций расстановки.
#[allow(clippy::too_many_arguments)]
fn generate_impl(
    preset_hint: Option<TerrainPreset>,
    data: &TerrainPresetData,
    seed: Seed,
    presets_root: Option<&Path>,
) -> TerrainScene {
    // Архетип для расстановки; в демо совпадает с именем пресета.
    let preset = preset_hint
        .unwrap_or_else(|| TerrainPreset::from_name(&data.name).unwrap_or(TerrainPreset::Field));

    // Ландшафт: конфиг из файла суб-пресета (с заменой seed), либо архетип.
    let sub_landscape = presets_root
        .and_then(|root| resolve_cross_category("landscapes", &data.landscape_id, root))
        .and_then(|meta| load_preset_config::<LandscapeConfig>(&meta));
    let mut cfg = match sub_landscape {
        Some(mut loaded) => {
            loaded.seed = seed.with_label("landscape");
            loaded.size_m = 100.0;
            loaded
        }
        None => {
            // Файл ландшафта недоступен — архетип по архетипу террейна.
            let fallback = preset_hint
                .map(TerrainPreset::landscape)
                .unwrap_or(LandscapePreset::Plain);
            let mut c = landscapes_generator::resolve(fallback, seed.with_label("landscape"));
            c.size_m = 100.0;
            c
        }
    };
    // Developed areas need moderately smooth ground for building foundations.
    if data.buildings > 0 {
        cfg.noise.amplitude = cfg.noise.amplitude.min(0.25);
    }
    let landscape = landscape::generate(&cfg);
    let mut map = PlacementMap::new(&landscape);
    let roads = Vec::new();

    // Целевые счётчики (как прежде, с джиттером ±1/3).
    let mut count_rng = seed.with_label("object-counts").rng();
    let tree_count = count_rng.random_range(
        data.trees.saturating_sub(data.trees / 3)..=data.trees + data.trees / 3,
    );
    let bush_count = count_rng.random_range(
        data.bushes.saturating_sub(data.bushes / 3)..=data.bushes + data.bushes / 3,
    );
    let grass_count = count_rng.random_range(
        data.grass.saturating_sub(data.grass / 3)..=data.grass + data.grass / 3,
    );
    let house_count = count_rng.random_range(
        data.buildings.saturating_sub(data.buildings / 3)..=data.buildings + data.buildings / 3,
    );

    // Здания: базовый конфиг из файла (твики этажей/размеров в нём уже есть),
    // либо архетип с твиками по `preset`.
    let building_base = if data.buildings == 0 || data.building_id.is_empty() {
        None
    } else {
        presets_root
            .and_then(|root| resolve_cross_category("buildings", &data.building_id, root))
            .and_then(|meta| load_preset_config::<BuildingConfig>(&meta))
    };
    let mut buildings = Vec::new();
    let mut house_rng = seed.with_label("building-positions").rng();
    for idx in 0..house_count {
        let mut build_cfg = match &building_base {
            Some(base) => base.clone(),
            None => {
                let mut b = BuildingConfig::default();
                match preset {
                    TerrainPreset::Town => {
                        b.min_size = 5.0;
                        b.max_size = 9.0;
                        b.min_floors = 2;
                        b.max_floors = 4;
                    }
                    TerrainPreset::Village | TerrainPreset::ForestCabin => {
                        b.min_size = 5.0;
                        b.max_size = 9.0;
                        b.min_floors = 1;
                        b.max_floors = 2;
                    }
                    _ => {}
                }
                b
            }
        };
        build_cfg.seed = seed.with_label("building-model").child(idx as u64);
        let building = Building::generate(&build_cfg);
        let radius = building.occupancy.shape.bounding_radius();
        // A circle around the entire foundation is conservative for collision checks.
        let rules = PlacementRules {
            radius,
            clearance: 1.5,
            max_slope_deg: data.max_slope_deg,
        };
        for _ in 0..120 {
            let candidate = house_candidate(&mut house_rng, preset);
            if let Some(positioned) = map.try_place(candidate, rules) {
                buildings.push(PlacedBuilding {
                    position: positioned.position,
                    rotation: house_rng.random_range(0.0..TAU),
                    building,
                    footprint: positioned.occupancy,
                });
                break;
            }
        }
    }

    // Деревья и кусты: конфиги из файлов суб-пресетов, либо архетипы.
    let tree_base = if data.trees == 0 {
        None
    } else {
        load_plant_base(presets_root, &data.tree_id)
    };
    let bush_base = if data.bushes == 0 {
        None
    } else {
        load_plant_base(presets_root, &data.bush_id)
    };
    let tree_anchors = clusters(seed.with_label("tree-anchors"), 7);
    let bush_anchors = clusters(seed.with_label("bush-anchors"), 10);
    let mut trees = Vec::new();
    let mut bushes = Vec::new();
    for (kind, count, anchors, base, out) in [
        (
            PlantPreset::DeciduousTree,
            tree_count,
            &tree_anchors,
            tree_base.as_ref(),
            &mut trees,
        ),
        (PlantPreset::Bush, bush_count, &bush_anchors, bush_base.as_ref(), &mut bushes),
    ] {
        let mut rng = seed.with_label(kind.label()).rng();
        for i in 0..count {
            let obj_seed = seed.with_label(kind.label()).child(i as u64);
            let plant_cfg = match base {
                Some(b) => b.with_seed(obj_seed),
                None => plants_generator::resolve(kind, obj_seed),
            };
            let plant = Plant::generate(&plant_cfg);
            let radius = plant.extent.max(0.1);
            let rules = PlacementRules {
                radius,
                clearance: if kind == PlantPreset::DeciduousTree {
                    0.5
                } else {
                    0.2
                },
                max_slope_deg: 30.0,
            };
            for _ in 0..90 {
                let xy = vegetation_candidate(&mut rng, anchors, preset);
                // Keep the clearing around the cabin open even where its foundation is small.
                if preset == TerrainPreset::ForestCabin && xy.length() < 17.0 + radius {
                    continue;
                }
                if map.try_place(xy, rules).is_some() {
                    out.push(PlacedPlant {
                        position: xy,
                        rotation: rng.random_range(0.0..TAU),
                        plant,
                    });
                    break;
                }
            }
        }
    }

    // Трава: конфиг из файла суб-пресета, либо архетип (редкая/густая).
    let grass_fallback = if matches!(
        preset,
        TerrainPreset::Field | TerrainPreset::Village | TerrainPreset::Town
    ) {
        PlantPreset::SparseGrass
    } else {
        PlantPreset::TallGrass
    };
    let grass_base = if data.grass == 0 {
        None
    } else {
        load_plant_base(presets_root, &data.grass_id)
    };
    let mut grass = Vec::new();
    let mut grass_rng = seed.with_label("grass-positions").rng();
    let anchors = clusters(seed.with_label("grass-anchors"), 14);
    for i in 0..grass_count {
        let tuft_seed = seed.with_label("grass-model").child(i as u64);
        let grass_cfg = match &grass_base {
            Some(b) => b.with_seed(tuft_seed),
            None => plants_generator::resolve(grass_fallback, tuft_seed),
        };
        let tuft = generate_tuft(&grass_cfg, tuft_seed);
        let rules = PlacementRules {
            radius: tuft.radius.max(0.05),
            clearance: 0.02,
            max_slope_deg: 35.0,
        };
        for _ in 0..50 {
            let xy = vegetation_candidate(&mut grass_rng, &anchors, preset);
            if preset == TerrainPreset::ForestCabin && xy.length() < 12.0 {
                continue;
            }
            if map.try_place(xy, rules).is_some() {
                grass.push(PlacedTuft {
                    position: xy,
                    rotation: grass_rng.random_range(0.0..TAU),
                    tuft,
                });
                break;
            }
        }
    }

    let footprints = map.occupied().to_vec();

    drop(map);

    TerrainScene {
        preset,
        seed,
        landscape,
        trees,
        bushes,
        grass,
        buildings,
        roads,
        footprints,
    }
}

/// Загрузить базовый конфиг растения суб-пресета (`presets/plants/<id>`),
/// если пресеты доступны; иначе `None` — генератор сам возьмёт архетип.
fn load_plant_base(presets_root: Option<&Path>, id: &str) -> Option<PlantConfig> {
    presets_root
        .and_then(|root| resolve_cross_category("plants", id, root))
        .and_then(|meta| load_preset_config::<PlantConfig>(&meta))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn field_is_100m_and_populated() {
        let scene = generate(TerrainPreset::Field, Seed::new(42));
        assert_eq!(scene.landscape.heightfield.size_m, 100.0);
        assert!(scene.object_count() > 0);
    }
    #[test]
    fn footprints_do_not_intersect_except_roads() {
        let scene = generate(TerrainPreset::Field, Seed::new(123));
        for (i, a) in scene.footprints.iter().enumerate() {
            for b in &scene.footprints[..i] {
                assert!(!a.intersects(b));
            }
        }
    }
    #[test]
    fn positions_are_deterministic() {
        let a = generate(TerrainPreset::Field, Seed::new(9));
        let b = generate(TerrainPreset::Field, Seed::new(9));
        assert_eq!(
            a.trees.iter().map(|p| p.position).collect::<Vec<_>>(),
            b.trees.iter().map(|p| p.position).collect::<Vec<_>>()
        );
        assert_eq!(
            a.grass.iter().map(|p| p.position).collect::<Vec<_>>(),
            b.grass.iter().map(|p| p.position).collect::<Vec<_>>()
        );
    }
}
