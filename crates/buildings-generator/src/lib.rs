//! Генератор построек.
//!
//! Крейт может использоваться двояко:
//! - как библиотека с чистой генерацией данных (`generation`, без рендеринга)
//!   — для headless-приложений (`MinimalPlugins`);
//! - как полноценное приложение с визуализацией (`visual`, `plugin`,
//!   бинарь `buildings-generator`).

pub mod generation;
pub mod plugin;
pub mod visual;

#[cfg(test)]
mod tests;

pub use generation::{Building, BuildingColors, BuildingConfig, RoofStyle};
pub use plugin::BuildingPlugin;
pub use visual::{BuildingRoot, PointCloudEntity, VisualMesh};
