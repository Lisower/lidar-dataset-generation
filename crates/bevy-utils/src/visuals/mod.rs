//! Визуальные примитивы: создание мешей и материалов.
//!
//! Меши строятся из полигонов [`generation_core::geometry::Polygon`] — те же
//! поверхности, из которых генераторы делают облака точек, поэтому визуальная
//! модель и облако не расходятся.

pub mod materials;
pub mod meshes;

pub use materials::{material, material_double_sided};
pub use meshes::{
    box_mesh, cone_mesh, mesh_from_polygons, n_gon_mesh, quad_mesh, triangle_mesh,
};
