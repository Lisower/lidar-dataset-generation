//! Кусочно-линейный профиль высоты как функция расстояния до линии деформации.
//!
//! Профиль — главный «строительный блок» линий-деформаций: он превращает
//! (знаковое) расстояние `d` до кривой в смещение высоты. Так одна и та же
//! кривая умеет создавать уступ («ровно — склон — ровно»), склон с разными
//! углами по сторонам, берега и террасы — достаточно описать узлы `(d, h)`.

use bevy::math::Vec2;

/// Профиль `h = f(d)` от (знакового) расстояния `d` до кривой.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Profile {
    /// Узлы `(d, h)` со строго возрастающим `d`.
    pub nodes: Vec<Vec2>,
    /// Плавная (smoothstep) интерполяция между узлами вместо линейной.
    pub smooth: bool,
}

impl Profile {
    /// Профиль из узлов `(d, h)`.
    pub fn new(nodes: impl Into<Vec<(f32, f32)>>, smooth: bool) -> Self {
        Self {
            nodes: nodes
                .into()
                .into_iter()
                .map(|(d, h)| Vec2::new(d, h))
                .collect(),
            smooth,
        }
    }

    /// Значение профиля в точке `d`.
    ///
    /// Между узлами — линейная или плавная интерполяция; за краями — продолжение
    /// по крайнему наклону (так профиль умеет задавать постоянный уклон склона).
    pub fn eval(&self, d: f32) -> f32 {
        let nodes = &self.nodes;
        let n = nodes.len();
        if n == 0 {
            return 0.0;
        }
        if n == 1 {
            return nodes[0].y;
        }

        // Экстраполяция за края по крайним наклонам.
        if d <= nodes[0].x {
            let a = nodes[0];
            let b = nodes[1];
            let slope = (b.y - a.y) / (b.x - a.x).max(f32::EPSILON);
            return a.y + slope * (d - a.x);
        }
        let last = nodes[n - 1];
        if d >= last.x {
            let a = nodes[n - 2];
            let slope = (last.y - a.y) / (last.x - a.x).max(f32::EPSILON);
            return last.y + slope * (d - last.x);
        }

        // Сегмент, содержащий d.
        let mut i = 0;
        while i + 1 < n && d > nodes[i + 1].x {
            i += 1;
        }
        let a = nodes[i];
        let b = nodes[i + 1];
        let span = b.x - a.x;
        if span <= f32::EPSILON {
            return b.y;
        }
        let t = ((d - a.x) / span).clamp(0.0, 1.0);
        let k = if self.smooth { t * t * (3.0 - 2.0 * t) } else { t };
        a.y + (b.y - a.y) * k
    }

    /// Двусторонний порог: «ровно — склон — ровно». Слева ровно на `0`,
    /// справа ровно на `delta`, переход (уступ/склон) — в полосе `width`.м
    pub fn step_flat(delta: f32, width: f32, smooth: bool) -> Self {
        let w = width * 0.5;
        Self::new(
            vec![(-1e3, 0.0), (-w, 0.0), (w, delta), (1e3, delta)],
            smooth,
        )
    }

    /// «Равнина — склон»: слева плоскость на `0`, правее полосы `band` —
    /// постоянный уклон `slope` (переход сглажен).
    pub fn plain_to_slope(slope: f32, band: f32) -> Self {
        Self::new(vec![(-1e3, 0.0), (0.0, 0.0), (band, band * slope)], true)
    }

    /// Пологий берег: плавный спуск от 0 (суша) к `drop` (ниже уровня воды)
    /// в полосе `width`, с дальнейшей постоянной депрессией (берег реки/озера).
    pub fn shore_drop(drop: f32, width: f32) -> Self {
        Self::new(
            vec![
                (-1e3, 0.0),
                (-width * 0.5, 0.0),
                (width * 0.5, drop),
                (1e3, drop),
            ],
            true,
        )
    }

    /// Симметричная «канавка» вдоль линии: опускается на `depth` у самой
    /// линии и возвращается к 0 за `width` (для извилистых берегов — линия
    /// создаёт заливы/западины независимо от ориентации).
    pub fn dip(depth: f32, width: f32) -> Self {
        let w = width * 0.5;
        Self::new(
            vec![
                (-1e3, 0.0),
                (-w, 0.0),
                (0.0, -depth),
                (w, 0.0),
                (1e3, 0.0),
            ],
            true,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_flat_is_two_plateaus() {
        let p = Profile::step_flat(10.0, 6.0, false);
        assert!((p.eval(-5.0) - 0.0).abs() < 1e-3);
        assert!((p.eval(5.0) - 10.0).abs() < 1e-3);
        // В середине перехода — половина перепада.
        assert!((p.eval(0.0) - 5.0).abs() < 1e-3);
        // За краями — продолжение плоских участков.
        assert!((p.eval(100.0) - 10.0).abs() < 1e-3);
        assert!((p.eval(-100.0) - 0.0).abs() < 1e-3);
    }

    #[test]
    fn plain_to_slope_extrapolates() {
        let p = Profile::plain_to_slope(0.2, 10.0);
        // Слева — плоскость.
        assert!((p.eval(-50.0) - 0.0).abs() < 1e-3);
        // В полосе — подъём.
        assert!(p.eval(10.0) > 1.5);
        // За полосой — постоянный уклон 0.2.
        let far = p.eval(40.0);
        assert!((far - p.eval(30.0) - 0.2 * 10.0).abs() < 0.01, "{far}");
    }
}
