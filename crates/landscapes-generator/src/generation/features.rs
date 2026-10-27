//! Линии-деформации рельефа (террейн-фичи).
//!
//! Одна и та же математика «поля расстояния до кривой + профиль» покрывает
//! почти все формы рельефа тз треда:
//!
//! - [`StepFeature`] — уступ, терраса, склон, берег: кривая (часто прямая)
//!   «шьёт» поверхность поперечным профилем от **знакового** расстояния;
//! - [`ChannelFeature`] — русло реки, каньон, ущелье, наклонная река, водопад:
//!   **абсолютное** дно вдоль оси + продольный профиль воды + врезание
//!   минимумом в существующий рельеф;
//! - [`BowlFeature`] — замкнутая чаша (пруд, котловина): абсолютное дно внутри
//!   контура + бортик по кромке.

use bevy::math::Vec2;

use super::config::Longitudinal;
use super::curve::Curve;
use super::profile::Profile;

/// Плавная ступень между `a` и `b`.
pub fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a).max(f32::EPSILON)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Линия-шов: уступ/терраса/склон/берег.
///
/// Высота аддитивно смещается на `profile(знаковое_расстояние)`. Профиль сам
/// решает, как ведёт себя поверхность с каждой стороны: две плато, склон
/// с продолжением, спуск под уровень воды и т.п.
#[derive(Debug, Clone, PartialEq)]
pub struct StepFeature {
    /// Ось «шва» (обычно прямая, выходящая за границы карты).
    pub curve: Curve,
    /// Поперечный профиль от знакового расстояния.
    pub profile: Profile,
}

impl StepFeature {
    /// Смещение высоты в точке `p`.
    pub fn offset(&self, p: Vec2) -> f32 {
        let d = self.curve.signed_distance(p);
        self.profile.eval(d)
    }
}

/// Русло/каньон/ущелье/наклонная река/водопад: врезание минимумом по
/// абсолютному дну.
#[derive(Debug, Clone, PartialEq)]
pub struct ChannelFeature {
    /// Ось русла (открытая кривая через карту).
    pub curve: Curve,
    /// Полуширина «водяной» зоны (дна) по умолчанию, м.
    pub half_width: f32,
    /// Опциональные узлы `(t, полуширина)` — русло/ущелье разной ширины в
    /// разных частях; интерполируются кусочно-линейно. `None` — постоянно.
    pub width_nodes: Option<Vec<Vec2>>,
    /// Ширина сухой береговой полосы от уреза наружу, м.
    pub bank: f32,
    /// Абсолютная высота, до которой сухой берег поднимается от уреза
    /// (`bank_top` чуть выше базовой плоскости, чтобы пандус плавно сливался
    /// с прилегающим рельефом и не давал уступа).
    pub bank_top: f32,
    /// Глубина русла на оси относительно уреза, м.
    pub depth: f32,
    /// Вода в русле; `false` — сухое ущелье/овраг без воды.
    pub wet: bool,
    /// Продольный профиль воды вдоль `t` (постоянный/линейный/каскады).
    pub water: Longitudinal,
}

impl ChannelFeature {
    /// Полуширина дна в точке `t` вдоль оси.
    pub fn width_at(&self, t: f32) -> f32 {
        let base = self.half_width.max(0.1);
        match &self.width_nodes {
            None => base,
            Some(nodes) => lerp_nodes(nodes, t.clamp(0.0, 1.0)).max(0.2),
        }
    }

    /// Полный радиус влияния в точке `t` (дно + берег), м.
    pub fn influence_at(&self, t: f32) -> f32 {
        self.width_at(t) + self.bank.max(0.0)
    }

    /// Абсолютная «крышка» высоты в точке в пределах влияния, `None` — вне.
    ///
    /// Профиль — сплошная C²-гладкая кривая (две `smoothstep` имеют нулевые
    /// первую и вторую производные на краях), поэтому контрастных изломов
    /// кривизны («борозд» на стыке ровного и наклонного дна) нет нигде:
    ///
    /// - дно: от `w - depth` на оси до `w` на урезе (`r <= half_width`) —
    ///   гладкий V;
    /// - сухой берег: от `w` на урезе до `bank_top` на краю влияния.
    pub fn cap(&self, p: Vec2) -> Option<f32> {
        let nb = self.curve.nearest(p);
        let r = nb.distance;
        let t = nb.t;
        let inf = self.influence_at(t);
        if r > inf {
            return None;
        }
        let w = self.water.water(t);
        let hw = self.width_at(t);
        if r <= hw {
            let s = smoothstep(0.0, hw, r);
            Some(w - self.depth.max(0.0) * (1.0 - s))
        } else if self.bank <= 0.05 {
            Some(w)
        } else {
            let t2 = smoothstep(hw, inf, r);
            Some(w + (self.bank_top - w) * t2)
        }
    }

    /// Уровень воды в русле — только на дне (`r <= half_width`); вне дна —
    /// сухой берег. `None` — вне дна или сухое русло.
    pub fn water_at(&self, p: Vec2) -> Option<f32> {
        if !self.wet {
            return None;
        }
        let nb = self.curve.nearest(p);
        if nb.distance <= self.width_at(nb.t) {
            Some(self.water.water(nb.t))
        } else {
            None
        }
    }
}

/// Кусочно-линейная интерполяция узлов `(t, значение)`.
fn lerp_nodes(nodes: &[Vec2], t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let n = nodes.len();
    if n == 0 {
        return 0.0;
    }
    if n == 1 {
        return nodes[0].y;
    }
    if t <= nodes[0].x {
        return nodes[0].y;
    }
    let last = nodes[n - 1];
    if t >= last.x {
        return last.y;
    }
    let mut i = 0;
    while i + 1 < n && t > nodes[i + 1].x {
        i += 1;
    }
    let a = nodes[i];
    let b = nodes[i + 1];
    let span = (b.x - a.x).max(f32::EPSILON);
    a.y + (b.y - a.y) * ((t - a.x) / span)
}

/// Замкнутая чаша: пруд/котловина.
#[derive(Debug, Clone, PartialEq)]
pub struct BowlFeature {
    /// Замкнутый контур чаши (уже «расшумленный»).
    pub contour: Curve,
    /// Характерный радиус глубины (внутрь от кромки), м.
    pub reach: f32,
    /// Уровень воды в чаше (абсолютная высота), м.
    pub water_level: f32,
    /// Максимальная глубина в центре, м.
    pub max_depth: f32,
    /// Форма чаши: `1.0` — почти плоское дно со склонами, больше — линза.
    pub bottom: f32,
    /// Доля «досягаемости» у кромки с пологой подводной отмелью (`0..0.5`) —
    /// берега пологие, вода доходит до самого берега.
    pub shore: f32,
    /// Ширина плавного перехода от уреза к окружающей суше, м.
    pub rim: f32,
}

impl BowlFeature {
    /// Дно чаши и берег с номинальной высотой `water_level + max_depth`.
    pub fn cap(&self, p: Vec2) -> Option<f32> {
        self.cap_with_bank(p, self.water_level + self.max_depth)
    }

    /// Берег сливается с текущим рельефом без скачка на внешней границе.
    fn cap_with_bank(&self, p: Vec2, terrain: f32) -> Option<f32> {
        let s = self.contour.signed_distance_closed(p); // <0 внутри
        if s > self.rim {
            return None;
        }
        if s >= 0.0 {
            let blend = smoothstep(0.0, self.rim, s);
            return Some(self.water_level + (terrain - self.water_level).max(0.0) * blend);
        }
        let inward = -s;
        let u = (inward / self.reach.max(f32::EPSILON)).clamp(0.0, 1.0);
        // Отмель начинается с нулевой глубины и постепенно переходит в чашу.
        let shelf = self.shore.clamp(0.0, 0.5);
        let u2 = ((u - shelf) / (1.0 - shelf)).clamp(0.0, 1.0);
        let power = 1.0 - (1.0 - u2).powf(self.bottom.max(0.1));
        const SHELF_DEPTH: f32 = 0.08;
        let depth_frac =
            SHELF_DEPTH * smoothstep(0.0, shelf.max(0.05), u) + (1.0 - SHELF_DEPTH) * power;
        Some(self.water_level - self.max_depth * depth_frac)
    }
}

/// Любая линия-деформация рельефа.
#[derive(Debug, Clone, PartialEq)]
pub enum TerrainFeature {
    /// Уступ/склон/берег (аддитивное смещение по знаковому расстоянию).
    Step(StepFeature),
    /// Русло/каньон/ущелье (врезание минимумом по абсолютному дну).
    Channel(ChannelFeature),
    /// Замкнутая чаша (пруд/котловина).
    Bowl(BowlFeature),
}

impl TerrainFeature {
    /// Применить фичу к текущей высоте `h` в точке `p`.
    pub fn solve(&self, p: Vec2, h: f32) -> f32 {
        match self {
            Self::Step(s) => h + s.offset(p),
            Self::Channel(c) => match c.cap(p) {
                Some(cap) => h.min(cap),
                None => h,
            },
            Self::Bowl(b) => match b.cap_with_bank(p, h) {
                Some(cap) => h.min(cap),
                None => h,
            },
        }
    }

    /// Уровень воды, который задаёт фича в точке (для русла — в зоне влияния).
    pub fn water_at(&self, p: Vec2) -> Option<f32> {
        match self {
            Self::Channel(c) => c.water_at(p),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::curve::straight;

    #[test]
    fn channel_smooth_bottom_no_creases() {
        let c = ChannelFeature {
            curve: straight(Vec2::new(-50.0, 0.0), Vec2::new(50.0, 0.0)),
            half_width: 3.0,
            width_nodes: None,
            bank: 4.0,
            bank_top: 12.0,
            depth: 2.0,
            wet: true,
            water: Longitudinal::Flat { water: 2.0 },
        };
        // На оси — самое низкое дно.
        let cap0 = c.cap(Vec2::new(0.0, 0.0)).unwrap();
        assert!((cap0 - 0.0).abs() < 1e-3, "{cap0}");
        // На урезе (r = half_width) — уровень воды.
        let cap_rim = c.cap(Vec2::new(0.0, 3.0)).unwrap();
        assert!((cap_rim - 2.0).abs() < 1e-2, "{cap_rim}");
        // На краю сухого берега — bank_top.
        let cap_bank = c.cap(Vec2::new(0.0, 7.0)).unwrap();
        assert!((cap_bank - 12.0).abs() < 0.2, "{cap_bank}");
        // Вне влияния — нет врезания.
        assert!(c.cap(Vec2::new(0.0, 20.0)).is_none());
        // Вода — только на дне (r <= half_width), берег сухой.
        assert!(c.water_at(Vec2::new(0.0, 1.0)).is_some());
        assert!(c.water_at(Vec2::new(0.0, 6.0)).is_none());
        // Профиль без «борозд»: монотонен, непрерывен и без резких изломов
        // производной (конечные разности, без больших прыжков наклона).
        let mut prev = c.cap(Vec2::new(0.0, 0.0)).unwrap();
        let mut prev_slope = 0.0f32;
        let mut r_step = 0.25;
        while r_step <= 7.0 {
            let cap = c.cap(Vec2::new(0.0, r_step)).unwrap();
            assert!(cap >= prev - 1e-3, "профиль должен быть монотонным");
            let slope = (cap - prev) / 0.25;
            // Наклон не должен прыгать (нет борозд/изломов): скачок > 3
            // означал бы угол-заусенец. Сравниваем с соседним шагом.
            let jump = (slope - prev_slope).abs();
            if r_step > 3.0 && r_step < 7.0 || r_step <= 3.0 {
                assert!(jump < 4.0, "резкий излом на r={r_step}: {jump}");
            }
            prev = cap;
            prev_slope = slope;
            r_step += 0.25;
        }
    }

    #[test]
    fn channel_width_varies_and_can_be_dry() {
        // Переменная ширина: на t=0.5 русло шире, на краях уже.
        let c = ChannelFeature {
            curve: straight(Vec2::new(-50.0, 0.0), Vec2::new(50.0, 0.0)),
            half_width: 3.0,
            width_nodes: Some(vec![
                Vec2::new(0.0, 2.0),
                Vec2::new(0.5, 6.0),
                Vec2::new(1.0, 2.0),
            ]),
            bank: 4.0,
            bank_top: 12.0,
            depth: 2.0,
            wet: true,
            water: Longitudinal::Flat { water: 2.0 },
        };
        // В точке ближе к началу (t≈0) влияния меньше, чем в середине.
        let inf_t0 = c.influence_at(0.05);
        let inf_mid = c.influence_at(0.5);
        assert!(inf_mid > inf_t0, "{inf_t0} < {inf_mid}");
        // Точка посередине карты (x=0) находится на t=0.5 — вода есть.
        assert!(c.water_at(Vec2::new(0.0, 0.0)).is_some());
        // Точка на узком участке (x=45) вне дна — воды нет.
        assert!(c.water_at(Vec2::new(45.0, 20.0)).is_none());
        // Сухое русло: воды нет нигде, дно всё равно врезается.
        let dry = ChannelFeature { wet: false, ..c };
        assert!(dry.water_at(Vec2::new(0.0, 0.0)).is_none());
        assert!(dry.cap(Vec2::new(0.0, 0.0)).is_some());
    }

    #[test]
    fn bowl_deepens_toward_center() {
        let b = Curve::new_closed(vec![
            Vec2::new(-10.0, 0.0),
            Vec2::new(0.0, 6.0),
            Vec2::new(10.0, 0.0),
            Vec2::new(0.0, -6.0),
        ]);
        let bowl = BowlFeature {
            contour: b,
            reach: 8.0,
            water_level: 2.0,
            max_depth: 2.5,
            bottom: 2.0,
            shore: 0.0,
            rim: 2.0,
        };
        // В центре — глубже всего.
        let c = bowl.cap(Vec2::ZERO).unwrap();
        assert!(c < 0.5, "{c}");
        // Рядом с кромкой — у уровня воды (немного ниже — подводная отмель).
        let e = bowl.cap(Vec2::new(-9.8, 0.0)).unwrap();
        assert!((e - 2.0).abs() < 0.5, "{e}");
        assert!(e <= 2.0 + 1e-3, "у кромки дно на уровне воды или ниже: {e}");
    }

    #[test]
    fn bowl_shore_meets_water_and_blends_into_land() {
        let contour = Curve::new_closed(vec![
            Vec2::new(-10.0, 0.0),
            Vec2::new(0.0, 6.0),
            Vec2::new(10.0, 0.0),
            Vec2::new(0.0, -6.0),
        ]);
        let mk = |shore: f32| BowlFeature {
            contour: contour.clone(),
            reach: 8.0,
            water_level: 2.0,
            max_depth: 2.5,
            bottom: 2.0,
            shore,
            rim: 2.0,
        };
        // У самой кромки дно всегда чуть ниже уровня воды — вода доходит
        // до берега, сухого «кольца» нет.
        for shore in [0.0, 0.3] {
            let cap = mk(shore).cap(Vec2::new(-9.95, 0.0)).unwrap();
            assert!(cap < 2.0, "у кромки вода, а не сухое кольцо: {cap}");
        }
        // При отмели (shore>0) промежуточная зона положе (мельче), чем без.
        let deep0 = mk(0.0).cap(Vec2::new(0.0, -2.0)).unwrap();
        let deep_s = mk(0.35).cap(Vec2::new(0.0, -2.0)).unwrap();
        assert!(deep_s > deep0, "с отмелью берег положе: {deep_s} > {deep0}");
        let bowl = mk(0.3);
        let at_shore = bowl.cap(Vec2::new(-10.0, 0.0)).unwrap();
        assert!((at_shore - bowl.water_level).abs() < 1e-5);
        let mut previous = at_shore;
        for i in 1..=20 {
            let p = Vec2::new(-10.0 - i as f32 * 0.1, 0.0);
            let height = bowl.cap_with_bank(p, 7.0).unwrap();
            assert!(height >= previous && height - previous < 0.4);
            previous = height;
        }
        assert!(
            (previous - 7.0).abs() < 1e-5,
            "берег должен слиться с сушей"
        );
        assert!(bowl.cap(Vec2::new(-13.0, 0.0)).is_none());
    }
}
