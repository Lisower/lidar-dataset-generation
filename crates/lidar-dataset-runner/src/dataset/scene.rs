//! Композиция сцены: ландшафт и расстановка объектов.
//!
//! Одна сцена — это ландшафт 100×100 м и объекты на нём. Порядок расстановки
//! и константы повторяют `terrains_generator::scene::generate_impl`, чтобы
//! сцена в раннере вела себя как сцена в существующем просмотрщике; общим
//! остаётся `PlacementMap`, который и проверяет пересечения, уклон, воду
//! и края карты.
//!
//! Все кандидаты берутся из сида сцены, поэтому один и тот же
//! `(рецепт, индекс)` даёт побайтово одинаковую сцену.

use std::collections::BTreeMap;
use std::f32::consts::TAU;

use bevy::math::{Vec2, Vec3};
use buildings_generator::{Building, BuildingConfig};
use generation_core::presets::load_preset_config;
use generation_core::{Seed};
use landscapes_generator::{Landscape, LandscapeConfig, LandscapePreset, generation::landscape};
use plants_generator::generation::grass::generate_tuft;
use plants_generator::{GrassTuft, Plant, PlantConfig, PlantType};
use rand::RngExt;
use terrains_generator::{PlacementMap, PlacementRules};

use super::catalog::Catalog;
use super::recipe::{ElementKind, ElementSpec, Recipe};
use crate::error::RunnerError;

/// Объект, расставленный на сцене: положение, поворот и сам объект.
///
/// `PlacementMap::try_place` возвращает только позицию и занятую территорию,
/// поэтому поворот хранится здесь: он нужен и облаку точек, и будущему
/// экспорту поз.
#[derive(Debug, Clone)]
pub struct Placed<T> {
    /// Точка на поверхности рельефа; `z` уже равен высоте рельефа.
    pub position: Vec3,
    /// Поворот вокруг вертикали, радианы.
    pub rotation: f32,
    /// Сам объект в локальных координатах.
    pub item: T,
}

/// Одна собранная сцена.
#[derive(Debug)]
pub struct Scene {
    /// Сид сцены (производный от сида запуска и индекса).
    pub seed: Seed,
    /// id пресета ландшафта.
    pub landscape_name: String,
    /// Ландшафт с рельефом и облаками грунта и воды.
    pub landscape: Landscape,
    /// Поставленные здания.
    pub buildings: Vec<Placed<Building>>,
    /// Поставленные деревья.
    pub trees: Vec<Placed<Plant>>,
    /// Поставленные кусты.
    pub bushes: Vec<Placed<Plant>>,
    /// Поставленные пучки травы.
    pub grass: Vec<Placed<GrassTuft>>,
    /// Запрошенное количество по видам элементов.
    pub requested: BTreeMap<String, u32>,
}

/// Пределы случайных координат кандидатов для расстановки.
const BUILDING_SPREAD_M: f32 = 45.0;
const VEGETATION_SPREAD_M: f32 = 49.0;
const TREE_CLUSTERS: usize = 7;
const BUSH_CLUSTERS: usize = 10;
const GRASS_CLUSTERS: usize = 14;
const TREE_CLUSTER_RADIUS_M: f32 = 17.0;
const GRASS_CLUSTER_RADIUS_M: f32 = 11.0;
const CLUSTER_PROBABILITY: f32 = 0.78;
/// Сколько попыток даётся одному объекту, прежде чем сцена останется без него.
const BUILDING_ATTEMPTS: usize = 120;
const PLANT_ATTEMPTS: usize = 90;
const GRASS_ATTEMPTS: usize = 50;

/// Собрать сцену `index` из рецепта.
///
/// Сид сцены производный: `Seed::new(recipe.seed).child(index)`. Поэтому один
/// и тот же `(рецепт, индекс)` всегда даёт побайтово одинаковое облако.
pub fn compose(catalog: &Catalog, recipe: &Recipe, index: u32) -> Result<Scene, RunnerError> {
    let scene_seed = Seed::new(recipe.seed).child(u64::from(index));
    let landscape = build_landscape(catalog, recipe, scene_seed)?;
    let mut map = PlacementMap::new(&landscape);

    let buildings = place_buildings(catalog, recipe, scene_seed, &mut map)?;
    let trees = place_plants(ElementKind::Tree, catalog, recipe, scene_seed, &mut map)?;
    let bushes = place_plants(ElementKind::Bush, catalog, recipe, scene_seed, &mut map)?;
    let grass = place_grass(catalog, recipe, scene_seed, &mut map)?;

    Ok(Scene {
        seed: scene_seed,
        landscape_name: recipe.landscape.clone(),
        landscape,
        buildings,
        trees,
        bushes,
        grass,
        requested: recipe.requested(),
    })
}

/// Ландшафт сцены по правилу разрешения пресетов.
///
/// Для архетипных `kind` конфиг пересобирается от сида сцены: каталожный
/// `config.ron` — «запечённая» геометрия от `Seed::new(0)`, и без пересборки
/// рельеф не зависел бы от сида. Кастомный пресет с неизвестным `kind` берётся
/// из файла с заменой сида.
fn build_landscape(
    catalog: &Catalog,
    recipe: &Recipe,
    scene_seed: Seed,
) -> Result<Landscape, RunnerError> {
    let missing = || RunnerError::MissingPreset {
        category: "landscapes".to_string(),
        id: recipe.landscape.clone(),
    };
    let meta = catalog.meta("landscapes", &recipe.landscape).ok_or_else(missing)?;

    let landscape_seed = scene_seed.child(1);
    let mut cfg = match LandscapePreset::from_kind(&meta.kind) {
        Some(archetype) => landscapes_generator::resolve(archetype, landscape_seed),
        None => {
            let mut loaded =
                load_preset_config::<LandscapeConfig>(&meta).ok_or_else(|| {
                    RunnerError::UnsupportedPreset {
                        id: recipe.landscape.clone(),
                    }
                })?;
            loaded.seed = landscape_seed;
            loaded
        }
    };
    cfg.size_m = 100.0;

    // Фундаменты зданий должны стоять на ровном грунте (как в terrains-generator).
    if recipe.has_buildings() {
        cfg.noise.amplitude = cfg.noise.amplitude.min(0.25);
    }

    Ok(landscape::generate(&cfg))
}

/// Конфиг растения из каталога; ошибка, если пресета нет или он нечитаем.
fn plant_config(catalog: &Catalog, id: &str) -> Result<PlantConfig, RunnerError> {
    let meta = catalog
        .meta("plants", id)
        .ok_or_else(|| RunnerError::MissingPreset {
            category: "plants".to_string(),
            id: id.to_string(),
        })?;
    load_preset_config::<PlantConfig>(&meta).ok_or_else(|| RunnerError::UnsupportedPreset {
        id: id.to_string(),
    })
}

/// Конфиг здания из каталога; ошибка, если пресета нет или он нечитаем.
fn building_config(catalog: &Catalog, id: &str) -> Result<BuildingConfig, RunnerError> {
    let meta = catalog
        .meta("buildings", id)
        .ok_or_else(|| RunnerError::MissingPreset {
            category: "buildings".to_string(),
            id: id.to_string(),
        })?;
    load_preset_config::<BuildingConfig>(&meta).ok_or_else(|| RunnerError::UnsupportedPreset {
        id: id.to_string(),
    })
}

/// Элемент рецепта нужного вида с ненулевым количеством.
fn element(recipe: &Recipe, kind: ElementKind) -> Option<&ElementSpec> {
    recipe
        .elements
        .iter()
        .find(|e| e.kind == kind && e.count > 0)
}

/// Случайная точка-кандидат вокруг одного из кластеров либо по всей карте.
fn vegetation_candidate(
    rng: &mut impl rand::Rng,
    anchors: &[Vec2],
    cluster_radius: f32,
) -> Vec2 {
    if !anchors.is_empty() && rng.random::<f32>() < CLUSTER_PROBABILITY {
        let center = anchors[rng.random_range(0..anchors.len())];
        let r = rng.random::<f32>().sqrt() * cluster_radius;
        let angle = rng.random_range(0.0..TAU);
        center + Vec2::new(angle.cos() * r, angle.sin() * r)
    } else {
        let half = VEGETATION_SPREAD_M;
        Vec2::new(rng.random_range(-half..half), rng.random_range(-half..half))
    }
}

/// Центры кластеров растительности.
fn clusters(seed: Seed, count: usize) -> Vec<Vec2> {
    let mut rng = seed.with_label("clusters").rng();
    (0..count)
        .map(|_| Vec2::new(rng.random_range(-38.0..38.0), rng.random_range(-38.0..38.0)))
        .collect()
}

/// Поставить здания.
///
/// Здания ставятся первыми: у них самая большая занимаемая площадь, и на
/// тесной карте растительности проще найти место вокруг них, чем наоборот.
/// Каждое здание генерируется со своим дочерним сидом, поворот берётся из
/// отдельного потока случайности.
fn place_buildings(
    catalog: &Catalog,
    recipe: &Recipe,
    scene_seed: Seed,
    map: &mut PlacementMap<'_>,
) -> Result<Vec<Placed<Building>>, RunnerError> {
    let Some(spec) = element(recipe, ElementKind::Building) else {
        return Ok(Vec::new());
    };
    let base = building_config(catalog, &spec.preset_id)?;

    let mut positions = scene_seed.with_label("building-positions").rng();
    let mut rotations = scene_seed.with_label("building-rotation").rng();
    let mut placed = Vec::new();

    for index in 0..spec.count {
        let cfg = BuildingConfig {
            seed: scene_seed.child(10 + u64::from(index)),
            ..base.clone()
        };
        let building = Building::generate(&cfg);
        let rules = PlacementRules {
            radius: building.occupancy.shape.bounding_radius(),
            clearance: 1.5,
            max_slope_deg: recipe.max_slope_deg,
        };
        for _ in 0..BUILDING_ATTEMPTS {
            let candidate = Vec2::new(
                positions.random_range(-BUILDING_SPREAD_M..BUILDING_SPREAD_M),
                positions.random_range(-BUILDING_SPREAD_M..BUILDING_SPREAD_M),
            );
            if let Some(positioned) = map.try_place(candidate, rules) {
                placed.push(Placed {
                    position: positioned.position,
                    rotation: rotations.random_range(0.0..TAU),
                    item: building,
                });
                break;
            }
        }
    }

    Ok(placed)
}

/// Поставить деревья или кусты.
///
/// Тип пресета обязан совпадать с видом элемента: прогон травяного конфига
/// через `Plant::generate` даёт пустые ветки, а дерева через `generate_tuft` —
/// мусор. Поэтому пара проверяется по
/// `PlantConfig.preset.plant_type()` до генерации и отсекается ошибкой.
fn place_plants(
    kind: ElementKind,
    catalog: &Catalog,
    recipe: &Recipe,
    scene_seed: Seed,
    map: &mut PlacementMap<'_>,
) -> Result<Vec<Placed<Plant>>, RunnerError> {
    let Some(spec) = element(recipe, kind) else {
        return Ok(Vec::new());
    };
    let base = plant_config(catalog, &spec.preset_id)?;
    if Some(base.preset.plant_type()) != expected_plant_type(kind) {
        return Err(RunnerError::UnsupportedPreset {
            id: spec.preset_id.clone(),
        });
    }

    let clusters = clusters(scene_seed, plant_cluster_count(kind));
    let cluster_radius = plant_cluster_radius(kind);
    let mut rng = scene_seed.with_label(kind.key()).rng();
    let mut placed = Vec::new();

    for index in 0..spec.count {
        let obj_seed = scene_seed.with_label(kind.key()).child(u64::from(index));
        let plant = Plant::generate(&base.with_seed(obj_seed));
        let rules = PlacementRules {
            radius: plant.extent.max(0.1),
            clearance: plant_clearance(kind),
            max_slope_deg: 30.0,
        };
        for _ in 0..PLANT_ATTEMPTS {
            let candidate = vegetation_candidate(&mut rng, &clusters, cluster_radius);
            if let Some(positioned) = map.try_place(candidate, rules) {
                placed.push(Placed {
                    position: positioned.position,
                    rotation: rng.random_range(0.0..TAU),
                    item: plant,
                });
                break;
            }
        }
    }

    Ok(placed)
}

/// Поставить траву.
///
/// Трава ставится последней: пучки мелкие, и на тесной карте они должны
/// заполнять остатки, а не отбирать место у зданий и деревьев.
fn place_grass(
    catalog: &Catalog,
    recipe: &Recipe,
    scene_seed: Seed,
    map: &mut PlacementMap<'_>,
) -> Result<Vec<Placed<GrassTuft>>, RunnerError> {
    let Some(spec) = element(recipe, ElementKind::Grass) else {
        return Ok(Vec::new());
    };
    let base = plant_config(catalog, &spec.preset_id)?;
    if base.preset.plant_type() != PlantType::Grass {
        return Err(RunnerError::UnsupportedPreset {
            id: spec.preset_id.clone(),
        });
    }

    let clusters = clusters(scene_seed.with_label("grass-anchors-seed"), GRASS_CLUSTERS);
    let mut rng = scene_seed.with_label("grass-positions").rng();
    let mut placed = Vec::new();

    for index in 0..spec.count {
        let tuft_seed = scene_seed.with_label("grass-model").child(u64::from(index));
        let tuft = generate_tuft(&base.with_seed(tuft_seed), tuft_seed);
        let rules = PlacementRules {
            radius: tuft.radius.max(0.05),
            clearance: 0.02,
            max_slope_deg: 35.0,
        };
        for _ in 0..GRASS_ATTEMPTS {
            let candidate = vegetation_candidate(&mut rng, &clusters, GRASS_CLUSTER_RADIUS_M);
            if let Some(positioned) = map.try_place(candidate, rules) {
                placed.push(Placed {
                    position: positioned.position,
                    rotation: rng.random_range(0.0..TAU),
                    item: tuft,
                });
                break;
            }
        }
    }

    Ok(placed)
}

/// Тип растения, который обязан быть у пресета этого вида элемента.
fn expected_plant_type(kind: ElementKind) -> Option<PlantType> {
    match kind {
        ElementKind::Building => None,
        ElementKind::Tree => Some(PlantType::Tree),
        ElementKind::Bush => Some(PlantType::Bush),
        ElementKind::Grass => Some(PlantType::Grass),
    }
}

/// Сколько кластеров растительности используется для вида элемента.
fn plant_cluster_count(kind: ElementKind) -> usize {
    match kind {
        ElementKind::Bush => BUSH_CLUSTERS,
        _ => TREE_CLUSTERS,
    }
}

/// Радиус кластера растительности для вида элемента.
fn plant_cluster_radius(kind: ElementKind) -> f32 {
    match kind {
        ElementKind::Tree => TREE_CLUSTER_RADIUS_M,
        _ => GRASS_CLUSTER_RADIUS_M,
    }
}

/// Зазор вокруг объекта при расстановке.
fn plant_clearance(kind: ElementKind) -> f32 {
    match kind {
        ElementKind::Bush => 0.2,
        _ => 0.5,
    }
}


#[cfg(test)]
mod tests {
    use generation_core::PointClass;
    use generation_core::point_clouds::export::to_xyz;

    use super::*;
    use crate::dataset::catalog::load_catalog;
    use crate::dataset::merge::scene_cloud;
    use crate::dataset::test_support::repo_presets;

    #[test]
    fn landscape_scene_is_deterministic_and_covers_the_map() {
        let catalog = load_catalog(&repo_presets());
        let recipe = Recipe {
            scenes: 1,
            seed: 0,
            landscape: "plain".into(),
            max_slope_deg: 25.0,
            elements: vec![ElementSpec {
                kind: ElementKind::Building,
                preset_id: "rectangular".into(),
                count: 1,
            }],
        };
        let a = scene_cloud(&compose(&catalog, &recipe, 0).unwrap());
        let b = scene_cloud(&compose(&catalog, &recipe, 0).unwrap());
        assert_eq!(to_xyz(&a), to_xyz(&b));
        assert!(a.len() > 10_000);
        let classes: std::collections::BTreeSet<u8> =
            a.points.iter().map(|p| p.class.code()).collect();
        assert!(classes.contains(&PointClass::Ground.code()));
        assert!(
            a.points
                .iter()
                .all(|p| p.position.x.abs() <= 50.5 && p.position.y.abs() <= 50.5)
        );
    }

    #[test]
    fn different_scene_indices_differ() {
        let catalog = load_catalog(&repo_presets());
        let recipe = Recipe {
            scenes: 3,
            seed: 0,
            landscape: "slope".into(),
            max_slope_deg: 25.0,
            elements: vec![ElementSpec {
                kind: ElementKind::Tree,
                preset_id: "deciduous-tree".into(),
                count: 1,
            }],
        };
        assert_ne!(
            to_xyz(&scene_cloud(&compose(&catalog, &recipe, 0).unwrap())),
            to_xyz(&scene_cloud(&compose(&catalog, &recipe, 1).unwrap()))
        );
    }

    #[test]
    fn unknown_landscape_is_a_typed_error() {
        let catalog = load_catalog(&repo_presets());
        let recipe = Recipe {
            scenes: 1,
            seed: 0,
            landscape: "нет-такого".into(),
            max_slope_deg: 25.0,
            elements: vec![ElementSpec {
                kind: ElementKind::Tree,
                preset_id: "deciduous-tree".into(),
                count: 1,
            }],
        };
        assert!(matches!(
            compose(&catalog, &recipe, 0),
            Err(RunnerError::MissingPreset { .. })
        ));
    }

    #[test]
    fn vegetation_candidates_stay_inside_the_map() {
        let mut rng = Seed::new(7).rng();
        let anchors = clusters(Seed::new(7), TREE_CLUSTERS);
        for _ in 0..500 {
            let p = vegetation_candidate(&mut rng, &anchors, TREE_CLUSTER_RADIUS_M);
            assert!(p.abs().max_element() <= 50.0, "{p:?}");
        }
    }

    #[test]
    fn buildings_are_placed_once_and_keep_class() {
        let catalog = load_catalog(&repo_presets());
        let recipe = Recipe {
            scenes: 1,
            seed: 7,
            landscape: "plain".into(),
            max_slope_deg: 10.0,
            elements: vec![ElementSpec {
                kind: ElementKind::Building,
                preset_id: "rectangular".into(),
                count: 3,
            }],
        };
        let scene = compose(&catalog, &recipe, 0).unwrap();
        assert!(scene.buildings.len() <= 3);
        assert!(!scene.buildings.is_empty(), "здание должно поместиться");
        let cloud = scene_cloud(&scene);
        let building_points = cloud
            .points
            .iter()
            .filter(|p| p.class == PointClass::Building)
            .count();
        assert!(building_points > 0);
        let b = &scene.buildings[0];
        let ground = scene
            .landscape
            .heightfield
            .height_at(b.position.x, b.position.y);
        assert!((b.position.z - ground).abs() < 0.01);
    }

    #[test]
    fn excessive_buildings_fit_or_are_reported_not_hung() {
        let catalog = load_catalog(&repo_presets());
        let recipe = Recipe {
            scenes: 1,
            seed: 3,
            landscape: "plain".into(),
            max_slope_deg: 20.0,
            elements: vec![ElementSpec {
                kind: ElementKind::Building,
                preset_id: "apartment-block".into(),
                count: 300,
            }],
        };
        let started = std::time::Instant::now();
        let scene = compose(&catalog, &recipe, 0).unwrap();
        assert!(
            started.elapsed().as_secs() < 60,
            "расстановка не должна зависать"
        );
        assert!(scene.buildings.len() <= 300);
        assert_eq!(scene.requested["building"], 300);
    }

    #[test]
    fn vegetation_is_placed_and_keeps_class() {
        let catalog = load_catalog(&repo_presets());
        let recipe = Recipe {
            scenes: 1,
            seed: 11,
            landscape: "plain".into(),
            max_slope_deg: 30.0,
            elements: vec![
                ElementSpec {
                    kind: ElementKind::Tree,
                    preset_id: "deciduous-tree".into(),
                    count: 6,
                },
                ElementSpec {
                    kind: ElementKind::Bush,
                    preset_id: "bush".into(),
                    count: 4,
                },
                ElementSpec {
                    kind: ElementKind::Grass,
                    preset_id: "tall-grass".into(),
                    count: 30,
                },
            ],
        };
        let scene = compose(&catalog, &recipe, 0).unwrap();
        assert_eq!(scene.trees.len(), 6);
        assert!(scene.bushes.len() <= 4);
        assert!(!scene.grass.is_empty());
        let cloud = scene_cloud(&scene);
        assert!(
            cloud
                .points
                .iter()
                .any(|p| p.class == PointClass::Vegetation)
        );
        let tree = &scene.trees[0];
        let base_z = tree.position.z;
        let lowest = tree
            .item
            .point_cloud
            .points
            .iter()
            .map(|p| p.position.z)
            .fold(f32::INFINITY, f32::min);
        assert!(
            lowest.abs() < 0.5,
            "облако растения начинается от основания: {lowest}"
        );
        let ground = scene
            .landscape
            .heightfield
            .height_at(tree.position.x, tree.position.y);
        assert!((base_z - ground).abs() < 0.01);
        assert_eq!(scene.requested["tree"], 6);
        assert_eq!(scene.requested["bush"], 4);
        assert_eq!(scene.requested["grass"], 30);
    }

    #[test]
    fn grass_preset_is_refused_for_a_tree_element() {
        let catalog = load_catalog(&repo_presets());
        let recipe = Recipe {
            scenes: 1,
            seed: 1,
            landscape: "plain".into(),
            max_slope_deg: 30.0,
            elements: vec![ElementSpec {
                kind: ElementKind::Tree,
                preset_id: "tall-grass".into(),
                count: 2,
            }],
        };
        // Композиция не паникует на несовместимой паре: генератор растения
        // строит не то, что ждёт расстановка, поэтому пара отсекается ошибкой.
        assert!(matches!(
            compose(&catalog, &recipe, 0),
            Err(RunnerError::UnsupportedPreset { .. })
        ));
    }

    #[test]
    fn vegetation_clouds_are_deterministic() {
        let catalog = load_catalog(&repo_presets());
        let recipe = Recipe {
            scenes: 1,
            seed: 21,
            landscape: "plain".into(),
            max_slope_deg: 30.0,
            elements: vec![
                ElementSpec {
                    kind: ElementKind::Tree,
                    preset_id: "deciduous-tree".into(),
                    count: 3,
                },
                ElementSpec {
                    kind: ElementKind::Grass,
                    preset_id: "sparse-grass".into(),
                    count: 10,
                },
            ],
        };
        let a = scene_cloud(&compose(&catalog, &recipe, 0).unwrap());
        let b = scene_cloud(&compose(&catalog, &recipe, 0).unwrap());
        assert_eq!(to_xyz(&a), to_xyz(&b));
    }
}

