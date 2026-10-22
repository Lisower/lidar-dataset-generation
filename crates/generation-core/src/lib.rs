//! Библиотека общих данных, облаков точек и визуальных (полигональных) моделей.
//!
//! Нижний уровень всех генераторов проекта. Здесь:
//! - [`seeds`] — детерминированные seed-ы;
//! - [`occupancy`] — занимаемая территория (квадраты и круги);
//! - [`geometry`] — полигональные модели (грани параллелепипедов, конусов);
//! - [`point_clouds`] — классы, облака точек и их генерация по поверхностям;
//! - [`general`] — общий функционал генераторов (режимы отображения,
//!   перегенерация, горячие клавиши).

pub mod general;
pub mod geometry;
pub mod occupancy;
#[path = "point-clouds/mod.rs"]
pub mod point_clouds;
pub mod seeds;

pub use general::*;
pub use geometry::Polygon;
pub use occupancy::Occupancy;
pub use point_clouds::{Point, PointClass, PointCloud};
pub use seeds::{GlobalSeed, Seed};
