//! Визуализация облака точек: меш примитива `PointList` с цветом на точку.
//!
//! Каждая точка — отдельная вершина с атрибутом цвета; в шейдере PBR вершины
//! перекрывают `base_color` материала, поэтому материал ставится белым, а
//! цвета приходят прямо из точек (см. `PointClass::color`).

use bevy::asset::RenderAssetUsages;
use bevy::color::Color;
use bevy::mesh::{Mesh, PrimitiveTopology};
use bevy::prelude::default;
use bevy::pbr::StandardMaterial;

use super::cloud::PointCloud;

/// Меш, отрисовывающий точки облака (`PointList`).
pub fn point_cloud_mesh(cloud: &PointCloud) -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::PointList, RenderAssetUsages::default());

    let positions: Vec<[f32; 3]> = cloud
        .points
        .iter()
        .map(|p| [p.position.x, p.position.y, p.position.z])
        .collect();

    let colors: Vec<[f32; 4]> = cloud
        .points
        .iter()
        .map(|p| {
            let c = p.class.color().to_srgba();
            [c.red, c.green, c.blue, 1.0]
        })
        .collect();

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh
}

/// Материал для визуализации облака точек: неосвещаемый, цвет из вершин.
pub fn point_cloud_material() -> StandardMaterial {
    StandardMaterial {
        base_color: Color::WHITE, // цвет приходит из вершин
        unlit: true,
        ..default()
    }
}
