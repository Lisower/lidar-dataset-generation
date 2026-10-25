//! Полилинии: рабочие линии деформации рельефа.
//!
//! Линия — ломаная (открытая или замкнутая) в плоскости карты XY, высота
//! наружу. Для карты 100×100 м все оси русел, берега, уступы и контуры чаш
//! моделируются такими ломаными. «Расшумливание» делается по рекомендациям
//! из треда: кривая строится уже «извилистой» (смещение по нормали шумом
//! fBm), поэтому берега и русла получаются естественными, а прямые линии
//! (террасы, уступы) остаются чистыми.

use std::f32::consts::TAU;

use bevy::math::Vec2;
use generation_core::Seed;
use noise::{Fbm, MultiFractal, NoiseFn, Perlin};
use rand::RngExt;

/// Результат поиска ближайшей точки на кривой.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Nearest {
    /// Беззнаковое расстояние до кривой, м.
    pub distance: f32,
    /// Положение ближайшей точки вдоль кривой, `0..=1` (по длине дуги).
    pub t: f32,
    /// Индекс сегмента, на котором лежит ближайшая точка.
    pub segment: usize,
}

/// Полилиния: открытая (уступ, берег, русло) или замкнутая (контур чаши).
#[derive(Debug, Clone, PartialEq)]
pub struct Curve {
    /// Вершины ломаной (для открытой — минимум 2).
    pub points: Vec<Vec2>,
    /// Замкнутая кривая (последняя точка соединена с первой).
    pub closed: bool,
    /// Полная длина дуги, м.
    pub length: f32,
}

impl Curve {
    /// Открытая ломаная.
    pub fn new_open(points: Vec<Vec2>) -> Self {
        let mut c = Self {
            points,
            closed: false,
            length: 0.0,
        };
        c.update_length();
        c
    }

    /// Замкнутая ломаная.
    pub fn new_closed(points: Vec<Vec2>) -> Self {
        let mut c = Self {
            points,
            closed: true,
            length: 0.0,
        };
        c.update_length();
        c
    }

    fn update_length(&mut self) {
        self.length = self.sum_arc();
    }

    /// Сумма длин всех сегментов (для замкнутой — включая замыкающий).
    fn sum_arc(&self) -> f32 {
        let mut arc = 0.0;
        for i in 0..self.segments() {
            let (a, b) = self.segment(i);
            arc += a.distance(b);
        }
        arc
    }

    /// Число сегментов.
    pub fn segments(&self) -> usize {
        if self.closed {
            self.points.len()
        } else {
            self.points.len() - 1
        }
    }

    /// Вершины `i`-го сегмента (для замкнутой `i` заворачивается к первой).
    pub fn segment(&self, i: usize) -> (Vec2, Vec2) {
        let n = self.points.len();
        let a = self.points[i % n];
        let b = self.points[(i + 1) % n];
        (a, b)
    }

    /// Ближайшая к `p` точка: беззнаковое расстояние, параметр вдоль дуги.
    pub fn nearest(&self, p: Vec2) -> Nearest {
        let mut best = Nearest {
            distance: f32::INFINITY,
            t: 0.0,
            segment: 0,
        };
        let mut arc_before = 0.0;
        for i in 0..self.segments() {
            let (a, b) = self.segment(i);
            let ab = b - a;
            let len2 = ab.length_squared();
            let t_seg = if len2 > 1e-12 {
                ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let q = a + ab * t_seg;
            let dist = p.distance(q);
            if dist < best.distance {
                best.distance = dist;
                best.segment = i;
                if self.length > 1e-6 {
                    best.t = (arc_before + ab.length() * t_seg) / self.length;
                }
            }
            arc_before += ab.length();
        }
        best
    }

    /// Знаковое расстояние до открытой кривой: сторона определяется
    /// кросс-произведением относительно направления ближайшего сегмента.
    pub fn signed_distance(&self, p: Vec2) -> f32 {
        let nb = self.nearest(p);
        let (a, b) = self.segment(nb.segment);
        let cross = (p.x - a.x) * (b.y - a.y) - (p.y - a.y) * (b.x - a.x);
        if cross >= 0.0 {
            nb.distance
        } else {
            -nb.distance
        }
    }

    /// Точка на кривой по параметру дуги `t ∈ [0,1]` (линейно по сегментам).
    pub fn point_at(&self, t: f32) -> Vec2 {
        let t = t.clamp(0.0, 1.0);
        if self.points.is_empty() {
            return Vec2::ZERO;
        }
        if self.length <= 1e-6 {
            return self.points[0];
        }
        let target = t * self.length;
        let mut arc = 0.0;
        for i in 0..self.segments() {
            let (a, b) = self.segment(i);
            let len = a.distance(b);
            if arc + len >= target - 1e-6 {
                let f = if len > 1e-6 { ((target - arc) / len).clamp(0.0, 1.0) } else { 0.0 };
                return a + (b - a) * f;
            }
            arc += len;
        }
        *self.points.last().unwrap_or(&Vec2::ZERO)
    }

    /// Единичный касательный вектор в точке [`Self::point_at`] по параметру `t`.
    pub fn tangent_at(&self, t: f32) -> Vec2 {
        let t = t.clamp(0.0, 1.0);
        if self.points.is_empty() {
            return Vec2::X;
        }
        if self.length <= 1e-6 {
            return Vec2::X;
        }
        let target = t * self.length;
        let mut arc = 0.0;
        for i in 0..self.segments() {
            let (a, b) = self.segment(i);
            let len = a.distance(b);
            let dir = b - a;
            if (arc + len >= target - 1e-6 && len > 1e-6) || i == self.segments() - 1 {
                let n = dir.normalize_or_zero();
                return if n.length_squared() > 1e-8 { n } else { Vec2::X };
            }
            arc += len;
        }
        Vec2::X
    }

    /// Знаковое расстояние до замкнутой кривой: внутри — отрицательное.
    ///
    /// Точность inside/outside — алгоритмом пересечения луча (even-odd),
    /// расстояние — до граничных сегментов.
    pub fn signed_distance_closed(&self, p: Vec2) -> f32 {
        debug_assert!(self.closed);
        let n = self.points.len();
        let mut dist = f32::INFINITY;
        let mut inside = false;
        for i in 0..n {
            let a = self.points[i];
            let b = self.points[(i + 1) % n];
            let ab = b - a;
            let len2 = ab.length_squared();
            let t = if len2 > 1e-12 {
                ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let q = a + ab * t;
            dist = dist.min(p.distance(q));

            let dy = b.y - a.y;
            if dy.abs() > 1e-9 && (a.y > p.y) != (b.y > p.y) {
                let xing = a.x + (p.y - a.y) * (b.x - a.x) / dy;
                if xing > p.x {
                    inside = !inside;
                }
            }
        }
        if inside {
            -dist
        } else {
            dist
        }
    }
}

/// Прямая (нерасшумленная) полилиния.
pub fn straight(from: Vec2, to: Vec2) -> Curve {
    Curve::new_open(vec![from, to])
}

/// Извилистая открытая кривая из `from` в `to`.
///
/// «Расшумливание» по нормали двумя октавными fBm — крупные изгибы задаёт
/// первая гармоника, мелкие неровности — вторая. `amplitude` — размах смещения
/// в метрах, `waves` — число «волн» изгибов на всю длину пути.
pub fn warped_line(
    from: Vec2,
    to: Vec2,
    amplitude: f32,
    waves: f32,
    samples: usize,
    seed: Seed,
) -> Curve {
    let dir = to - from;
    let len = dir.length();
    let unit = if len > 1e-6 { dir / len } else { Vec2::X };
    let perp = Vec2::new(-unit.y, unit.x);

    let s = (seed.value() & 0xFFFF_FFFF) as u32;
    let m1 = Fbm::<Perlin>::new(s).set_octaves(3);
    let m2 = Fbm::<Perlin>::new(s.wrapping_add(9_731)).set_octaves(3);

    let mut pts = Vec::with_capacity(samples + 1);
    for i in 0..=samples {
        let frac = i as f32 / samples as f32;
        let x = frac as f64 * waves as f64;
        let o1 = m1.get([x, 0.0]) as f32;
        let o2 = m2.get([x * 2.7, 41.3]) as f32;
        let off = (o1 * 0.7 + o2 * 0.3) * amplitude;
        pts.push(from + dir * frac + perp * off);
    }
    Curve::new_open(pts)
}

/// Настоящая извилистая кривая из `from` в `to` (Catmull–Rom).
///
/// В отличие от [`warped_line`] (зашумлённая прямая), путь строится через
/// случайные контрольные точки, разбросанные вокруг прямой — кривая получает
/// заметные изгибы, а не «волны» вокруг оси. `amplitude` — размах отклонения
/// контрольных точек от прямой, `bends` — число изгибов, `samples` — точек на
/// сегмент сплайна (кривизна сглаживается).
pub fn winding_path(
    from: Vec2,
    to: Vec2,
    amplitude: f32,
    bends: usize,
    samples: usize,
    seed: Seed,
) -> Curve {
    let mut rng = seed.rng();
    let dir = to - from;
    let len = dir.length();
    let unit = if len > 1e-6 { dir / len } else { Vec2::X };
    let perp = Vec2::new(-unit.y, unit.x);

    // Контрольные точки: блуждание вокруг прямой с затуханием к концам
    // (кривая плавно входит/выходит через границы карты).
    let mut ctrl = Vec::with_capacity(bends + 2);
    ctrl.push(from);
    let mut side = if rng.random::<bool>() { 1.0 } else { -1.0 };
    for k in 1..=bends {
        let frac = k as f32 / (bends + 1) as f32;
        let base = from + dir * frac;
        let envelope = (frac * std::f32::consts::PI).sin();
        let off = side * rng.random_range(0.55..1.0) * amplitude * envelope;
        ctrl.push(base + perp * off);
        side = -side;
    }
    ctrl.push(to);

    // Catmull-Rom: плавная кривая через контрольные точки.
    let mut pts = Vec::with_capacity((ctrl.len() - 1) * samples + 1);
    for i in 0..ctrl.len() - 1 {
        let p0 = if i == 0 { ctrl[0] } else { ctrl[i - 1] };
        let p1 = ctrl[i];
        let p2 = ctrl[i + 1];
        let p3 = if i + 2 < ctrl.len() {
            ctrl[i + 2]
        } else {
            ctrl[ctrl.len() - 1]
        };
        for s in 0..samples {
            let t = s as f32 / samples as f32;
            pts.push(catmull_rom(p0, p1, p2, p3, t));
        }
    }
    pts.push(to);
    Curve::new_open(pts)
}

/// Точка кубического сплайна Катмулла–Рома в параметре `t` (`0..=1`).
fn catmull_rom(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f32) -> Vec2 {
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * ((2.0 * p1)
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
}

/// Замкнутый «живой» контур: эллипс с радиальным шумом (форма пруда/котловины).
///
/// Радиус в каждой точке лёгко «дышит» вокруг `1.0` на `wobble`, поэтому контур
/// получается неровным и естественным, а не идеальным эллипсом.
pub fn warped_contour(
    center: Vec2,
    rx: f32,
    ry: f32,
    wobble: f32,
    samples: usize,
    seed: Seed,
) -> Vec<Vec2> {
    let s = (seed.value() & 0xFFFF_FFFF) as u32;
    let m1 = Fbm::<Perlin>::new(s).set_octaves(3);
    let m2 = Fbm::<Perlin>::new(s.wrapping_add(5_137)).set_octaves(3);

    let mut pts = Vec::with_capacity(samples);
    for i in 0..samples {
        let a = TAU * i as f32 / samples as f32;
        let n = m1.get([a as f64 * 2.0, 0.0]) as f32 * 0.7
            + m2.get([a as f64 * 5.0, 9.0]) as f32 * 0.3;
        let r = 1.0 + wobble * n;
        pts.push(center + Vec2::new(rx * r * a.cos(), ry * r * a.sin()));
    }
    pts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn straight_nearest_and_sign() {
        // Горизонтальная линия вдоль X на y=0.
        let c = straight(Vec2::new(-10.0, 0.0), Vec2::new(10.0, 0.0));
        let nb = c.nearest(Vec2::new(2.0, 3.0));
        assert!((nb.distance - 3.0).abs() < 1e-4);
        // Соглашение: положительная сторона — та, где кросс-произведение
        // (p − a)×(b − a) > 0, т.е. «правая» сторона направления кривой.
        // Для направления +X правая сторона — это Y < 0.
        assert!(c.signed_distance(Vec2::new(0.0, -2.0)) > 0.0);
        assert!(c.signed_distance(Vec2::new(0.0, 2.0)) < 0.0);
    }

    #[test]
    fn closed_contour_inside_negative() {
        let c = Curve::new_closed(warped_contour(Vec2::ZERO, 10.0, 6.0, 0.0, 64, Seed::new(1)));
        assert!(c.signed_distance_closed(Vec2::new(0.0, 0.0)) < 0.0);
        assert!(c.signed_distance_closed(Vec2::new(30.0, 0.0)) > 0.0);
        // На границе — почти ноль.
        assert!(c.signed_distance_closed(Vec2::new(10.0, 0.0)).abs() < 1.0);
    }

    #[test]
    fn warped_line_keeps_endpoints_roughly() {
        let c = warped_line(Vec2::ZERO, Vec2::new(100.0, 0.0), 5.0, 2.0, 64, Seed::new(7));
        assert!(c.points.len() == 65);
        assert!(c.length > 100.0);
    }

    #[test]
    fn winding_path_is_smooth_and_curvy() {
        let c = winding_path(
            Vec2::new(-50.0, 0.0),
            Vec2::new(50.0, 0.0),
            10.0,
            4,
            24,
            Seed::new(9),
        );
        // Непрерывная плавная кривая с заметными изгибами.
        assert!(c.points.len() >= 100, "{}", c.points.len());
        assert!(c.length > 100.5, "кривая должна быть длиннее прямой: {}", c.length);
        // Крайние точки совпадают с концами пути.
        assert_eq!(c.points[0], Vec2::new(-50.0, 0.0));
        assert_eq!(*c.points.last().unwrap(), Vec2::new(50.0, 0.0));
        // Проверка извилистости: отклонение от прямой к оси заметное.
        let max_dev = c.points.iter().map(|p| p.y.abs()).fold(0.0f32, f32::max);
        assert!(max_dev > 1.0, "должны быть заметные изгибы: {max_dev}");
    }
}
