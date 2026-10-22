//! Режимы отображения сгенерированного объекта.
//!
//! Пока их два — визуальные модели и облако точек, но их может стать больше
//! (например, сетки). `V` переключает режим.

use bevy::prelude::*;

/// Режим отображения объекта.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    /// Полигональные (визуальные) модели.
    #[default]
    Visual,
    /// Облако точек.
    PointCloud,
}

impl ViewMode {
    /// Противоположный режим.
    pub fn toggled(self) -> Self {
        match self {
            Self::Visual => Self::PointCloud,
            Self::PointCloud => Self::Visual,
        }
    }

    /// Человекочитаемое имя режима.
    pub fn label(self) -> &'static str {
        match self {
            Self::Visual => "Визуальные модели",
            Self::PointCloud => "Облако точек",
        }
    }
}

/// Броадкаст о смене режима: визуализации объектов могут скрывать/показывать
/// свои сущности (обычно они и так читают ресурс [`ViewMode`]).
#[derive(Message, Debug, Clone, Copy)]
pub struct ViewModeToggle;
