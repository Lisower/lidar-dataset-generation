//! Генерация зданий: чистые данные без привязки к рендерингу.
//!
//! Модуль пригоден и для headless-приложений (`MinimalPlugins`), и для
//! визуализации (см. [`crate::visual`]).

pub mod building;
pub mod config;
pub mod elements;
pub mod palette;
pub mod part;
pub mod roof;
pub mod preset;

pub use building::{Building, BuildingColors};
pub use config::{BuildingConfig, BuildingPreset, RoofStyle};
pub use part::Part;
