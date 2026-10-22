//! Генерация растений: чистые данные без привязки к рендерингу.
//!
//! Пресеты ([`PlantPreset`]) разворачиваются в конфигурации ([`resolve`]),
//! из которых строятся отдельные растения (`plant`), пучки травы (`grass`)
//! и целые сцены (`scene`). Модуль пригоден и для headless-приложений
//! (`MinimalPlugins`), и для визуализации (см. [`crate::visual`]).

pub mod config;
pub mod foliage;
pub mod grass;
pub mod part;
pub mod plant;
pub mod preset;
pub mod scene;
pub mod skeleton;

pub use config::{BranchGen, BranchLevels, FoliageConfig, GenerationMode, PlantConfig, PlantPreset, PlantType};
pub use grass::{GrassTuft, PlacedTuft, generate_field, generate_tuft};
pub use part::Part;
pub use plant::Plant;
pub use preset::resolve;
pub use scene::{PlacedPlant, PlantScene};
pub use skeleton::{PlantSkeleton, SkeletonPoint, SkeletonSegment};

/// Реэкспорт для краткости в тестах и приложениях.
pub use config::FoliagePlacement;
pub use config::FoliageStyle;
