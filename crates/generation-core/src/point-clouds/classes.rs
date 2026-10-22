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
    Noise,
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
            Self::Noise => "noise",
            Self::Other => "other",
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
            Self::Noise => Color::srgb(0.60, 0.10, 0.10),
            Self::Other => Color::srgb(0.55, 0.55, 0.55),
        }
    }
}
