//! Визуализация здания: превращение [`Building`] в дерево сущностей Bevy.
//!
//! Иерархия соответствует wiki «Создание объекта»: корень с маркером и
//! `Occupancy` → части (этажи → стены → окна/двери/балконы, крыша) →
//! для каждой части своя сущность меша и дочерняя сущность облака точек.

use bevy::asset::Assets;
use bevy::mesh::Mesh;
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::prelude::*;
use bevy_utils::visuals::materials::{material, material_double_sided};
use bevy_utils::visuals::meshes::mesh_from_polygons;
use generation_core::point_clouds::visual::{point_cloud_material, point_cloud_mesh};

use crate::generation::{Building, Part};

/// Маркер корневой сущности здания (для поиска/перегенерации).
#[derive(Component, Debug, Clone, Copy)]
pub struct BuildingRoot;

/// Маркер сущности визуального меша (показывается в режиме
/// [`ViewMode::Visual`](generation_core::ViewMode)).
#[derive(Component, Debug, Clone, Copy)]
pub struct VisualMesh;

/// Маркер сущности облака точек (показывается в режиме
/// [`ViewMode::PointCloud`](generation_core::ViewMode)).
#[derive(Component, Debug, Clone, Copy)]
pub struct PointCloudEntity;

/// Результат спавна здания.
pub struct SpawnedBuilding {
    /// Корневая сущность.
    pub root: Entity,
    /// Все созданные сущности (для очистки при перегенерации).
    pub entities: Vec<Entity>,
}

/// Спавн здания в мир.
pub fn spawn_building(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    building: &Building,
) -> SpawnedBuilding {
    let mut entities = Vec::new();

    let root = commands
        .spawn((
            BuildingRoot,
            building.occupancy,
            // Вся генерация идёт в локальной системе с осью Z вверх, а в мире
            // Bevy вертикаль — ось Y. Поворот переводит локальную вертикаль
            // (Z) в мировую (Y), чтобы здание стояло, а не лежало на боку.
            Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            Visibility::default(),
            Name::new("здание"),
        ))
        .id();
    entities.push(root);

    for child in &building.root.children {
        spawn_part(commands, meshes, materials, root, child, &mut entities);
    }

    SpawnedBuilding { root, entities }
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
        .spawn((part.transform, PartMarker(part.name), Visibility::default()))
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

/// Вспомогательный компонент: имя части (для отладки и будущего UI).
#[derive(Component)]
pub struct PartMarker(pub &'static str);
