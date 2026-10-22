//! Занимаемая объектом территория на общей карте (`terrains-generator`).
//!
//! Размещаемые объекты (здания, растения) имеют `Occupancy`: при расстановке
//! на территории вышестоящий генератор проверяет, не пересекается ли новая
//! зона с уже занятыми. Пока достаточно квадрата и круга; далее логика будет
//! расширяться (агрегированные зоны, вложенность и т.п.).

use bevy::math::{Vec2, Vec3};
use bevy::prelude::Component;
use serde::{Deserialize, Serialize};

/// Форма занимаемой территории в плоскости XY (вертикаль — Z).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum OccupancyShape {
    /// Квадрат (прямоугольник) с полуразмерами вдоль локальных осей.
    /// Поворачивается вместе с объектом вокруг `Occupancy::rotation`.
    Square { half_extent: Vec2 },
    /// Круг с центром в точке-центре объекта.
    Circle { radius: f32 },
}

impl OccupancyShape {
    /// Гарантированный радиус описанной окружности (для грубых проверок).
    pub fn bounding_radius(&self) -> f32 {
        match *self {
            Self::Square { half_extent } => half_extent.length(),
            Self::Circle { radius } => radius,
        }
    }
}

/// Занимаемая территория: положение на карте + форма + поворот.
///
/// Компонент Bevy — ставится на корневую сущность размещаемого объекта
/// (здания, растения), чтобы `terrains-generator` мог её учитывать.
#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Occupancy {
    /// Центр территории в плоскости карты (игнорируется вертикальная ось).
    pub center: Vec2,
    /// Поворот формы вокруг вертикальной оси, радианы.
    pub rotation: f32,
    /// Форма территории.
    pub shape: OccupancyShape,
}

impl Occupancy {
    /// Квадрат/прямоугольник с центром в `center`.
    pub fn square(center: Vec2, half_extent: Vec2, rotation: f32) -> Self {
        Self {
            center,
            rotation,
            shape: OccupancyShape::Square { half_extent },
        }
    }

    /// Круг с центром в `center`.
    pub fn circle(center: Vec2, radius: f32) -> Self {
        Self {
            center,
            rotation: 0.0,
            shape: OccupancyShape::Circle { radius },
        }
    }

    /// Центр как трёхмерная точка на высоте `z` (для удобства).
    pub fn center3(&self, z: f32) -> Vec3 {
        Vec3::new(self.center.x, self.center.y, z)
    }

    /// Проверка пересечения двух зон.
    ///
    /// Квадрат обрабатывается как OBB (separating axis theorem), поэтому
    /// повёрнутые зоны считаются корректно; круг — точная проверка дистанции.
    pub fn intersects(&self, other: &Self) -> bool {
        match (self.shape, other.shape) {
            (OccupancyShape::Circle { radius: r1 }, OccupancyShape::Circle { radius: r2 }) => {
                self.center.distance(other.center) < r1 + r2
            }
            (OccupancyShape::Circle { radius }, OccupancyShape::Square { .. }) => {
                other.square_intersects_circle(self, radius)
            }
            (OccupancyShape::Square { .. }, OccupancyShape::Circle { radius }) => {
                self.square_intersects_circle(other, radius)
            }
            (OccupancyShape::Square { .. }, OccupancyShape::Square { .. }) => {
                self.square_intersects_square(other)
            }
        }
    }

    /// Квадрат (self) против круга радиуса `radius` с центром `circle`.
    fn square_intersects_circle(&self, circle: &Self, radius: f32) -> bool {
        let half_extent: Vec2 = match self.shape {
            OccupancyShape::Square { half_extent } => half_extent,
            _ => unreachable!(),
        };
        // Приводим локальные оси квадрата к мировой системе и ищем ближайшую
        // точку прямоугольника к центру круга.
        let (sin, cos) = self.rotation.sin_cos();
        let axis_x = Vec2::new(cos, sin);
        let axis_y = Vec2::new(-sin, cos);

        let d = circle.center - self.center;
        // Проекция на локальные оси квадрата.
        let dx = d.dot(axis_x).clamp(-half_extent.x, half_extent.x);
        let dy = d.dot(axis_y).clamp(-half_extent.y, half_extent.y);
        let closest = self.center + axis_x * dx + axis_y * dy;
        closest.distance_squared(circle.center) < radius * radius
    }

    /// Два квадрата (OBB) — через separating axis theorem.
    fn square_intersects_square(&self, other: &Self) -> bool {
        let (ha, hb) = match (self.shape, other.shape) {
            (OccupancyShape::Square { half_extent: a }, OccupancyShape::Square { half_extent: b }) => {
                (a, b)
            }
            _ => unreachable!(),
        };
        let (sin_a, cos_a) = self.rotation.sin_cos();
        let (sin_b, cos_b) = other.rotation.sin_cos();
        let axes_a = [Vec2::new(cos_a, sin_a), Vec2::new(-sin_a, cos_a)];
        let axes_b = [Vec2::new(cos_b, sin_b), Vec2::new(-sin_b, cos_b)];

        let axes: [Vec2; 4] = [axes_a[0], axes_a[1], axes_b[0], axes_b[1]];

        for axis in axes {
            let proj_a = project_square(self.center, ha, axes_a, axis);
            let proj_b = project_square(other.center, hb, axes_b, axis);
            if proj_a.1 < proj_b.0 || proj_b.1 < proj_a.0 {
                return false;
            }
        }
        true
    }
}

/// Проекция квадрата (центр, полуразмеры, локальные оси) на ось.
fn project_square(center: Vec2, half: Vec2, local_axes: [Vec2; 2], axis: Vec2) -> (f32, f32) {
    let r = half.x * local_axes[0].dot(axis).abs() + half.y * local_axes[1].dot(axis).abs();
    let c = center.dot(axis);
    (c - r, c + r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circles_intersect() {
        let a = Occupancy::circle(Vec2::ZERO, 1.0);
        let b = Occupancy::circle(Vec2::new(1.5, 0.0), 1.0);
        assert!(a.intersects(&b));
        let c = Occupancy::circle(Vec2::new(3.0, 0.0), 1.0);
        assert!(!a.intersects(&c));
    }

    #[test]
    fn squares_overlap_and_separate() {
        let a = Occupancy::square(Vec2::ZERO, Vec2::splat(1.0), 0.0);
        // Перекрытие по X.
        let b = Occupancy::square(Vec2::new(1.5, 0.0), Vec2::splat(1.0), 0.0);
        assert!(a.intersects(&b));
        // Полное разделение.
        let c = Occupancy::square(Vec2::new(3.0, 0.0), Vec2::splat(1.0), 0.0);
        assert!(!a.intersects(&c));
    }
}
