//! Визуализация ландшафта: меши рельефа и воды, облака точек.
//!
//! Иерархия сущностей соответствует wiki «Создание объекта»: корень с
//! маркером → сущности мешей (рельеф, вода) и облаков точек (грунт, вода).
//! Вся геометрия строится в локальной системе с осью Z вверх; корень
//! поворачивается так, чтобы локальная вертикаль совпала с мировой (Y).

use bevy::asset::{Assets, RenderAssetUsages};
use bevy::color::Color;
use bevy::math::{Vec2, Vec3};
use bevy::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::prelude::*;
use bevy_utils::visuals::materials::material;
use generation_core::point_clouds::visual::{point_cloud_material, point_cloud_mesh};

use crate::generation::{falls_clear_offset, FallsCurtain, Heightfield, Landscape, WaterField};

/// Маркер корневой сущности ландшафта (для перегенерации).
#[derive(Component, Debug, Clone, Copy)]
pub struct LandscapeRoot;

/// Маркер визуального меша (показывается в режиме
/// [`ViewMode::Visual`](generation_core::ViewMode)).
#[derive(Component, Debug, Clone, Copy)]
pub struct VisualMesh;

/// Маркер меша/сущности воды.
#[derive(Component, Debug, Clone, Copy)]
pub struct WaterMesh;

/// Маркер сущности облака точек (режим
/// [`ViewMode::PointCloud`](generation_core::ViewMode)).
#[derive(Component, Debug, Clone, Copy)]
pub struct PointCloudEntity;

/// Результат спавна ландшафта.
pub struct SpawnedLandscape {
    /// Корневая сущность.
    pub root: Entity,
    /// Все созданные сущности.
    pub entities: Vec<Entity>,
}

/// Цвет грунта.
const GROUND_COLOR: Color = Color::srgb(0.42, 0.34, 0.25);

/// Спавн ландшафта в мир.
pub fn spawn_landscape(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    landscape: &Landscape,
) -> SpawnedLandscape {
    let mut entities = Vec::new();

    let root = commands
        .spawn((
            LandscapeRoot,
            // Локальная вертикаль Z → мировая Y (как у зданий и растений).
            Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            Visibility::default(),
            Name::new("ландшафт"),
        ))
        .id();
    entities.push(root);

    // --- меш рельефа (визуальный режим) ---
    let ground_mesh = meshes.add(terrain_mesh(&landscape.heightfield));
    let ground_material = materials.add(material(GROUND_COLOR));
    let ground = commands
        .spawn((
            Mesh3d(ground_mesh),
            MeshMaterial3d(ground_material),
            VisualMesh,
            Visibility::default(),
        ))
        .id();
    entities.push(ground);
    commands.entity(root).add_child(ground);

    // --- меш воды ---
    if let Some(water_mesh) = water_mesh(&landscape.heightfield, &landscape.water, &landscape.water_walls) {
        let mesh = meshes.add(water_mesh);
        let material = materials.add(water_material());
        let water = commands
            .spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                VisualMesh,
                WaterMesh,
                Visibility::default(),
            ))
            .id();
        entities.push(water);
        commands.entity(root).add_child(water);
    }

    // --- облака точек (режим облака точек) ---
    if !landscape.ground_cloud.is_empty() {
        let mesh = meshes.add(point_cloud_mesh(&landscape.ground_cloud));
        let material = materials.add(point_cloud_material());
        let cloud = commands
            .spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                PointCloudEntity,
                Visibility::default(),
            ))
            .id();
        entities.push(cloud);
        commands.entity(root).add_child(cloud);
    }
    if !landscape.water_cloud.is_empty() {
        let mesh = meshes.add(point_cloud_mesh(&landscape.water_cloud));
        let material = materials.add(point_cloud_material());
        let cloud = commands
            .spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                PointCloudEntity,
                Visibility::default(),
            ))
            .id();
        entities.push(cloud);
        commands.entity(root).add_child(cloud);
    }

    SpawnedLandscape { root, entities }
}

/// Меш рельефа из поля высот (общие вершины, сглаженные нормали).
pub fn terrain_mesh(hf: &Heightfield) -> Mesh {
    let n = hf.n;
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(hf.values.len());
    let mut normals = vec![Vec3::ZERO; hf.values.len()];
    let mut indices: Vec<u32> = Vec::with_capacity((n - 1).saturating_mul(n - 1) * 6);

    for (i, j, p) in hf.nodes() {
        positions.push([p.x, p.y, hf.values[hf.index(i, j)]]);
    }

    for j in 0..n - 1 {
        for i in 0..n - 1 {
            let a = hf.index(i, j) as u32;
            let b = hf.index(i + 1, j) as u32;
            let c = hf.index(i + 1, j + 1) as u32;
            let d = hf.index(i, j + 1) as u32;
            indices.extend([a, b, c, a, c, d]);

            let pa = Vec3::new(positions[a as usize][0], positions[a as usize][1], positions[a as usize][2]);
            let pb = Vec3::new(positions[b as usize][0], positions[b as usize][1], positions[b as usize][2]);
            let pc = Vec3::new(positions[c as usize][0], positions[c as usize][1], positions[c as usize][2]);
            let pd = Vec3::new(positions[d as usize][0], positions[d as usize][1], positions[d as usize][2]);

            let n1 = (pb - pa).cross(pc - pa);
            let n2 = (pc - pa).cross(pd - pa);
            for corner in [a, b, c] {
                normals[corner as usize] += n1;
            }
            for corner in [a, c, d] {
                normals[corner as usize] += n2;
            }
        }
    }

    let normals: Vec<[f32; 3]> = normals
        .into_iter()
        .map(|v| {
            if v.length_squared() > 1e-12 {
                v.normalize().to_array()
            } else {
                Vec3::Z.to_array()
            }
        })
        .collect();

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Меш воды: ячейки сетки, в которых вся вода присутствует; высота углов —
/// своя у каждого (ровная вода, ленты наклонных рек и стенки водопадов
/// получаются естественно). Ячейки с частичной водой отбрасываются (край
/// уреза слегка «пикселизируется», но не рвётся). На вертикальных обрывах
/// русла дополнительно строятся сплошные «занавесы» (`curtains`) — иначе
/// падение воды на каскаде упирается в тонкую наклонную полоску ячейки и
/// визуально почти не видно.
pub fn water_mesh(
    hf: &Heightfield,
    water: &WaterField,
    curtains: &[FallsCurtain],
) -> Option<Mesh> {
    let n = hf.n;
    let wet = |i: usize, j: usize| water.levels.get(hf.index(i, j)).copied().flatten();

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for j in 0..n - 1 {
        for i in 0..n - 1 {
            let ws = [
                wet(i, j),
                wet(i + 1, j),
                wet(i + 1, j + 1),
                wet(i, j + 1),
            ];
            let [Some(w00), Some(w10), Some(w11), Some(w01)] = ws else {
                continue;
            };
            let p00 = hf.node_xy(i, j);
            let p10 = hf.node_xy(i + 1, j);
            let p11 = hf.node_xy(i + 1, j + 1);
            let p01 = hf.node_xy(i, j + 1);

            // Два треугольника: (00,10,11) и (00,11,01).
            let v00 = Vec3::new(p00.x, p00.y, w00);
            let v10 = Vec3::new(p10.x, p10.y, w10);
            let v11 = Vec3::new(p11.x, p11.y, w11);
            let v01 = Vec3::new(p01.x, p01.y, w01);

            let base = positions.len() as u32;
            let n1 = (v10 - v00).cross(v11 - v00);
            let n2 = (v11 - v00).cross(v01 - v00);
            let n1 = if n1.length_squared() < 1e-12 { Vec3::Z } else { n1.normalize() };
            let n2 = if n2.length_squared() < 1e-12 { Vec3::Z } else { n2.normalize() };

            for v in [v00, v10, v11, v00, v11, v01] {
                positions.push(v.to_array());
            }
            normals.push(n1.to_array());
            normals.push(n1.to_array());
            normals.push(n1.to_array());
            normals.push(n2.to_array());
            normals.push(n2.to_array());
            normals.push(n2.to_array());
            indices.extend([base, base + 1, base + 2, base + 3, base + 4, base + 5]);
        }
    }

    // Одиночный вертикальный «занавес» воды на каскаде: стена поперёк русла
    // от `lo` до `hi`, выдвинутая по потоку настолько ([`falls_clear_offset`]),
    // чтобы её низ не упирался в наклонный грунт под водой (иначе нижняя
    // часть стены оказывается за грунтом и не видна). Колонки, под которыми
    // дно не ушло ниже `lo`, пропускаем.
    let wall_step = 0.5;
    for c in curtains {
        let s_clear = falls_clear_offset(c, &|x, y| hf.height_at(x, y));
        let cols = ((c.hw * 2.0) / wall_step).ceil() as usize + 1;
        let mut prev_xy: Option<Vec2> = None;
        for k in 0..cols {
            let off = -c.hw + k as f32 * wall_step;
            let xy = c.p + c.n * off + c.flow * s_clear;
            // Колонка участвует, если дно под ней уже ушло ниже lo.
            let clear = hf.height_at(xy.x, xy.y) < c.lo - 0.2;
            if clear {
                if let Some(p0) = prev_xy {
                    // Квад между колонками p0 и xy: (p0,hi)-(xy,hi)-(xy,lo)-(p0,lo).
                    let v0 = Vec3::new(p0.x, p0.y, c.hi);
                    let v1 = Vec3::new(xy.x, xy.y, c.hi);
                    let v2 = Vec3::new(xy.x, xy.y, c.lo);
                    let v3 = Vec3::new(p0.x, p0.y, c.lo);
                    let nrm = (v1 - v0).cross(Vec3::Z).normalize_or_zero();
                    let base = positions.len() as u32;
                    for v in [v0, v1, v2, v3] {
                        positions.push(v.to_array());
                        normals.push(nrm.to_array());
                    }
                    indices.extend([base, base + 1, base + 2, base + 2, base + 3, base]);
                }
            }
            prev_xy = if clear { Some(xy) } else { None };
        }
    }

    if positions.is_empty() {
        return None;
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_indices(Indices::U32(indices));
    Some(mesh)
}

/// Материал воды: полупрозрачный, слегка блестящий, видимый с обеих сторон.
pub fn water_material() -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgba(0.16, 0.35, 0.72, 0.55),
        perceptual_roughness: 0.1,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        ..default()
    }
}

/// Полуразмер карты для грубой настройки камеры.
pub fn map_half_size(landscape: &Landscape) -> f32 {
    landscape.heightfield.size_m * 0.5
}
