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
use plants_generator::{GrassTuft, Plant, PlantConfig};
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

/// Ландшафт сцены по правилу разрешения пресетов (`DESIGN.md` §3.4).
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

/// Поставить здания (пока не реализовано).
fn place_buildings(
    _catalog: &Catalog,
    _recipe: &Recipe,
    _scene_seed: Seed,
    _map: &mut PlacementMap<'_>,
) -> Result<Vec<Placed<Building>>, RunnerError> {
    Ok(Vec::new())
}

/// Поставить деревья или кусты (пока не реализовано).
fn place_plants(
    _kind: ElementKind,
    _catalog: &Catalog,
    _recipe: &Recipe,
    _scene_seed: Seed,
    _map: &mut PlacementMap<'_>,
) -> Result<Vec<Placed<Plant>>, RunnerError> {
    Ok(Vec::new())
}

/// Поставить траву (пока не реализовано).
fn place_grass(
    _catalog: &Catalog,
    _recipe: &Recipe,
    _scene_seed: Seed,
    _map: &mut PlacementMap<'_>,
) -> Result<Vec<Placed<GrassTuft>>, RunnerError> {
    Ok(Vec::new())
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
}
