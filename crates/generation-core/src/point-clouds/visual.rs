//! Визуализация облака точек: меш с одной вершиной на точку.
//!
//! Вершины облака используются как **инстансы**: на каждую точку рисуется
//! экранный билборд (4 вершины), а позиция, цвет и базовый размер точки
//! приходят из вершинного буфера. Такой путь не раздувает геометрию в 4–6 раз
//! и работает быстрее, чем отдельные квады на CPU.
//!
//! Отрисовку выполняет [`render`](super::render): собственный конвейер
//! `Transparent3d`, мягкие круглые точки и масштабирование размера по
//! расстоянию до камеры.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Mesh, MeshVertexAttribute, PrimitiveTopology, VertexFormat};
use bevy::prelude::*;

use super::cloud::PointCloud;
use super::render::PointCloudTag;

/// Базовый размер точки в пикселях (атрибут вершины).
///
/// Хранится рядом с позицией и цветом, чтобы шейдер мог задать разный
/// размер для разных классов (`PointClass::point_size`).
pub const ATTRIBUTE_POINT_SIZE: MeshVertexAttribute =
    MeshVertexAttribute::new("PointCloudSize", 988_540_917, VertexFormat::Float32);

/// Меш облака точек: по одной вершине на точку.
///
/// Топология `PointList` сохранена для совместимости с данными и тестами;
/// пользовательский конвейер всё равно рисует четыре вершины на инстанс.
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

    let sizes: Vec<f32> = cloud.points.iter().map(|p| p.class.point_size()).collect();

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_attribute(ATTRIBUTE_POINT_SIZE, sizes);
    mesh
}

/// Спавн сущности облака точек.
///
/// Возвращает `Entity`, чтобы вызывающий код мог добавить собственный маркер
/// (`PointCloudEntity`) и привязать облако к иерархии сцены.
pub fn spawn_point_cloud(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    cloud: &PointCloud,
) -> Entity {
    commands
        .spawn((
            Mesh3d(meshes.add(point_cloud_mesh(cloud))),
            PointCloudTag,
            Visibility::default(),
        ))
        .id()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::point_clouds::{Point, PointClass};
    use bevy::mesh::VertexAttributeValues;

    #[test]
    fn point_cloud_mesh_stores_per_class_sizes() {
        let cloud = PointCloud {
            points: vec![
                Point::new(Vec3::new(0.0, 0.0, 0.0), PointClass::Ground),
                Point::new(Vec3::new(1.0, 0.0, 0.0), PointClass::LowNoise),
                Point::new(Vec3::new(2.0, 0.0, 0.0), PointClass::Water),
            ],
            default_class: PointClass::Other,
        };

        let mesh = point_cloud_mesh(&cloud);

        assert_eq!(mesh.count_vertices(), cloud.len());
        assert_eq!(mesh.primitive_topology(), PrimitiveTopology::PointList);

        let Some(VertexAttributeValues::Float32(sizes)) = mesh.attribute(ATTRIBUTE_POINT_SIZE)
        else {
            panic!("в меше нет атрибута размера точки");
        };
        let expected: Vec<f32> = cloud.points.iter().map(|p| p.class.point_size()).collect();
        assert_eq!(sizes, &expected);
        assert!(sizes[1] >= 2.0 * sizes[0]);

        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("в меше нет позиций");
        };
        assert_eq!(positions.len(), cloud.len());

        let Some(VertexAttributeValues::Float32x4(colors)) = mesh.attribute(Mesh::ATTRIBUTE_COLOR)
        else {
            panic!("в меше нет цветов");
        };
        assert_eq!(colors.len(), cloud.len());
        assert!(colors.iter().all(|color| color[3] == 1.0));
    }
}
