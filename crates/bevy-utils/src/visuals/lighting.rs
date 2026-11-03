//! Настройки освещения, общие для всех генераторов.
//!
//! Сцены проекта — стометровые карты с высокими деревьями и зданиями, поэтому
//! стандартной дальности теней Bevy (150 м) мало: тени должны оставаться
//! видимыми примерно в пять раз дальше.

use bevy::light::{CascadeShadowConfig, CascadeShadowConfigBuilder};

/// Максимальная дистанция каскадных теней солнца, м.
///
/// Стандартное значение Bevy — 150 м; для сцен проекта увеличено в пять раз.
pub const SUN_SHADOW_DISTANCE: f32 = 750.0;

/// Конфигурация каскадных теней солнечного света.
///
/// Добавляется к сущности [`DirectionalLight`](bevy::light::DirectionalLight)
/// вместо стандартной, чтобы тени не пропадали на дальних объектах.
pub fn sun_shadows() -> CascadeShadowConfig {
    CascadeShadowConfigBuilder {
        maximum_distance: SUN_SHADOW_DISTANCE,
        ..Default::default()
    }
    .into()
}
