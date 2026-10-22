//! Облако точек: основные типы данных.

use bevy::math::{Mat4, Vec3};
use bevy::prelude::Component;
use serde::{Deserialize, Serialize};

use super::classes::PointClass;

/// Точка облака: положение в пространстве + класс.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    /// Положение в локальной (или мировой) системе координат.
    pub position: Vec3,
    /// Класс точки.
    pub class: PointClass,
}

impl Point {
    /// Новая точка.
    pub fn new(position: Vec3, class: PointClass) -> Self {
        Self { position, class }
    }
}

impl From<Vec3> for Point {
    fn from(position: Vec3) -> Self {
        Self {
            position,
            class: PointClass::default(),
        }
    }
}

/// Облако точек, пригодное и для хранения данных, и как компонент Bevy
/// на сущности облака точек.
///
/// Координаты точек держатся в локальной системе координат родительской
/// части; мировая позиция получается наследованием `Transform`'а (см. wiki
/// «Создание объекта»).
#[derive(Component, Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PointCloud {
    /// Точки облака.
    pub points: Vec<Point>,
    /// Класс по умолчанию (для `push` без явного класса).
    pub default_class: PointClass,
}

impl PointCloud {
    /// Пустое облако с заданным классом по умолчанию.
    pub fn new(default_class: PointClass) -> Self {
        Self {
            points: Vec::new(),
            default_class,
        }
    }

    /// Точка добавлена с классом по умолчанию.
    pub fn push(&mut self, position: Vec3) {
        self.points.push(Point::new(position, self.default_class));
    }

    /// Добавить всё облако (классы сохраняются).
    pub fn merge(&mut self, other: &PointCloud) {
        self.points.extend_from_slice(&other.points);
    }

    /// Добавить произвольные точки с фиксированным классом.
    pub fn extend_with_class(
        &mut self,
        positions: impl IntoIterator<Item = Vec3>,
        class: PointClass,
    ) {
        self.points
            .extend(positions.into_iter().map(|p| Point::new(p, class)));
    }

    /// Позиции всех точек.
    pub fn positions(&self) -> impl Iterator<Item = Vec3> + '_ {
        self.points.iter().map(|p| p.position)
    }

    /// Количество точек.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Облако пусто.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Трансформировать все точки матрицей (например, для перевода в мировые
    /// координаты при headless-расстановке).
    pub fn transform_by(&mut self, mat: Mat4) {
        for p in &mut self.points {
            p.position = mat.transform_point3(p.position);
        }
    }

    /// Сбросить облако.
    pub fn clear(&mut self) {
        self.points.clear();
    }
}
