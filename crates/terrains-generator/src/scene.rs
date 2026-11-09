//! Scene composition only. Every model comes from an existing generator.
use crate::{
    placement::{PlacementMap, PlacementRules},
    preset::{GrassDensity, Settlement, TerrainPreset, TerrainPresetData},
    sources::{self, Candidate},
};
use bevy::math::{Mat4, Quat, Vec2, Vec3};
use buildings_generator::{Building, BuildingConfig};
use generation_core::presets::load_preset_config;
use generation_core::{Occupancy, PointCloud, Seed};
use landscapes_generator::{Landscape, LandscapeConfig, LandscapePreset, generation::landscape};
use plants_generator::generation::{
    config::PlantType,
    instances::{GrassField, GrassInstance},
};
use plants_generator::{PlacedPlant, Plant, PlantConfig, PlantPreset};
use rand::RngExt;
use std::{
    f32::consts::{FRAC_PI_2, TAU},
    path::Path,
    time::{Duration, Instant},
};

#[derive(Debug, Clone)]
pub struct PlacedBuilding {
    pub position: Vec3,
    pub rotation: f32,
    pub building: Building,
    pub footprint: Occupancy,
}
/// Logical corridors only: no road meshes or generated road geometry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoadCorridor {
    pub start: Vec2,
    pub end: Vec2,
    pub width: f32,
}
impl RoadCorridor {
    pub fn nearest(&self, point: Vec2) -> Vec2 {
        let delta = self.end - self.start;
        self.start
            + delta * ((point - self.start).dot(delta) / delta.length_squared()).clamp(0.0, 1.0)
    }
    pub fn distance(&self, point: Vec2) -> f32 {
        point.distance(self.nearest(point))
    }
}

#[derive(Debug)]
pub struct TerrainScene {
    pub preset: TerrainPreset,
    pub seed: Seed,
    pub landscape: Landscape,
    pub landscape_name: String,
    pub trees: Vec<PlacedPlant>,
    pub bushes: Vec<PlacedPlant>,
    pub grass: GrassField,
    pub buildings: Vec<PlacedBuilding>,
    pub roads: Vec<RoadCorridor>,
    /// Woody-object and building bounds; grass has a separate ground layer.
    pub footprints: Vec<Occupancy>,
    pub sandy_ground: bool,
    pub generation_time: Duration,
    pub grass_generation_time: Duration,
}
impl TerrainScene {
    pub fn object_count(&self) -> usize {
        self.trees.len() + self.bushes.len() + self.grass.len() + self.buildings.len()
    }
    /// Full classified headless cloud, Z up, with the same transforms as the viewer.
    pub fn point_cloud(&self) -> PointCloud {
        let mut cloud = self.landscape.ground_cloud.clone();
        cloud.merge(&self.landscape.water_cloud);
        for placed in self.trees.iter().chain(&self.bushes) {
            let mut points = placed.plant.point_cloud.clone();
            let z = self
                .landscape
                .heightfield
                .height_at(placed.position.x, placed.position.y);
            points.transform_by(Mat4::from_rotation_translation(
                Quat::from_rotation_z(placed.rotation),
                placed.position.extend(z),
            ));
            cloud.merge(&points);
        }
        for placed in &self.buildings {
            let mut points = placed.building.point_cloud.clone();
            points.transform_by(Mat4::from_rotation_translation(
                Quat::from_rotation_z(placed.rotation),
                placed.position,
            ));
            cloud.merge(&points);
        }
        cloud.merge(&self.grass.point_cloud());
        cloud
    }
}

pub fn generate(preset: TerrainPreset, seed: Seed) -> TerrainScene {
    generate_from_data(&TerrainPresetData::from_preset(preset), seed, None)
}
/// Compatibility wrapper. Fallible callers (UI/CLI) should use try_generate_from_data.
/// A bad reference is an explicit error, never an arbitrary substitute model.
pub fn generate_from_data(
    data: &TerrainPresetData,
    seed: Seed,
    root: Option<&Path>,
) -> TerrainScene {
    try_generate_from_data(data, seed, root)
        .unwrap_or_else(|error| panic!("{}: {error}", data.name))
}

struct PlantChoice {
    config: PlantConfig,
    candidate: Candidate,
}
fn plant_choices(items: Vec<Candidate>, expected: PlantType) -> Result<Vec<PlantChoice>, String> {
    items
        .into_iter()
        .map(|candidate| {
            let config = match &candidate.meta {
                Some(meta) => load_preset_config::<PlantConfig>(meta)
                    .ok_or_else(|| format!("Не удалось загрузить растение '{}'", meta.id))?,
                None => {
                    let kind = PlantPreset::ALL
                        .into_iter()
                        .find(|p| p.ron_id() == candidate.source.id)
                        .ok_or_else(|| {
                            format!("Неизвестный пресет растения '{}'", candidate.source.id)
                        })?;
                    plants_generator::resolve(kind, Seed::new(0))
                }
            };
            if config.plant_type != expected || config.preset.plant_type() != expected {
                return Err(format!(
                    "Пресет '{}' не относится к типу {expected:?}",
                    candidate.source.id
                ));
            }
            Ok(PlantChoice { config, candidate })
        })
        .collect()
}

fn buildings(items: Vec<Candidate>) -> Result<Vec<(Candidate, BuildingConfig)>, String> {
    items
        .into_iter()
        .map(|candidate| {
            let config = match &candidate.meta {
                Some(meta) => load_preset_config::<BuildingConfig>(meta)
                    .ok_or_else(|| format!("Не удалось загрузить здание '{}'", meta.id))?,
                None => {
                    use buildings_generator::generation::config::BuildingPreset;
                    let preset = match candidate.source.id.as_str() {
                        "rectangular" | "cabin" => BuildingPreset::Rectangular,
                        "annex" => BuildingPreset::WithAnnex,
                        "l-shaped" => BuildingPreset::LShaped,
                        "apartment-block" => BuildingPreset::ApartmentBlock,
                        "skyscraper" => BuildingPreset::Skyscraper,
                        id => return Err(format!("Неизвестный пресет здания '{id}'")),
                    };
                    let mut config = BuildingConfig {
                        preset,
                        ..Default::default()
                    };
                    if candidate.source.id == "cabin" {
                        config.max_floors = 1;
                        config.min_size = 5.0;
                        config.max_size = 8.0;
                    }
                    config
                }
            };
            Ok((candidate, config))
        })
        .collect()
}

fn target(count: usize, rng: &mut impl rand::Rng) -> usize {
    rng.random_range(count.saturating_sub(count / 3)..=count.saturating_add(count / 3))
}
fn require_nonempty(count: usize, items: usize, label: &str) -> Result<(), String> {
    if count > 0 && items == 0 {
        Err(format!("Не заданы пресеты: {label}"))
    } else {
        Ok(())
    }
}

pub fn try_generate_from_data(
    data: &TerrainPresetData,
    seed: Seed,
    root: Option<&Path>,
) -> Result<TerrainScene, String> {
    let started = Instant::now();
    data.validate()?;
    let landscapes = sources::resolve(
        "landscapes",
        &data.landscape_sources,
        &data.landscape_id,
        root,
    )?;
    require_nonempty(1, landscapes.len(), "базовый ландшафт")?;
    // Resolve every configured reference, including disabled categories, to expose typos.
    let trees = plant_choices(
        sources::resolve("plants", &data.tree_sources, &data.tree_id, root)?,
        PlantType::Tree,
    )?;
    let bushes = plant_choices(
        sources::resolve("plants", &data.bush_sources, &data.bush_id, root)?,
        PlantType::Bush,
    )?;
    let grasses = plant_choices(
        sources::resolve("plants", &data.grass_sources, &data.grass_id, root)?,
        PlantType::Grass,
    )?;
    let houses = buildings(sources::resolve(
        "buildings",
        &data.building_sources,
        &data.building_id,
        root,
    )?)?;
    require_nonempty(
        data.trees
            .max(data.tree_count_range.map_or(0, |(_, max)| max)),
        trees.len(),
        "деревья",
    )?;
    require_nonempty(data.bushes, bushes.len(), "кустарники")?;
    require_nonempty(data.buildings, houses.len(), "здания")?;
    let mut rng = seed.with_label("landscape-choice").rng();
    let chosen = &landscapes[sources::choose(&landscapes, |c| c.source.weight, &mut rng)];
    let landscape_seed = seed.with_label("landscape");
    let mut config = if let Some(meta) = &chosen.meta {
        let loaded = load_preset_config::<LandscapeConfig>(meta)
            .ok_or_else(|| format!("Не удалось загрузить ландшафт '{}'", meta.id))?;
        // Preserve the existing seeded archetype regeneration for baked demo landscapes.
        // Unknown/custom kinds load their actual config, with no fallback to Field.
        match LandscapePreset::from_kind(&meta.kind).filter(|_| data.regenerate_landscape) {
            Some(kind) => landscapes_generator::resolve(kind, landscape_seed),
            None => loaded,
        }
    } else {
        let kind = LandscapePreset::from_kind(&chosen.source.id)
            .ok_or_else(|| format!("Неизвестный ландшафт '{}'", chosen.source.id))?;
        landscapes_generator::resolve(kind, landscape_seed)
    };
    config.seed = landscape_seed;
    config.size_m = 100.0;
    if data.buildings > 0 {
        config.noise.amplitude = config.noise.amplitude.min(0.20);
    }
    let landscape_name = chosen
        .meta
        .as_ref()
        .map_or_else(|| config.label.clone(), |meta| meta.name.clone());
    let landscape = landscape::generate(&config);
    let mut map = PlacementMap::new(&landscape);
    let roads = street_layout(data, seed);
    for road in &roads {
        let delta = road.end - road.start;
        map.reserve(Occupancy::square(
            (road.start + road.end) * 0.5,
            Vec2::new(delta.length() * 0.5, road.width * 0.5),
            delta.y.atan2(delta.x),
        ));
    }
    let mut counts = seed.with_label("object-counts").rng();
    let tree_count = data.tree_count_range.map_or_else(
        || target(data.trees, &mut counts),
        |(min, max)| {
            // Separate deterministic stream avoids borrowing the count stream twice.
            seed.with_label("tree-count").rng().random_range(min..=max)
        },
    );
    let bush_count = target(data.bushes, &mut counts);
    let house_count = target(data.buildings, &mut counts);
    let mut house_rng = seed.with_label("building-positions").rng();
    let mut placed_buildings = Vec::new();
    for index in 0..house_count {
        let choice = sources::choose(&houses, |(c, _)| c.source.weight, &mut house_rng);
        let mut config = houses[choice].1.clone();
        config.seed = seed.with_label("building-model").child(index as u64);
        let building = Building::generate(&config);
        // Include roof overhang and facade details in the conservative bound.
        let radius = building
            .occupancy
            .shape
            .bounding_radius()
            .max(building.size.truncate().length() * 0.5 + building.overhang + 0.5);
        let rules = PlacementRules {
            radius,
            clearance: data.placement.setback,
            max_slope_deg: data.max_slope_deg,
        };
        for _ in 0..250 {
            let (xy, rotation) = house_candidate(data, &roads, radius, &mut house_rng);
            if let Some(object) = map.try_place(xy, rules) {
                placed_buildings.push(PlacedBuilding {
                    position: object.position,
                    rotation,
                    building,
                    footprint: object.occupancy,
                });
                break;
            }
        }
    }
    let mut placed_trees = Vec::new();
    let mut placed_bushes = Vec::new();
    for (label, choices, count, out, tree) in [
        ("trees", &trees, tree_count, &mut placed_trees, true),
        ("bushes", &bushes, bush_count, &mut placed_bushes, false),
    ] {
        let mut rng = seed.with_label(label).rng();
        let anchors = anchors(seed.with_label(label));
        for index in 0..count {
            let choice =
                &choices[sources::choose(choices, |c| c.candidate.source.weight, &mut rng)];
            let plant = Plant::generate(
                &choice
                    .config
                    .with_seed(seed.with_label(label).child(index as u64)),
            );
            let radius = plant.extent.max(0.1);
            // The generator's first branch level defines the actual trunk base radius.
            let trunk_radius = plant
                .point_cloud
                .points
                .iter()
                .filter(|p| p.position.z < 0.5)
                .map(|p| p.position.truncate().length())
                .fold(0.1f32, f32::max);
            let rules = PlacementRules {
                radius,
                clearance: if tree {
                    data.placement.tree_clearance
                } else {
                    0.2
                },
                max_slope_deg: data.placement.plant_slope_deg,
            };
            for _ in 0..350 {
                let xy = vegetation_candidate(&anchors, data, &mut rng);
                if xy.length() < data.placement.clearing_radius + radius
                    && data.placement.clearing_radius > 0.0
                {
                    continue;
                }
                // Use the existing curtain metadata to keep the waterfall visible.
                let margin = data.placement.waterfall_clearance;
                if margin > 0.0
                    && landscape.water_walls.iter().any(|fall| {
                        let delta = xy - fall.p;
                        delta.dot(fall.n).abs() < fall.hw + radius + margin
                            && delta.dot(fall.flow) > -radius
                            && delta.dot(fall.flow) < radius + margin * 2.0
                    })
                {
                    continue;
                }
                if !choice
                    .candidate
                    .source
                    .accepts_water_distance(map.distance_to_water(xy))
                {
                    continue;
                }
                let placed = if tree {
                    map.try_place_tree(xy, rules, trunk_radius + 0.1)
                } else {
                    map.try_place(xy, rules)
                };
                if placed.is_some() {
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
    let grass_started = Instant::now();
    let grass_count = match data.grass_settings.density {
        GrassDensity::Legacy => target(data.grass, &mut counts),
        GrassDensity::None => 0,
        GrassDensity::Sparse => 800,
        GrassDensity::Medium => 3500,
        GrassDensity::Dense => 10000,
    };
    require_nonempty(grass_count, grasses.len(), "трава")?;
    let configs: Vec<_> = grasses.iter().map(|c| c.config.clone()).collect();
    let mut grass = if grass_count == 0 {
        GrassField::default()
    } else {
        GrassField::variants(
            &configs,
            data.grass_settings.variants_per_preset,
            seed.with_label("grass-variants"),
        )
    };
    let mut rng = seed.with_label("grass-positions").rng();
    let anchors = anchors(seed.with_label("grass-anchors"));
    for _ in 0..grass_count {
        let choice_index = sources::choose(&grasses, |c| c.candidate.source.weight, &mut rng);
        let choice = &grasses[choice_index];
        let variant = choice_index * data.grass_settings.variants_per_preset
            + rng.random_range(0..data.grass_settings.variants_per_preset);
        let scale = rng.random_range(data.grass_settings.min_scale..=data.grass_settings.max_scale);
        let tuft = &grass.variants[variant];
        let radius = tuft
            .part
            .faces
            .iter()
            .flat_map(|face| face.points())
            .map(|p| p.truncate().length())
            .fold(tuft.radius, f32::max)
            * scale;
        let rules = PlacementRules {
            radius: radius.max(0.05),
            clearance: 0.02,
            max_slope_deg: data.placement.plant_slope_deg.min(40.0),
        };
        for _ in 0..30 {
            let xy = vegetation_candidate(&anchors, data, &mut rng);
            let distance = map.distance_to_water(xy);
            if distance < data.grass_settings.min_water_distance
                || !choice.candidate.source.accepts_water_distance(distance)
            {
                continue;
            }
            if xy.length() < data.placement.clearing_radius && data.placement.clearing_radius > 0.0
            {
                continue;
            }
            if let Some(object) = map.try_place_grass(xy, rules) {
                grass.instances.push(GrassInstance {
                    variant,
                    position: object.position,
                    rotation: rng.random_range(0.0..TAU),
                    scale,
                });
                break;
            }
        }
    }
    let grass_generation_time = grass_started.elapsed();
    let footprints = map.occupied().to_vec();
    drop(map);
    Ok(TerrainScene {
        preset: TerrainPreset::from_name(&data.name).unwrap_or(TerrainPreset::Custom),
        seed,
        landscape,
        landscape_name,
        trees: placed_trees,
        bushes: placed_bushes,
        grass,
        buildings: placed_buildings,
        roads,
        footprints,
        sandy_ground: data.sandy_ground,
        generation_time: started.elapsed(),
        grass_generation_time,
    })
}

fn anchors(seed: Seed) -> Vec<Vec2> {
    let mut rng = seed.rng();
    (0..12)
        .map(|_| Vec2::new(rng.random_range(-38.0..38.0), rng.random_range(-38.0..38.0)))
        .collect()
}
fn vegetation_candidate(
    anchors: &[Vec2],
    data: &TerrainPresetData,
    rng: &mut impl rand::Rng,
) -> Vec2 {
    if rng.random::<f32>() < data.placement.cluster_probability {
        let center = anchors[rng.random_range(0..anchors.len())];
        let angle = rng.random_range(0.0..TAU);
        let radius = rng.random::<f32>().sqrt() * data.placement.cluster_radius;
        center + Vec2::new(angle.cos(), angle.sin()) * radius
    } else {
        Vec2::new(rng.random_range(-49.0..49.0), rng.random_range(-49.0..49.0))
    }
}
fn street_layout(data: &TerrainPresetData, seed: Seed) -> Vec<RoadCorridor> {
    let mut rng = seed.with_label("streets").rng();
    let width = data.placement.street_width;
    match data.placement.settlement {
        Settlement::None | Settlement::Cabin => Vec::new(),
        Settlement::Grid => vec![
            RoadCorridor {
                start: Vec2::new(-50.0, 0.0),
                end: Vec2::new(50.0, 0.0),
                width,
            },
            RoadCorridor {
                start: Vec2::new(0.0, -50.0),
                end: Vec2::new(0.0, 50.0),
                width,
            },
        ],
        Settlement::Village => {
            let offset = rng.random_range(-5.0..5.0);
            vec![
                RoadCorridor {
                    start: Vec2::new(-50.0, -9.0),
                    end: Vec2::new(50.0, 9.0 + offset),
                    width: width * 0.75,
                },
                RoadCorridor {
                    start: Vec2::new(-12.0, -50.0),
                    end: Vec2::new(0.0, 0.0),
                    width: width * 0.6,
                },
            ]
        }
    }
}
fn house_candidate(
    data: &TerrainPresetData,
    roads: &[RoadCorridor],
    radius: f32,
    rng: &mut impl rand::Rng,
) -> (Vec2, f32) {
    if data.placement.settlement == Settlement::Cabin {
        return (
            Vec2::new(rng.random_range(-3.0..3.0), rng.random_range(-3.0..3.0)),
            0.0,
        );
    }
    if roads.is_empty() {
        return (
            Vec2::new(rng.random_range(-40.0..40.0), rng.random_range(-40.0..40.0)),
            rng.random_range(0.0..TAU),
        );
    }
    let road = &roads[rng.random_range(0..roads.len())];
    let along = (road.end - road.start).normalize();
    let side = if rng.random::<bool>() { 1.0 } else { -1.0 };
    let normal = Vec2::new(-along.y, along.x) * side;
    let jitter = if data.placement.settlement == Settlement::Village {
        rng.random_range(0.0..3.0)
    } else {
        0.3
    };
    let xy = road.start.lerp(road.end, rng.random_range(0.08..0.92))
        + normal * (road.width * 0.5 + radius + data.placement.setback + jitter);
    // All existing building generators place their entrance on the local south facade (-Y).
    // Face the nearest road: a house placed along one road can be closer to
    // another one, especially near intersections.
    let nearest = roads
        .iter()
        .min_by(|a, b| a.distance(xy).total_cmp(&b.distance(xy)))
        .unwrap();
    let toward = (nearest.nearest(xy) - xy).normalize_or_zero();
    let toward = if toward == Vec2::ZERO { -normal } else { toward };
    (xy, toward.y.atan2(toward.x) + FRAC_PI_2)
}
