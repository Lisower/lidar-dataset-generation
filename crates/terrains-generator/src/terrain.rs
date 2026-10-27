//! Composes existing landscape, plant and building generators into a 100x100 m scene.
//! Horizontal coordinates: XY; up axis: Z. The viewer handles conversion to Bevy Y-up.
use std::f32::consts::TAU;

use bevy::math::{Vec2, Vec3};
use buildings_generator::{Building, BuildingConfig};
use generation_core::{Occupancy, Seed};
use landscapes_generator::Landscape;
use plants_generator::generation::grass::generate_tuft;
use plants_generator::{PlacedPlant, PlacedTuft, Plant, PlantPreset};
use rand::{RngExt, rngs::StdRng};

use crate::placement::{PlacementMap, PlacementRules};
use crate::preset::{TerrainPreset, TerrainSettings};

#[derive(Debug, Clone, Copy)]
pub struct Road {
    pub start: Vec2,
    pub end: Vec2,
    pub width: f32,
}

#[derive(Debug)]
pub struct PlacedBuilding {
    pub position: Vec3,
    pub rotation: f32,
    pub building: Building,
}

#[derive(Debug)]
pub struct TerrainScene {
    pub preset: TerrainPreset,
    pub seed: Seed,
    pub landscape: Landscape,
    pub plants: Vec<PlacedPlant>,
    pub grass: Vec<PlacedTuft>,
    pub buildings: Vec<PlacedBuilding>,
    pub roads: Vec<Road>,
}

impl TerrainScene {
    pub fn total_objects(&self) -> usize {
        self.plants.len() + self.grass.len() + self.buildings.len()
    }

    pub fn altitude(&self, xy: Vec2) -> f32 {
        self.landscape.heightfield.height_at(xy.x, xy.y)
    }
}

/// Generate terrain from one seed. Unplaceable objects are skipped (never forcibly overlapped).
pub fn generate(preset: TerrainPreset, seed: Seed) -> TerrainScene {
    generate_with_settings(preset, seed, preset.settings())
}

fn generate_with_settings(preset: TerrainPreset, seed: Seed, settings: TerrainSettings) -> TerrainScene {
    let mut config = landscapes_generator::resolve(preset.landscape(), seed.with_label("terrain-shape"));
    config.size_m = 100.0;
    // Built-up areas need broadly level ground; preserve the project's existing noise generator.
    if matches!(preset, TerrainPreset::Village | TerrainPreset::Town | TerrainPreset::ForestCabin) {
        config.noise.amplitude *= 0.2;
    }
    let landscape = landscapes_generator::generation::landscape::generate(&config);
    let mut placement = PlacementMap::new(&landscape);
    let mut roads = Vec::new();
    let mut buildings = Vec::new();

    // Important: street corridors are reserved *before* placing houses or plants.
    match preset {
        TerrainPreset::Village => {
            roads.push(Road { start: Vec2::new(-48.0, 0.0), end: Vec2::new(48.0, 0.0), width: 5.0 });
        }
        TerrainPreset::Town => {
            for offset in [-22.0, 22.0] {
                roads.push(Road { start: Vec2::new(-48.0, offset), end: Vec2::new(48.0, offset), width: 6.0 });
                roads.push(Road { start: Vec2::new(offset, -48.0), end: Vec2::new(offset, 48.0), width: 6.0 });
            }
        }
        TerrainPreset::ForestCabin => {
            roads.push(Road { start: Vec2::new(0.0, -48.0), end: Vec2::new(0.0, -8.0), width: 1.6 });
        }
        _ => {}
    }
    for road in &roads {
        reserve_road(&mut placement, *road);
    }

    let mut building_rng = seed.with_label("buildings-positions").rng();
    for i in 0..settings.buildings {
        let building_seed = seed.with_label("building-model").child(i as u64 + 1);
        let mut cfg = BuildingConfig::default();
        cfg.seed = building_seed;
        if matches!(preset, TerrainPreset::ForestCabin | TerrainPreset::Village) {
            cfg.min_size = 5.0;
            cfg.max_size = 8.0;
            cfg.max_ratio = 1.35;
            cfg.min_floors = 1;
            cfg.max_floors = 1;
        } else {
            cfg.min_size = 6.0;
            cfg.max_size = 10.0;
            cfg.min_floors = 1;
            cfg.max_floors = 3;
        }
        let building = Building::generate(&cfg);
        let radius = (building.size.x.hypot(building.size.y) * 0.5).max(1.0);
        let rules = PlacementRules { radius, clearance: 1.7, max_slope_deg: settings.max_building_slope };
        let mut chosen = None;
        for attempt in 0..120 {
            let xy = building_candidate(preset, i, attempt, &mut building_rng);
            if let Some(object) = placement.try_place(xy, rules) {
                chosen = Some(object.position);
                break;
            }
        }
        if let Some(position) = chosen {
            buildings.push(PlacedBuilding { position, rotation: 0.0, building });
        }
    }

    // Keep a clearing around the cabin; avoid both its model and the access path.
    if preset == TerrainPreset::ForestCabin {
        if let Some(house) = buildings.first() {
            placement.reserve(Occupancy::circle(house.position.truncate(), 12.0));
        }
    }

    let mut plants = Vec::new();
    for (kind, count, label, clearance) in [
        (PlantPreset::DeciduousTree, settings.trees, "trees", 0.6),
        (PlantPreset::Bush, settings.bushes, "bushes", 0.2),
    ] {
        let mut rng = seed.with_label(label).rng();
        let patches = patch_centers(&mut rng, if kind == PlantPreset::DeciduousTree { 8 } else { 12 });
        for i in 0..count {
            let obj_seed = seed.with_label(label).child(i as u64 + 1);
            let cfg = plants_generator::resolve(kind, obj_seed);
            let plant = Plant::generate(&cfg);
            let rules = PlacementRules {
                radius: (plant.extent * 0.9).max(0.1),
                clearance,
                max_slope_deg: settings.max_plant_slope,
            };
            if let Some(xy) = place_in_patches(&mut placement, &mut rng, &patches, settings.tree_cluster_spread, rules, 45) {
                plants.push(PlacedPlant { position: xy, rotation: rng.random_range(0.0..TAU), plant });
            }
        }
    }

    let grass_preset = if matches!(preset, TerrainPreset::Forest | TerrainPreset::ForestCabin | TerrainPreset::River | TerrainPreset::Pond) {
        PlantPreset::TallGrass
    } else {
        PlantPreset::SparseGrass
    };
    let grass_config = plants_generator::resolve(grass_preset, seed.with_label("grass-configuration"));
    let mut grass_rng = seed.with_label("grass-positions").rng();
    let grass_patches = patch_centers(&mut grass_rng, 20);
    let mut grass = Vec::new();
    for i in 0..settings.grass {
        let tuft = generate_tuft(&grass_config, seed.with_label("grass-model").child(i as u64 + 1));
        let rules = PlacementRules { radius: tuft.radius.max(0.01), clearance: 0.04, max_slope_deg: 40.0 };
        if let Some(xy) = place_in_patches(&mut placement, &mut grass_rng, &grass_patches, 17.0, rules, 24) {
            grass.push(PlacedTuft { position: xy, rotation: grass_rng.random_range(0.0..TAU), tuft });
        }
    }

    TerrainScene { preset, seed, landscape, plants, grass, buildings, roads }
}

fn patch_centers(rng: &mut StdRng, count: usize) -> Vec<Vec2> {
    (0..count)
        .map(|_| Vec2::new(rng.random_range(-34.0..34.0), rng.random_range(-34.0..34.0)))
        .collect()
}

fn place_in_patches(
    placement: &mut PlacementMap<'_>,
    rng: &mut StdRng,
    patches: &[Vec2],
    spread: f32,
    rules: PlacementRules,
    attempts: usize,
) -> Option<Vec2> {
    for _ in 0..attempts {
        let center = patches[rng.random_range(0..patches.len())];
        let xy = if rng.random::<f32>() < 0.84 {
            // Triangular distribution: more plants in the center of a patch.
            center + Vec2::new(
                (rng.random_range(-spread..spread) + rng.random_range(-spread..spread)) * 0.5,
                (rng.random_range(-spread..spread) + rng.random_range(-spread..spread)) * 0.5,
            )
        } else {
            Vec2::new(rng.random_range(-48.0..48.0), rng.random_range(-48.0..48.0))
        };
        if placement.try_place(xy, rules).is_some() {
            return Some(xy);
        }
    }
    None
}

fn building_candidate(preset: TerrainPreset, index: usize, attempt: usize, rng: &mut StdRng) -> Vec2 {
    match preset {
        TerrainPreset::ForestCabin => Vec2::new(rng.random_range(-3.0..3.0), rng.random_range(-3.0..3.0)),
        TerrainPreset::Village => {
            let x = -40.0 + (index % 9) as f32 * 10.0 + rng.random_range(-1.0..1.0);
            let y = if (index + attempt) % 2 == 0 { 14.0 } else { -14.0 };
            Vec2::new(x, y + rng.random_range(-2.0..2.0))
        }
        _ => {
            // Town: houses occupy blocks between a network of reserved roads.
            let x = rng.random_range(-43.0..43.0);
            let y = rng.random_range(-43.0..43.0);
            Vec2::new(x, y)
        }
    }
}

fn reserve_road(placement: &mut PlacementMap<'_>, road: Road) {
    let length = road.start.distance(road.end);
    let steps = (length / (road.width * 0.4)).ceil().max(1.0) as usize;
    for i in 0..=steps {
        let center = road.start.lerp(road.end, i as f32 / steps as f32);
        placement.reserve(Occupancy::circle(center, road.width * 0.5));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_settings(p: TerrainPreset) -> TerrainSettings {
        let mut s = p.settings();
        s.trees = 3;
        s.bushes = 4;
        s.grass = 8;
        s.buildings = if p == TerrainPreset::ForestCabin { 1 } else { 0 };
        s
    }

    #[test]
    fn repeatable_positions() {
        let a = generate_with_settings(TerrainPreset::Forest, Seed::new(17), tiny_settings(TerrainPreset::Forest));
        let b = generate_with_settings(TerrainPreset::Forest, Seed::new(17), tiny_settings(TerrainPreset::Forest));
        let a_xy: Vec<_> = a.plants.iter().map(|p| p.position).collect();
        let b_xy: Vec<_> = b.plants.iter().map(|p| p.position).collect();
        assert_eq!(a_xy, b_xy);
        assert_eq!(a.grass.iter().map(|g| g.position).collect::<Vec<_>>(), b.grass.iter().map(|g| g.position).collect::<Vec<_>>());
    }

    #[test]
    fn on_dry_ground_without_intersections() {
        let s = generate_with_settings(TerrainPreset::Pond, Seed::new(42), tiny_settings(TerrainPreset::Pond));
        assert_eq!(s.landscape.heightfield.size_m, 100.0);
        let mut occ = Vec::new();
        for p in &s.plants { occ.push(Occupancy::circle(p.position, (p.plant.extent * 0.9).max(0.1))); }
        for g in &s.grass { occ.push(Occupancy::circle(g.position, g.tuft.radius.max(0.01))); }
        for (i, a) in occ.iter().enumerate() {
            assert!(a.center.x.abs() + a.shape.bounding_radius() <= 50.0);
            assert!(a.center.y.abs() + a.shape.bounding_radius() <= 50.0);
            for b in &occ[..i] { assert!(!a.intersects(b)); }
        }
    }
}
