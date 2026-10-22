//! Визуализация растений: превращение [`PlantScene`] в дерево сущностей Bevy.
//!
//! Иерархия соответствует wiki «Создание объекта»: корень сцены «растения»
//! → каждое расставленное растение/луг травы (с `Transform` и `Occupancy`) →
//! его части («ветки», «листва», «пучок травы») → меш и дочернее облако точек.
//!
//! Деревья и кусты спавнятся по одному мешу/облаку на часть. Трава — весь луг
//! одним «холстом»: один общий визуальный меш и одно общее облако точек на
//! все пучки (позиции вплавлены в вершины). Иначе тысячи пучков превратились
//! бы в тысячи сущностей, мешей и материалов.

use bevy::asset::Assets;
use bevy::mesh::Mesh;
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::prelude::*;
use bevy_utils::visuals::materials::{material, material_double_sided};
use bevy_utils::visuals::meshes::mesh_from_polygons;
use generation_core::geometry::Polygon;
use generation_core::occupancy::Occupancy;
use generation_core::point_clouds::visual::{point_cloud_material, point_cloud_mesh};
use generation_core::PointCloud;

use crate::generation::{Part, PlacedTuft, PlantScene};

/// Маркер корневой сущности всей сцены растений.
#[derive(Component, Debug, Clone, Copy)]
pub struct PlantsRoot;

/// Маркер расставленного дерева/куста.
#[derive(Component, Debug, Clone, Copy)]
pub struct PlacedPlantMarker;

/// Маркер расставленного пучка травы.
#[derive(Component, Debug, Clone, Copy)]
pub struct PlacedTuftMarker;

/// Маркер сущности визуального меша (режим `ViewMode::Visual`).
#[derive(Component, Debug, Clone, Copy)]
pub struct VisualMesh;

/// Маркер сущности облака точек (режим `ViewMode::PointCloud`).
#[derive(Component, Debug, Clone, Copy)]
pub struct PointCloudEntity;

/// Имя части (для отладки и будущего UI).
#[derive(Component)]
pub struct PartMarker(pub &'static str);

/// Результат спавна сцены.
pub struct SpawnedPlants {
    /// Корневая сущность.
    pub root: Entity,
    /// Все созданные сущности (для очистки при перегенерации).
    pub entities: Vec<Entity>,
}

/// Спавн всей сцены растений в мир.
pub fn spawn_plant_scene(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    scene: &PlantScene,
) -> SpawnedPlants {
    let mut entities = Vec::new();

    // Генерация идёт в локальной системе с осью Z вверх; в мире Bevy
    // вертикаль — ось Y. Поворот переводит локальную вертикаль в мировую.
    let root = commands
        .spawn((
            PlantsRoot,
            Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            Visibility::default(),
            Name::new("растения"),
        ))
        .id();
    entities.push(root);

    for placed in &scene.plants {
        let occupancy = Occupancy::circle(placed.position, placed.plant.extent * 0.9);
        let transform = Transform::from_translation(Vec3::new(
            placed.position.x,
            placed.position.y,
            0.0,
        ))
        .with_rotation(Quat::from_rotation_z(placed.rotation));
        let entity = commands
            .spawn((
                PlacedPlantMarker,
                occupancy,
                transform,
                Visibility::default(),
                Name::new("растение"),
            ))
            .id();
        entities.push(entity);
        commands.entity(root).add_child(entity);
        spawn_part(commands, meshes, materials, entity, &placed.plant.root, &mut entities);
    }

    if !scene.tufts.is_empty() {
        spawn_grass_field(commands, meshes, materials, root, &scene.tufts, &mut entities);
    }

    SpawnedPlants { root, entities }
}

/// Весь луг травы одним визуальным мешом и одним облаком точек.
///
/// Вместо сущности на каждый пучок собираем все полигоны и все точки облака
/// в один общий меш (позиция и поворот каждого пучка вплавлены в вершины под
/// локальную ось Z вверх; общий поворот Z→Y даст корень сцены). Материал
/// визуального меша и материал облака — по одному на весь луг.
fn spawn_grass_field(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    root: Entity,
    tufts: &[PlacedTuft],
    entities: &mut Vec<Entity>,
) {
    let mut faces: Vec<Polygon> = Vec::new();
    let mut cloud = PointCloud::default();
    if let Some(first) = tufts.first() {
        cloud.default_class = first.tuft.part.cloud.default_class;
    }
    for placed in tufts {
        let mat = Mat4::from_rotation_translation(
            Quat::from_rotation_z(placed.rotation),
            Vec3::new(placed.position.x, placed.position.y, 0.0),
        );
        for poly in &placed.tuft.part.faces {
            faces.push(transform_polygon(poly, mat));
        }
        // Сохраняем классы точек: трансформируем копию и сливаем в общее облако.
        let mut c = placed.tuft.part.cloud.clone();
        c.transform_by(mat);
        cloud.merge(&c);
    }

    let entity = commands
        .spawn((
            PlacedTuftMarker,
            Transform::default(),
            Visibility::default(),
            Name::new("луг травы"),
        ))
        .id();
    entities.push(entity);
    commands.entity(root).add_child(entity);

    if !faces.is_empty() {
        let mesh = meshes.add(mesh_from_polygons(&faces));
        let material = materials.add(material_double_sided(tufts[0].tuft.part.color));
        let mesh_entity = commands
            .spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                VisualMesh,
                Visibility::default(),
            ))
            .id();
        entities.push(mesh_entity);
        commands.entity(entity).add_child(mesh_entity);
    }

    if !cloud.is_empty() {
        let mesh = meshes.add(point_cloud_mesh(&cloud));
        let material = materials.add(point_cloud_material());
        let cloud_entity = commands
            .spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                PointCloudEntity,
                Visibility::default(),
            ))
            .id();
        entities.push(cloud_entity);
        commands.entity(entity).add_child(cloud_entity);
    }
}

/// Повернуть и сместить полигон матрицей (жёсткое преобразование сохраняет
/// ориентацию, поэтому нормали пересчитывать не нужно).
fn transform_polygon(poly: &Polygon, mat: Mat4) -> Polygon {
    let points: Vec<Vec3> = poly
        .points()
        .iter()
        .map(|p| mat.transform_point3(*p))
        .collect();
    Polygon::from_points(points)
}

/// Рекурсивный спавн части.
fn spawn_part(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Entity,
    part: &Part,
    entities: &mut Vec<Entity>,
) {
    let entity = commands
        .spawn((
            part.transform,
            PartMarker(part.name),
            Visibility::default(),
        ))
        .id();
    entities.push(entity);
    commands.entity(parent).add_child(entity);

    // Визуальный меш.
    if !part.faces.is_empty() {
        let mesh = meshes.add(mesh_from_polygons(&part.faces));
        let material = if part.double_sided {
            material_double_sided(part.color)
        } else {
            material(part.color)
        };
        let material = materials.add(material);

        let mesh_entity = commands
            .spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                VisualMesh,
                Visibility::default(),
            ))
            .id();
        entities.push(mesh_entity);
        commands.entity(entity).add_child(mesh_entity);
    }

    // Облако точек.
    if !part.cloud.is_empty() {
        let mesh = meshes.add(point_cloud_mesh(&part.cloud));
        let material = materials.add(point_cloud_material());
        let cloud_entity = commands
            .spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                PointCloudEntity,
                Visibility::default(),
            ))
            .id();
        entities.push(cloud_entity);
        commands.entity(entity).add_child(cloud_entity);
    }

    for child in &part.children {
        spawn_part(commands, meshes, materials, entity, child, entities);
    }
}
