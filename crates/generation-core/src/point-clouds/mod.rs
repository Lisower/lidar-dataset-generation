//! Облака точек: классы и цвета, данные, генерация по поверхностям примитивов,
//! визуализация как `Entity` облака точек.

pub mod classes;
pub mod cloud;
pub mod primitives;
pub mod visual;

pub use classes::PointClass;
pub use cloud::{Point, PointCloud};
