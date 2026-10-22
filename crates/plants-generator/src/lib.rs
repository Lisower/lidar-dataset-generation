//! Генератор растений: деревьев, кустов, травы.
//!
//! Крейт может использоваться двояко:
//! - как библиотека с чистой генерацией данных (`generation`, без рендеринга)
//!   — для headless-приложений (`MinimalPlugins`);
//! - как полноценное приложение с визуализацией (`visual`, `plugin`,
//!   бинарь `plants-generator`).
//!
//! Генерация деревьев и кустов повторяет логику прототипа (рекурсивное
//! ветвление), но с исправлением его багов и в стилистике workspace:
//! иерархия частей, облака точек (`PointCloud`), полигональные модели
//! (`Polygon`), `Occupancy` и полная детерминированность от `Seed` (см.
//! [пресеты](crate::generation::preset)).

pub mod generation;
pub mod plugin;
pub mod visual;

#[cfg(test)]
mod tests;

pub use generation::{
    BranchGen, BranchLevels, FoliageConfig, GenerationMode, GrassTuft, Part, Plant, PlantConfig,
    PlantPreset, PlantScene, PlantSkeleton, PlantType, PlacedPlant, PlacedTuft, resolve,
};
pub use plugin::PlantsPlugin;
pub use visual::{PlantsRoot, PlacedPlantMarker, PlacedTuftMarker, PointCloudEntity, VisualMesh};
