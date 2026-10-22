//! Генерация ландшафта: чистые данные без привязки к рендерингу.
//!
//! Ландшафт 100×100 м: грунт (рельеф) + вода + облака точек. Генерация
//! полностью детерминирована от seed и построена вокруг «линий деформации»
//! рельефа (идея, описанная в тред-запросе и ответах экспертов):
//!
//! 1. **Базовая плоскость** ([`BasePlane`]) задаёт общий наклон территории —
//!    от плоской равнины до склона.
//! 2. **Линии-деформации** ([`TerrainFeature`]): кривая хранит не высоту,
//!    а поперечный профиль относительно себя; высота в точке считается по
//!    (знаковому) расстоянию до кривой ([`Profile`]) или путём врезания
//!    минимумом по абсолютному дну (русло/каньон/чаша).
//! 3. **fBm-шум** ([`TerrainNoise`]) добавляет каменистость; у воды шум
//!    маскируется, чтобы берега были чистыми, но на уступах/стенках каньона
//!    шум работает в полную силу.
//! 4. **Облака точек**: грунт и вода сэмплируются по одной и той же
//!    горизонтальной сетке (шаг тот же на любых наклонах); под водой точек
//!    грунта нет.
//!
//! Модуль пригоден и для headless-приложений (`MinimalPlugins`), и для
//! визуализации (см. [`crate::visual`]).

pub mod config;
pub mod curve;
pub mod erosion;
pub mod features;
pub mod landscape;
pub mod noise;
pub mod preset;
pub mod profile;

pub use config::{BasePlane, LandscapeConfig, Longitudinal, TerrainNoise};
pub use curve::{warped_contour, winding_path, Curve, Nearest};
pub use features::{BowlFeature, ChannelFeature, StepFeature, TerrainFeature};
pub use landscape::{
    channel_falls_curtains, falls_clear_offset, FallsCurtain, Heightfield, Landscape, WaterField,
};
pub use noise::NoiseSampler;
pub use preset::{LandscapePreset, resolve};
pub use profile::Profile;
