//! Классы точек и их визуальные цвета.
//!
//! Классификация близка к стандарту LiDAR (земля, здания, растительность,
//! вода), плюс отдельный «шумовой» класс для ложно отражённых точек.

use bevy::color::Color;
use serde::{Deserialize, Serialize};

/// Класс точки облака.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum PointClass {
    /// Земля, грунт.
    Ground,
    /// Здания и их части.
    #[default]
    Building,
    /// Растительность (деревья, кусты, трава).
    Vegetation,
    /// Вода.
    Water,
    /// Ложно отражённые / шумовые точки.
    LowNoise,
    /// Прочее (не задействовано в конкретной генерации).
    Other,
}

impl PointClass {
    /// Короткое название класса.
    pub fn label(self) -> &'static str {
        match self {
            Self::Ground => "ground",
            Self::Building => "building",
            Self::Vegetation => "vegetation",
            Self::Water => "water",
            Self::LowNoise => "low noise",
            Self::Other => "other",
        }
    }

    /// Числовой код класса для текстовых форматов экспорта (`.xyz`, `.ply`).
    ///
    /// Нумерация совпадает с той, что исторически используется при экспорте
    /// PLY в `lidar-simulator` (LowNoise = 7 и т.п.).
    pub fn code(self) -> u8 {
        match self {
            Self::Ground => 2,
            Self::Vegetation => 5,
            Self::Building => 6,
            Self::LowNoise => 7,
            Self::Water => 9,
            Self::Other => 1,
        }
    }

    /// Базовый размер точки класса в пикселях (на опорной дистанции).
    ///
    /// Шейдер облака точек масштабирует его по расстоянию до камеры.
    /// «Шумовые» точки заметно крупнее остальных, чтобы их было видно
    /// даже в разреженном облаке.
    pub fn point_size(self) -> f32 {
        match self {
            Self::Ground => 3.5,
            Self::Building => 3.5,
            Self::Vegetation => 3.0,
            Self::Water => 3.5,
            Self::LowNoise => 10.0,
            Self::Other => 3.5,
        }
    }

    /// Цвет, которым рисуется класс (и для визуализации облака точек,
    /// и для легенды интерфейса).
    pub fn color(self) -> Color {
        match self {
            Self::Ground => Color::srgb(0.45, 0.36, 0.25),
            Self::Building => Color::srgb(0.85, 0.12, 0.10),
            Self::Vegetation => Color::srgb(0.20, 0.60, 0.20),
            Self::Water => Color::srgb(0.15, 0.35, 0.75),
            Self::LowNoise => Color::srgb(0.85, 0.15, 0.93),
            Self::Other => Color::srgb(0.55, 0.55, 0.55),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LowNoise должен быть заметно крупнее остальных классов: примерно
    /// в 2–3 раза, чтобы шумовые точки не терялись в плотном облаке.
    #[test]
    fn low_noise_is_two_to_three_times_larger() {
        let low_noise = PointClass::LowNoise.point_size();
        for class in [
            PointClass::Ground,
            PointClass::Building,
            PointClass::Vegetation,
            PointClass::Water,
            PointClass::Other,
        ] {
            let size = class.point_size();
            assert!(size > 0.0, "{class:?} должен иметь положительный размер");
            assert!(
                low_noise >= 2.0 * size && low_noise <= 3.0 * size,
                "LowNoise ({low_noise}) должен быть в 2–3 раза больше {class:?} ({size})"
            );
        }
    }

    #[test]
    fn different_classes_have_different_sizes() {
        let sizes = [
            PointClass::Ground.point_size(),
            PointClass::Building.point_size(),
            PointClass::Vegetation.point_size(),
            PointClass::Water.point_size(),
            PointClass::LowNoise.point_size(),
        ];
        for i in 0..sizes.len() {
            for j in (i + 1)..sizes.len() {
                assert_ne!(sizes[i], sizes[j], "размеры классов должны различаться");
            }
        }
    }
}
