//! Часть растения: узел иерархии (см. wiki «Создание объекта»).
//!
//! Следует тому же подходу, что в `buildings-generator`: у части есть имя,
//! цвет, трансформ относительно родителя, визуальная геометрия ([`Part::faces`])
//! и облако точек ([`Part::cloud`]), а также дочерние части. У растения два
//! содержательных уровня: группа «растение» → «ветки» и «листва» (у травы —
//! один «пучок»). Облако собирается рекурсивно [`Part::collect_cloud`].

use bevy::color::Color;
use bevy::math::{Mat4, Vec3};
use bevy::transform::components::Transform;
use generation_core::geometry::Polygon;
use generation_core::point_clouds::{PointClass, PointCloud};

/// Часть растения (узел дерева).
#[derive(Debug, Clone)]
pub struct Part {
    /// Имя части (используется для логов и имени сущности).
    pub name: &'static str,
    /// Цвет материала.
    pub color: Color,
    /// Трансформ относительно родителя.
    pub transform: Transform,
    /// Визуальная геометрия (полигоны, нормали наружу).
    pub faces: Vec<Polygon>,
    /// Рисовать ли геометрию с двух сторон (листья, трава).
    pub double_sided: bool,
    /// Облако точек части (в локальной системе координат части).
    pub cloud: PointCloud,
    /// Дочерние части.
    pub children: Vec<Part>,
}

impl Part {
    /// Контейнер без геометрии и облака (для группировки дочерних частей).
    pub fn group(name: &'static str, color: Color, transform: Transform) -> Self {
        Self {
            name,
            color,
            transform,
            faces: Vec::new(),
            double_sided: false,
            cloud: PointCloud::new(PointClass::Vegetation),
            children: Vec::new(),
        }
    }

    /// Часть, полностью заданная вручную (геометрия + облако).
    pub fn new(
        name: &'static str,
        color: Color,
        faces: Vec<Polygon>,
        double_sided: bool,
        cloud: Vec<Vec3>,
    ) -> Self {
        let mut point_cloud = PointCloud::new(PointClass::Vegetation);
        point_cloud.extend_with_class(cloud, PointClass::Vegetation);
        Self {
            name,
            color,
            transform: Transform::IDENTITY,
            faces,
            double_sided,
            cloud: point_cloud,
            children: Vec::new(),
        }
    }

    /// Добавить дочернюю часть.
    pub fn child(mut self, part: Part) -> Self {
        self.children.push(part);
        self
    }

    /// Добавить несколько дочерних частей.
    pub fn children(mut self, parts: Vec<Part>) -> Self {
        self.children.extend(parts);
        self
    }

    /// Облако точек этой части и всех потомков в её локальной системе
    /// координат (точки потомков переводятся их трансформами).
    pub fn collect_cloud(&self) -> PointCloud {
        let mut out = self.cloud.clone();
        for child in &self.children {
            let mut child_cloud = child.collect_cloud();
            child_cloud.transform_by(Mat4::from_rotation_translation(
                child.transform.rotation,
                child.transform.translation,
            ));
            out.merge(&child_cloud);
        }
        out
    }
}
