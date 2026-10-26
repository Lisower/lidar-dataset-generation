//! Часть здания: узел иерархии (см. wiki «Создание объекта»).
//!
//! Часть имеет собственный `Transform` относительно родителя, геометрию
//! ([`Part::faces`] — для визуального меша) и облако точек
//! ([`Part::cloud`] — точки обычно на внешних поверхностях). У части могут
//! быть дочерние части (этаж → стена → окно), тогда их облака собираются
//! методом [`Part::collect_cloud`].

use bevy::color::Color;
use bevy::math::{Mat4, Vec3};
use bevy::transform::components::Transform;
use generation_core::geometry::{Polygon, box_polygons};
use generation_core::point_clouds::primitives::{GridNoise, apply_noise, polygon_points};
use generation_core::point_clouds::{PointClass, PointCloud};

use super::config::BuildingConfig;

/// Часть здания (узел дерева).
#[derive(Debug, Clone)]
pub struct Part {
    /// Имя части (используется для логов и имени сущности).
    pub name: &'static str,
    /// Цвет материала (унаследованный от родителя можно задать явно).
    pub color: Color,
    /// Трансформ относительно родителя.
    pub transform: Transform,
    /// Визуальная геометрия (полигоны, нормали наружу).
    pub faces: Vec<Polygon>,
    /// Рисовать ли геометрию с двух сторон (для открытых поверхностей,
    /// например крыш — у них «изнанка» скатов не закрыта).
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
            cloud: PointCloud::new(PointClass::Building),
            children: Vec::new(),
        }
    }

    /// Параллелепипед с облаком только на одной «внешней» грани.
    pub fn boxed(
        name: &'static str,
        size: Vec3,
        outward: Vec3,
        color: Color,
        transform: Transform,
        config: &BuildingConfig,
    ) -> Self {
        Self::boxed_step(
            name, size, outward, color, transform, config, config.point_step,
        )
    }

    /// То же, что [`Part::boxed`], но с явным шагом сетки облака точек.
    pub fn boxed_step(
        name: &'static str,
        size: Vec3,
        outward: Vec3,
        color: Color,
        transform: Transform,
        config: &BuildingConfig,
        step: f32,
    ) -> Self {
        let cloud_face = outer_face_polygon(size, outward);
        Self::build_with_step(
            name,
            color,
            transform,
            box_polygons(size),
            vec![cloud_face],
            false,
            config,
            step,
        )
    }

    /// Параллелепипед с облаком на нескольких заданных гранях.
    pub fn boxed_outwards(
        name: &'static str,
        size: Vec3,
        outwards: &[Vec3],
        color: Color,
        transform: Transform,
        config: &BuildingConfig,
    ) -> Self {
        Self::boxed_outwards_step(
            name,
            size,
            outwards,
            color,
            transform,
            config,
            config.point_step,
        )
    }

    /// То же, что [`Part::boxed_outwards`], но с явным шагом сетки облака.
    pub fn boxed_outwards_step(
        name: &'static str,
        size: Vec3,
        outwards: &[Vec3],
        color: Color,
        transform: Transform,
        config: &BuildingConfig,
        step: f32,
    ) -> Self {
        let cloud_faces: Vec<Polygon> =
            outwards.iter().map(|o| outer_face_polygon(size, *o)).collect();
        Self::build_with_step(
            name,
            color,
            transform,
            box_polygons(size),
            cloud_faces,
            false,
            config,
            step,
        )
    }

    /// Часть с произвольным набором граней; облако — по тем же граням.
    ///
    /// Удобно для крыш: визуальная модель и облако строятся из одних и тех
    /// же полигонов, поэтому никогда не «разъезжаются».
    pub fn from_faces(
        name: &'static str,
        faces: Vec<Polygon>,
        color: Color,
        transform: Transform,
        double_sided: bool,
        config: &BuildingConfig,
    ) -> Self {
        Self::build(
            name,
            color,
            transform,
            faces.clone(),
            faces,
            double_sided,
            config,
        )
    }

    /// Параллелепипед с заданными (уже готовыми) точками облака — например,
    /// для стены с вырезанными проёмами окон/дверей/балконов.
    ///
    /// Шум на точки накладывает вызывающий код (чтобы проёмы «резались» до
    /// шума), поэтому здесь он не применяется.
    ///
    /// `config` сохраняется в сигнатуре для единообразия остальных
    /// конструкторов частей.
    #[allow(unused_variables)]
    pub fn boxed_cloud_positions(
        name: &'static str,
        size: Vec3,
        color: Color,
        transform: Transform,
        positions: Vec<Vec3>,
        config: &BuildingConfig,
    ) -> Self {
        let mut cloud = PointCloud::new(PointClass::Building);
        cloud.extend_with_class(positions, PointClass::Building);
        Self {
            name,
            color,
            transform,
            faces: box_polygons(size),
            double_sided: false,
            cloud,
            children: Vec::new(),
        }
    }

    /// Добавить дочернюю часть (для построения цепочками).
    pub fn child(mut self, part: Part) -> Self {
        self.children.push(part);
        self
    }

    /// Добавить несколько дочерних частей.
    pub fn children(mut self, parts: Vec<Part>) -> Self {
        self.children.extend(parts);
        self
    }

    /// Добавить дочернюю часть (Builder-вариант, цепочка после `group`).
    pub fn with_child(mut self, part: Part) -> Self {
        self.children.push(part);
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

    /// Создать часть с облаком из `cloud_faces` (шаг — из конфигурации).
    fn build(
        name: &'static str,
        color: Color,
        transform: Transform,
        faces: Vec<Polygon>,
        cloud_faces: Vec<Polygon>,
        double_sided: bool,
        config: &BuildingConfig,
    ) -> Self {
        Self::build_with_step(
            name,
            color,
            transform,
            faces,
            cloud_faces,
            double_sided,
            config,
            config.point_step,
        )
    }

    /// Создать часть и посчитать её облако из `cloud_faces` сеткой с явным
    /// шагом и лёгким детерминированным шумом.
    fn build_with_step(
        name: &'static str,
        color: Color,
        transform: Transform,
        faces: Vec<Polygon>,
        cloud_faces: Vec<Polygon>,
        double_sided: bool,
        config: &BuildingConfig,
        step: f32,
    ) -> Self {
        let mut positions: Vec<Vec3> = Vec::new();
        for poly in &cloud_faces {
            positions.extend(polygon_points(poly, step));
        }
        // Шум — по метке имени части, чтобы сетка не «светилась» правильной
        // решёткой, оставаясь детерминированной от seed.
        apply_noise(
            &mut positions,
            GridNoise {
                amplitude: config.point_jitter,
            },
            config.seed.with_label(name),
        );

        let mut cloud = PointCloud::new(PointClass::Building);
        cloud.extend_with_class(positions, PointClass::Building);

        Self {
            name,
            color,
            transform,
            faces,
            double_sided,
            cloud,
            children: Vec::new(),
        }
    }
}

/// Полигон внешней грани параллелепипеда (для облака точек).
pub(crate) fn outer_face_polygon(size: Vec3, outward: Vec3) -> Polygon {
    let outward = outward.normalize();
    // Ось «вдоль» перпендикулярна outward (выбираем оси X или Y).
    let along = if outward.x.abs() < 0.9 {
        Vec3::X
    } else {
        Vec3::Y
    };
    let up = outward.cross(along).normalize();
    // Размер проекции положителен; знак задаёт сам вектор направления.
    // Иначе для -X/-Y точки оказываются на противоположной, внутренней грани.
    let h_along = along.abs().dot(size);
    let h_up = up.abs().dot(size);
    let h_out = outward.abs().dot(size);

    let center = outward * (h_out * 0.5);
    let c1 = center + along * (h_along * 0.5) + up * (h_up * 0.5);
    let c2 = center - along * (h_along * 0.5) + up * (h_up * 0.5);
    let c3 = center - along * (h_along * 0.5) - up * (h_up * 0.5);
    let c4 = center + along * (h_along * 0.5) - up * (h_up * 0.5);
    Polygon::quad(c1, c2, c3, c4).oriented(outward)
}
