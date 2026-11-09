//! Общая сцена сканирования из RON-пресета любого генератора. Мир — Y-up.
use crate::ScanScene;
use bevy::prelude::*;
use generation_core::{
    Seed,
    presets::{PresetMeta, load_preset_config},
};
use landscapes_generator::Landscape;
use std::path::Path;

fn y_up() -> Mat4 {
    Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_2)
}

// Одинаковый контракт иерархии частей обоих генераторов; сохраняем все
// трансформации и реальную геометрию, включая двусторонние листья.
macro_rules! collector {
    ($name:ident, $part:ty) => {
        fn $name(part: &$part, parent: Mat4, triangles: &mut Vec<[Vec3; 3]>) {
            let world = parent
                * Mat4::from_scale_rotation_translation(
                    part.transform.scale,
                    part.transform.rotation,
                    part.transform.translation,
                );
            for face in &part.faces {
                let vertices: Vec<_> = face
                    .triangulate()
                    .map(|p| world.transform_point3(p))
                    .collect();
                triangles.extend(vertices.chunks_exact(3).map(|t| [t[0], t[1], t[2]]));
            }
            for child in &part.children {
                $name(child, world, triangles);
            }
        }
    };
}
collector!(building_triangles, buildings_generator::generation::Part);
collector!(plant_triangles, plants_generator::generation::part::Part);

fn landscape_triangles(landscape: &Landscape, triangles: &mut Vec<[Vec3; 3]>) {
    let h = &landscape.heightfield;
    let point = |i, j| {
        let xy = h.node_xy(i, j);
        y_up().transform_point3(xy.extend(h.get(i, j)))
    };
    for j in 0..h.n.saturating_sub(1) {
        for i in 0..h.n.saturating_sub(1) {
            let [a, b, c, d] = [
                point(i, j),
                point(i + 1, j),
                point(i + 1, j + 1),
                point(i, j + 1),
            ];
            triangles.extend([[a, b, c], [a, c, d]]);
            let water = [
                h.index(i, j),
                h.index(i + 1, j),
                h.index(i + 1, j + 1),
                h.index(i, j + 1),
            ]
            .map(|index| landscape.water.levels[index]);
            let positions = [a, b, c, d];
            for ids in [[0, 1, 2], [0, 2, 3]] {
                if ids.iter().all(|&k| water[k].is_some()) {
                    triangles.push(
                        ids.map(|k| Vec3::new(positions[k].x, water[k].unwrap(), positions[k].z)),
                    );
                }
            }
        }
    }
}

pub fn from_preset(
    meta: &PresetMeta,
    root: &Path,
    seed: Seed,
    source_step: f32,
) -> Result<ScanScene, String> {
    let category = meta.id.split('/').next().unwrap_or("");
    let error = || format!("Не удалось загрузить {}", meta.config_path.display());
    let mut triangles = Vec::new();
    let mut cloud;
    match category {
        "buildings" => {
            let mut config: buildings_generator::BuildingConfig =
                load_preset_config(meta).ok_or_else(error)?;
            config.seed = seed;
            config.point_step = source_step;
            config.wall_point_step = source_step;
            config.point_jitter = 0.0;
            return Ok(ScanScene::from_building(
                &buildings_generator::Building::generate(&config),
            ));
        }
        "plants" => {
            let mut config: plants_generator::PlantConfig =
                load_preset_config(meta).ok_or_else(error)?;
            config.seed = seed;
            config.point_step = source_step;
            config.jitter = 0.0;
            let plant = plants_generator::Plant::generate(&config);
            plant_triangles(&plant.root, y_up(), &mut triangles);
            cloud = plant.point_cloud;
        }
        "landscapes" => {
            let mut config: landscapes_generator::LandscapeConfig =
                load_preset_config(meta).ok_or_else(error)?;
            config.seed = seed;
            config.resolution = source_step;
            config.point_jitter = 0.0;
            let landscape = landscapes_generator::generation::landscape::generate(&config);
            landscape_triangles(&landscape, &mut triangles);
            cloud = landscape.ground_cloud;
            cloud.merge(&landscape.water_cloud);
        }
        "terrains" => {
            let data: terrains_generator::TerrainPresetData =
                load_preset_config(meta).ok_or_else(error)?;
            let scene = terrains_generator::try_generate_from_data(&data, seed, Some(root))?;
            landscape_triangles(&scene.landscape, &mut triangles);
            for placed in &scene.buildings {
                let world = y_up()
                    * Mat4::from_rotation_translation(
                        Quat::from_rotation_z(placed.rotation),
                        placed.position,
                    );
                building_triangles(&placed.building.root, world, &mut triangles);
            }
            for placed in scene.trees.iter().chain(&scene.bushes) {
                let height = scene
                    .landscape
                    .heightfield
                    .height_at(placed.position.x, placed.position.y);
                let world = y_up()
                    * Mat4::from_rotation_translation(
                        Quat::from_rotation_z(placed.rotation),
                        placed.position.extend(height),
                    );
                plant_triangles(&placed.plant.root, world, &mut triangles);
            }
            for instance in &scene.grass.instances {
                plant_triangles(
                    &scene.grass.variants[instance.variant].part,
                    y_up() * instance.matrix(),
                    &mut triangles,
                );
            }
            cloud = scene.point_cloud();
        }
        _ => return Err(format!("Неизвестная категория: {}", meta.id)),
    }
    cloud.transform_by(y_up());
    Ok(ScanScene::new(cloud, triangles))
}
