//! Создание мешей: прямоугольного параллелепипеда, конуса (в т.ч.
//! усечённого), треугольника, четырёхугольника, N-гранника — с нормалями.

use bevy::asset::RenderAssetUsages;
use bevy::math::Vec3;
use bevy::mesh::{Indices, Mesh, PrimitiveTopology};

use generation_core::geometry::{Polygon, box_polygons, cone_polygons, n_gon_polygon};

/// Собрать меш из набора плоских полигонов.
///
/// Нормаль вычисляется для каждой грани отдельно (плоское затенение).
pub fn mesh_from_polygons(polygons: &[Polygon]) -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());

    let mut vertices: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for polygon in polygons {
        let n = polygon.points().len();
        if n < 3 {
            continue;
        }
        let normal = polygon.normal();

        // Веер из первой вершины.
        for i in 1..n - 1 {
            indices.push(vertices.len() as u32);
            indices.push(vertices.len() as u32 + i as u32);
            indices.push(vertices.len() as u32 + i as u32 + 1);
        }

        for p in polygon.points() {
            vertices.push([p.x, p.y, p.z]);
            normals.push([normal.x, normal.y, normal.z]);
        }
    }

    if vertices.is_empty() {
        return mesh;
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// Меш прямоугольного параллелепипеда (6 граней), центр в начале координат.
pub fn box_mesh(size: Vec3) -> Mesh {
    mesh_from_polygons(&box_polygons(size))
}

/// Меш одного четырёхугольника.
pub fn quad_mesh(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> Mesh {
    mesh_from_polygons(&[Polygon::quad(a, b, c, d)])
}

/// Меш одного треугольника.
pub fn triangle_mesh(a: Vec3, b: Vec3, c: Vec3) -> Mesh {
    mesh_from_polygons(&[Polygon::triangle(a, b, c)])
}

/// Меш правильного N-гранника: передняя грань — полигон в плоскости XY.
pub fn n_gon_mesh(sides: usize, radius: f32) -> Mesh {
    mesh_from_polygons(&[n_gon_polygon(sides, radius, Vec3::Z, Vec3::X)])
}

/// Меш конуса (в т.ч. усечённого): боковая поверхность + основания.
/// Ось вдоль Z, нижнее основание радиуса `radius_bottom` на `z = 0`.
pub fn cone_mesh(radius_bottom: f32, radius_top: f32, height: f32, sides: usize) -> Mesh {
    let polygons = cone_polygons(radius_bottom, radius_top, height, sides, Vec3::Z);
    mesh_from_polygons(&polygons)
}
