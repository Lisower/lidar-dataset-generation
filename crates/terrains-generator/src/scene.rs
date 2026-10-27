//! Composition of existing landscape, vegetation and building generators.
//! No Bevy rendering is duplicated in this crate.
use std::f32::consts::TAU;

use bevy::math::{Vec2, Vec3};
use buildings_generator::{Building, BuildingConfig};
use generation_core::{Occupancy, Seed};
use landscapes_generator::{Landscape, generation::landscape};
use plants_generator::generation::grass::generate_tuft;
use plants_generator::{PlacedPlant, PlacedTuft, Plant, PlantPreset};
use rand::RngExt;

use crate::placement::{PlacementMap, PlacementRules};
use crate::preset::TerrainPreset;

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

fn reserve_road(
    map: &mut PlacementMap<'_>,
    roads: &mut Vec<RoadCorridor>,
    start: Vec2,
    end: Vec2,
    width: f32,
) {
    let diff = end - start;
    let center = (start + end) * 0.5;
    map.reserve(Occupancy::square(
        center,
        Vec2::new(diff.length() * 0.5, width * 0.5),
        diff.y.atan2(diff.x),
    ));
    roads.push(RoadCorridor { start, end, width });
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
pub fn generate(preset: TerrainPreset, seed: Seed) -> TerrainScene {
    let mut cfg = landscapes_generator::resolve(preset.landscape(), seed.with_label("landscape"));
    cfg.size_m = 100.0;
    // Developed areas need moderately smooth ground for building foundations.
    if matches!(
        preset,
        TerrainPreset::Village | TerrainPreset::Town | TerrainPreset::ForestCabin
    ) {
        cfg.noise.amplitude = cfg.noise.amplitude.min(0.25);
    }
    let landscape = landscape::generate(&cfg);
    let mut map = PlacementMap::new(&landscape);
    let mut roads = Vec::new();



    let (base_trees, base_bushes, base_grass, base_houses) = preset.amounts();

let mut count_rng = seed.with_label("object-counts").rng();

let tree_count = count_rng.random_range(
    base_trees.saturating_sub(base_trees / 3)
        ..=base_trees + base_trees / 3
);

let bush_count = count_rng.random_range(
    base_bushes.saturating_sub(base_bushes / 3)
        ..=base_bushes + base_bushes / 3
);

let grass_count = count_rng.random_range(
    base_grass.saturating_sub(base_grass / 3)
        ..=base_grass + base_grass / 3
);

let house_count = count_rng.random_range(
    base_houses.saturating_sub(base_houses / 3)
        ..=base_houses + base_houses / 3
);
    let mut buildings = Vec::new();
    let mut house_rng = seed.with_label("building-positions").rng();
    for idx in 0..house_count {
        let mut build_cfg = BuildingConfig::default();
        build_cfg.seed = seed.with_label("building-model").child(idx as u64);
        match preset {
            TerrainPreset::Town => {
                build_cfg.min_size = 5.0;
                build_cfg.max_size = 9.0;
                build_cfg.min_floors = 2;
                build_cfg.max_floors = 4;
            }
            TerrainPreset::Village | TerrainPreset::ForestCabin => {
                build_cfg.min_size = 5.0;
                build_cfg.max_size = 9.0;
                build_cfg.min_floors = 1;
                build_cfg.max_floors = 2;
            }
            _ => {}
        }
        let building = Building::generate(&build_cfg);
        let radius = building.occupancy.shape.bounding_radius();
        // A circle around the entire foundation is conservative for collision checks.
        let rules = PlacementRules {
            radius,
            clearance: 1.5,
            max_slope_deg: 10.0,
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

    let tree_anchors = clusters(seed.with_label("tree-anchors"), 7);
    let bush_anchors = clusters(seed.with_label("bush-anchors"), 10);
    let mut trees = Vec::new();
    let mut bushes = Vec::new();
    for (kind, count, anchors, out) in [
        (
            PlantPreset::DeciduousTree,
            tree_count,
            &tree_anchors,
            &mut trees,
        ),
        (PlantPreset::Bush, bush_count, &bush_anchors, &mut bushes),
    ] {
        let mut rng = seed.with_label(kind.label()).rng();
        for i in 0..count {
            let obj_seed = seed.with_label(kind.label()).child(i as u64);
            let plant_cfg = plants_generator::resolve(kind, obj_seed);
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
                if let Some(_) = map.try_place(xy, rules) {
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

    let grass_preset = if matches!(
        preset,
        TerrainPreset::Field | TerrainPreset::Village | TerrainPreset::Town
    ) {
        PlantPreset::SparseGrass
    } else {
        PlantPreset::TallGrass
    };
    let mut grass = Vec::new();
    let mut grass_rng = seed.with_label("grass-positions").rng();
    let anchors = clusters(seed.with_label("grass-anchors"), 14);
    for i in 0..grass_count {
        let tuft_seed = seed.with_label("grass-model").child(i as u64);
        let grass_cfg = plants_generator::resolve(grass_preset, tuft_seed);
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
