//! Генерация облаков точек для поверхностей примитивов.
//!
//! Прямоугольник, треугольник и конус «выдаются» по шагу сетки — так же, как
//! в прототипе: число точек по стороне — округлённое деление размера на шаг.
//! Поверхности могут быть ориентированы в пространстве произвольно.

use bevy::math::{Mat4, Quat, Vec3};
use rand::{rngs::StdRng, RngExt};

use crate::seeds::Seed;
use crate::geometry::Polygon;

/// Шаг сетки, меньше которого считаем поверхность вырожденной (нет точек).
const EPS: f32 = 1e-4;

/// Нарезать прямоугольник (в плоскости XY, центр в начале координат)
/// сеткой с шагом `step`. Возвращает точки в порядке сетки.
pub fn rectangle_points(length: f32, width: f32, step: f32) -> Vec<Vec3> {
    if length <= EPS || width <= EPS || step <= EPS {
        return Vec::new();
    }
    let cols = (length / step).round().max(1.0) as usize;
    let rows = (width / step).round().max(1.0) as usize;

    let mut out = Vec::with_capacity((cols + 1) * (rows + 1));
    for j in 0..=rows {
        let y = -width * 0.5 + width * (j as f32) / rows as f32;
        for i in 0..=cols {
            let x = -length * 0.5 + length * (i as f32) / cols as f32;
            out.push(Vec3::new(x, y, 0.0));
        }
    }
    out
}

/// Нарезать треугольник (в плоскости своих трёх точек) сеткой с шагом `step`.
/// Покрываются и грани, и внутренность треугольника.
pub fn triangle_points(a: Vec3, b: Vec3, c: Vec3, step: f32) -> Vec<Vec3> {
    if step <= EPS {
        return Vec::new();
    }
    let n = (a.distance(b).max(a.distance(c)) / step).round().max(1.0) as usize;
    let e1 = (b - a) / n as f32;
    let e2 = (c - a) / n as f32;

    let mut out = Vec::new();
    for t in 0..=n {
        for s in 0..=(n - t) {
            out.push(a + e1 * t as f32 + e2 * s as f32);
        }
    }
    out
}

/// Равномерная сетка по (деформированному) прямоугольнику с углами a,b,c,d.
///
/// Сетка билинейная по двум сторонам из `a` (к `b` и к `d`), точки — в
/// центрах ячеек. В отличие от разбиения на треугольники здесь:
///
/// - нет «двойной» диагонали (которую давали бы два треугольника) — значит,
///   нет более плотных линий по диагонали или по коньку крыши;
/// - шаг равномерный по обеим осям — вертикальная и горизонтальная плотность
///   одинаковы;
/// - рёбра многоугольников не «дублируются» соседними гранями.
fn quad_points(a: Vec3, b: Vec3, c: Vec3, d: Vec3, step: f32) -> Vec<Vec3> {
    if step <= EPS {
        return Vec::new();
    }
    let len = a.distance(b).max(c.distance(d));
    let wid = a.distance(d).max(b.distance(c));
    if len <= EPS || wid <= EPS {
        return Vec::new();
    }
    let cols = (len / step).round().max(1.0) as usize;
    let rows = (wid / step).round().max(1.0) as usize;
    let mut out = Vec::with_capacity(cols.saturating_mul(rows));
    for j in 0..rows {
        let v = (j as f32 + 0.5) / rows as f32;
        for i in 0..cols {
            let u = (i as f32 + 0.5) / cols as f32;
            out.push(a.lerp(b, u).lerp(d.lerp(c, u), v));
        }
    }
    out
}

/// Нарезать плоский выпуклый полигон: треугольник — напрямую,
/// четырёхугольник — равномерной сеткой, N-гранник — «веером».
pub fn polygon_points(polygon: &Polygon, step: f32) -> Vec<Vec3> {
    let pts = polygon.points();
    match pts.len() {
        0 | 1 | 2 => Vec::new(),
        3 => triangle_points(pts[0], pts[1], pts[2], step),
        4 => quad_points(pts[0], pts[1], pts[2], pts[3], step),
        n => {
            // Веер треугольников из первой вершины.
            let mut out = Vec::new();
            for i in 1..n - 1 {
                out.extend(triangle_points(pts[0], pts[i], pts[i + 1], step));
            }
            out
        }
    }
}

/// Нарезать боковую поверхность и основания усечённого конуса.
///
/// Ось вдоль Z, нижнее основание в `z = 0`, верхнее в `z = height`.
/// Возвращает точки боковой поверхности и (если `caps`) — основания.
pub fn cone_points(
    radius_bottom: f32,
    radius_top: f32,
    height: f32,
    step: f32,
    caps: bool,
) -> Vec<Vec3> {
    if step <= EPS {
        return Vec::new();
    }
    let mut out = Vec::new();

    // Боковая поверхность: кольца по высоте, точки по окружности.
    let rings = (height / step).round().max(1.0) as usize;
    for k in 0..=rings {
        let t = k as f32 / rings as f32;
        let r = radius_top * t + radius_bottom * (1.0 - t);
        if r <= EPS {
            continue;
        }
        let z = height * t;
        // Длина окружности определяет число сегментов (примерно шаг).
        let segs = ((std::f32::consts::TAU * r) / step).round().max(6.0) as usize;
        for i in 0..segs {
            let a = std::f32::consts::TAU * i as f32 / segs as f32;
            out.push(Vec3::new(a.cos() * r, a.sin() * r, z));
        }
    }

    if caps {
        for (r, z) in [(radius_bottom, 0.0), (radius_top, height)] {
            if r <= EPS {
                continue;
            }
            // Концентрические кольца снизу вверх по радиусу.
            let rings = (r / step).round().max(1.0) as usize;
            for k in 0..=rings {
                let rr = r * k as f32 / rings as f32;
                let segs = ((std::f32::consts::TAU * rr) / step)
                    .round()
                    .max(if k == 0 { 1.0 } else { 8.0 }) as usize;
                if k == 0 {
                    out.push(Vec3::ZERO.with_z(z));
                } else {
                    for i in 0..segs {
                        let a = std::f32::consts::TAU * i as f32 / segs as f32;
                        out.push(Vec3::new(a.cos() * rr, a.sin() * rr, z));
                    }
                }
            }
        }
    }

    out
}

/// Опции «шума» сетки облака точек.
#[derive(Debug, Clone, Copy, Default)]
pub struct GridNoise {
    /// Максимальная амплитуда смещения точки, м. 0 — без смещения.
    pub amplitude: f32,
}

/// Сместить точки небольшим детерминированным шумом, чтобы сетка не
/// «светилась» правильной решёткой (см. `point-clouds/description.txt`).
pub fn apply_noise(points: &mut [Vec3], noise: GridNoise, seed: Seed) {
    if noise.amplitude <= EPS || points.is_empty() {
        return;
    }
    let mut rng: StdRng = seed.rng();
    for p in points.iter_mut() {
        let dx = rng.random_range(-noise.amplitude..noise.amplitude);
        let dy = rng.random_range(-noise.amplitude..noise.amplitude);
        let dz = rng.random_range(-noise.amplitude..noise.amplitude);
        *p += Vec3::new(dx, dy, dz);
    }
}

/// Применить геометрическое преобразование (сдвиг + поворот) к точкам.
pub fn oriented(points: &[Vec3], translation: Vec3, rotation: Quat) -> Vec<Vec3> {
    let mat = Mat4::from_rotation_translation(rotation, translation);
    points.iter().map(|p| mat.transform_point3(*p)).collect()
}

/// Вариант для использования внутри генераторов: сетка + шум одним махом.
pub fn sample_rectangle(
    length: f32,
    width: f32,
    step: f32,
    translation: Vec3,
    rotation: Quat,
    noise: GridNoise,
    seed: Seed,
) -> Vec<Vec3> {
    let mut pts = oriented(&rectangle_points(length, width, step), translation, rotation);
    apply_noise(&mut pts, noise, seed);
    pts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangle_counts() {
        // 4x2 метра шагом 1 → 5x3 = 15 точек.
        let pts = rectangle_points(4.0, 2.0, 1.0);
        assert_eq!(pts.len(), 15);
    }

    #[test]
    fn degenerate_rectangle_is_empty() {
        assert!(rectangle_points(0.0, 2.0, 1.0).is_empty());
        assert!(rectangle_points(4.0, 0.0, 1.0).is_empty());
    }

    #[test]
    fn triangle_contains_vertices() {
        let pts = triangle_points(Vec3::ZERO, Vec3::X, Vec3::Y, 1.0);
        assert!(pts.contains(&Vec3::ZERO));
        assert!(pts.contains(&Vec3::X));
        assert!(pts.contains(&Vec3::Y));
        // Точка (X + Y) вне треугольника быть не должна.
        let xy = Vec3::new(1.0, 1.0, 0.0);
        assert!(!pts.contains(&xy));
    }
}
