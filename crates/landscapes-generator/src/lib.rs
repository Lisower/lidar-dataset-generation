//! Генератор базовых ландшафтов: рельеф (грунт) и вода.
//!
//! Крейт может использоваться двояко:
//! - как библиотека с чистой генерацией данных (`generation`, без рендеринга)
//!   — для headless-приложений (`MinimalPlugins`);
//! - как полноценное приложение с визуализацией (`visual`, `plugin`,
//!   бинарь `landscapes-generator`).
//!
//! Ландшафт — это квадрат 100×100 м: грунт с линиями-деформациями рельефа
//! и fBm-«каменистостью», вода (горизонтальная плоскость и/или ленты вдоль
//! русел наклонных рек) и облака точек (под водой грунтовых точек нет).
//! Все пресеты перечислены в [`LandscapePreset`], детерминированный разброс
//! от seed — в [`resolve`].

pub mod generation;
pub mod plugin;
pub mod visual;

#[cfg(test)]
mod tests;

pub use generation::{
    BasePlane, Curve, Heightfield, Landscape, LandscapeConfig, LandscapePreset, Longitudinal,
    NoiseSampler, Profile, TerrainNoise, resolve,
};
pub use plugin::LandscapePlugin;
pub use visual::{LandscapeRoot, PointCloudEntity, VisualMesh, WaterMesh};
