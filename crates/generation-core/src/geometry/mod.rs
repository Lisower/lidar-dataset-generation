//! Полигональные (визуальные) модели: плоские выпуклые полигоны и тела
//! из них (параллелепипед, конус, N-гранник).
//!
//! Это общий «строительный материал» для всех генераторов: и визуальный меш,
//! и облако точек для поверхности строятся из одних и тех же полигонов, поэтому
//! визуальная модель и облако никогда не разъезжаются.

use bevy::math::{Quat, Vec3};

/// Плоский выпуклый полигон: треугольник (3 точки) или четырёхугольник.
///
/// Точки перечислены так, что нормаль [`Polygon::normal`] (правило правой руки)
/// смотрит наружу от тела / на зрителя.
#[derive(Debug, Clone, PartialEq)]
pub struct Polygon {
    points: Vec<Vec3>,
}

impl Polygon {
    /// Треугольник.
    pub fn triangle(a: Vec3, b: Vec3, c: Vec3) -> Self {
        Self {
            points: vec![a, b, c],
        }
    }

    /// Четырёхугольник.
    pub fn quad(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> Self {
        Self {
            points: vec![a, b, c, d],
        }
    }

    /// Полигон из произвольного набора планарных выпуклых точек
    /// (например, правильный N-гранник).
    pub fn from_points(points: impl Into<Vec<Vec3>>) -> Self {
        Self {
            points: points.into(),
        }
    }

    /// Точки полигона.
    pub fn points(&self) -> &[Vec3] {
        &self.points
    }

    /// Число вершин.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Полигон пуст.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Единичная нормаль (правило правой руки по первым трём точкам).
    pub fn normal(&self) -> Vec3 {
        debug_assert!(self.points.len() >= 3);
        let [a, b, c] = [self.points[0], self.points[1], self.points[2]];
        (b - a).cross(c - a).normalize_or_zero()
    }

    /// Ориентировать полигон так, чтобы нормаль смотрела в направление
    /// `preferred` (или хотя бы по ту же сторону от плоскости).
    ///
    /// Удобно при ручном построении граней: задаём углы в произвольном
    /// порядке, а затем «просим» нормаль наружу.
    pub fn oriented(mut self, preferred: Vec3) -> Self {
        if self.normal().dot(preferred) < 0.0 {
            self.points.reverse();
        }
        self
    }

    /// Разбить на треугольники («веер» от первой вершины) — допустимо для
    /// выпуклых планарных многоугольников, которых у нас все.
    pub fn triangulate(&self) -> impl Iterator<Item = Vec3> + '_ {
        (1..self.points.len() - 1).flat_map(move |i| {
            [
                self.points[0],
                self.points[i],
                self.points[i + 1],
            ]
        })
    }
}

/// Периметр полигона (может понадобиться генераторам для длины свесов).
pub fn perimeter(polygon: &Polygon) -> f32 {
    let pts = polygon.points();
    let n = pts.len();
    (0..n)
        .map(|i| pts[i].distance(pts[(i + 1) % n]))
        .sum()
}

/// Грани прямоугольного параллелепипеда (6 шт.), центр в начале координат.
pub fn box_polygons(size: Vec3) -> Vec<Polygon> {
    let (x, y, z) = (size.x * 0.5, size.y * 0.5, size.z * 0.5);
    let c = |sign: Vec3| Vec3::new(x * sign.x, y * sign.y, z * sign.z);

    // Шесть граней, каждая задана четырьмя углами; нормали наружу.
    let faces = [
        // +Z (верх)
        vec![c(Vec3::new(1.0, 1.0, 1.0)), c(Vec3::new(1.0, -1.0, 1.0)), c(Vec3::new(-1.0, -1.0, 1.0)), c(Vec3::new(-1.0, 1.0, 1.0))],
        // -Z (низ)
        vec![c(Vec3::new(1.0, 1.0, -1.0)), c(Vec3::new(-1.0, 1.0, -1.0)), c(Vec3::new(-1.0, -1.0, -1.0)), c(Vec3::new(1.0, -1.0, -1.0))],
        // +X
        vec![c(Vec3::new(1.0, 1.0, 1.0)), c(Vec3::new(1.0, 1.0, -1.0)), c(Vec3::new(1.0, -1.0, -1.0)), c(Vec3::new(1.0, -1.0, 1.0))],
        // -X
        vec![c(Vec3::new(-1.0, 1.0, 1.0)), c(Vec3::new(-1.0, -1.0, 1.0)), c(Vec3::new(-1.0, -1.0, -1.0)), c(Vec3::new(-1.0, 1.0, -1.0))],
        // +Y
        vec![c(Vec3::new(-1.0, 1.0, 1.0)), c(Vec3::new(1.0, 1.0, 1.0)), c(Vec3::new(1.0, 1.0, -1.0)), c(Vec3::new(-1.0, 1.0, -1.0))],
        // -Y
        vec![c(Vec3::new(1.0, -1.0, 1.0)), c(Vec3::new(-1.0, -1.0, 1.0)), c(Vec3::new(-1.0, -1.0, -1.0)), c(Vec3::new(1.0, -1.0, -1.0))],
    ];

    let pref = [
        Vec3::Z, -Vec3::Z, Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y,
    ];

    faces
        .iter()
        .zip(pref)
        .map(|(pts, n)| Polygon::from_points(pts.clone()).oriented(n))
        .collect()
}

/// Правильный N-гранник в плоскости `normal` вокруг точки-оригина.
///
/// Центр полигона — в начале координат, первая вершина лежит вдоль опорного
/// направления `up` (уже спроецированного в плоскость).
pub fn n_gon_polygon(sides: usize, radius: f32, normal: Vec3, up: Vec3) -> Polygon {
    debug_assert!(sides >= 3);
    let normal = normal.normalize();
    // Ортонормированный базис плоскости.
    let u = up.reject_from(normal).normalize();
    let v = normal.cross(u).normalize();
    let step = std::f32::consts::TAU / sides as f32;

    let pts: Vec<Vec3> = (0..sides)
        .map(|i| {
            let a = step * i as f32;
            u * (radius * a.cos()) + v * (radius * a.sin())
        })
        .collect();

    Polygon::from_points(pts).oriented(normal)
}

/// Грани боковой поверхности и оснований усечённого конуса.
///
/// Ориентация: ось вдоль `normal`, нижнее основание радиуса `radius_bottom`
/// лежит в `normal`-плоскости на расстоянии `-height/2`, верхнее радиуса
/// `radius_top` — на `+height/2`. Нормали направлены наружу.
pub fn cone_polygons(
    radius_bottom: f32,
    radius_top: f32,
    height: f32,
    sides: usize,
    normal: Vec3,
) -> Vec<Polygon> {
    let normal = normal.normalize();
    let u = Vec3::X.reject_from(normal).normalize();
    let v = normal.cross(u).normalize();

    let z0 = -height * 0.5;
    let z1 = height * 0.5;
    let step = std::f32::consts::TAU / sides as f32;

    let mut polys = Vec::new();

    // Боковая поверхность.
    for i in 0..sides {
        let a0 = step * i as f32;
        let a1 = step * (i + 1) as f32;
        let c0 = u * a0.cos() + v * a0.sin();
        let c1 = u * a1.cos() + v * a1.sin();

        let b0 = c0 * radius_bottom + normal * z0;
        let b1 = c1 * radius_bottom + normal * z0;
        let t0 = c0 * radius_top + normal * z1;
        let t1 = c1 * radius_top + normal * z1;

        let mid = (c0 + c1) * 0.5;
        polys.push(Polygon::quad(b0, b1, t1, t0).oriented(mid));
    }

    // Основания (веером от центра) — покрытие круга треугольниками.
    for (z, r, pref) in [
        (z0, radius_bottom, -normal),
        (z1, radius_top, normal),
    ] {
        if r <= 0.0 {
            continue;
        }
        let center = normal * z;
        for i in 0..sides {
            let a0 = step * i as f32;
            let a1 = step * (i + 1) as f32;
            let p0 = center + (u * (a0.cos() * r) + v * (a0.sin() * r));
            let p1 = center + (u * (a1.cos() * r) + v * (a1.sin() * r));
            polys.push(Polygon::triangle(center, p0, p1).oriented(pref));
        }
    }

    polys
}

/// Собрать `Transform` из двух векторов плоскости (должны быть ортогональны):
/// локальные оси `(u, v, u.cross(v))`, начало — в `origin`.
pub fn transform_from_axes(origin: Vec3, u: Vec3, v: Vec3) -> (bevy::math::Vec3, Quat) {
    let w = u.cross(v).normalize();
    let m = bevy::math::Mat3::from_cols(u.normalize(), v.normalize(), w);
    (origin, Quat::from_mat3(&m))
}
