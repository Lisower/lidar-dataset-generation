//! Конфигурация генерации ландшафта — полностью детерминирована от seed.

use bevy::ecs::resource::Resource;
use bevy::math::Vec2;
use generation_core::Seed;

use super::features::TerrainFeature;

/// Наклонная базовая плоскость: `h = z0 + gx·x + gy·y`.
///
/// Это «начальное поле высот», из которого линии-деформации лепят рельеф.
/// Плоская плоскость даёт равнину, наклонная — склон (горы/холм), при этом
/// сама поверхность может быть ещё и «каменистой» от шума.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BasePlane {
    /// Базовая высота в центре карты, м.
    pub z0: f32,
    /// Наклон по X, м/м.
    pub gx: f32,
    /// Наклон по Y, м/м.
    pub gy: f32,
}

impl BasePlane {
    /// Плоскость без наклона.
    pub const fn flat(z0: f32) -> Self {
        Self { z0, gx: 0.0, gy: 0.0 }
    }

    /// Высота плоскости в точке `p`.
    pub fn height(&self, p: Vec2) -> f32 {
        self.z0 + self.gx * p.x + self.gy * p.y
    }
}

/// Параметры fBm-шума — «каменистость» грунта.
///
/// Частота (`frequency`) задаёт масштаб неровностей: чем она больше, тем
/// мельче и «скалистее» поверхность; амплитуда (`amplitude`) — вертикальный
/// размах в метрах.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainNoise {
    /// Число октав fBm.
    pub octaves: usize,
    /// Пространственная частота, м⁻¹.
    pub frequency: f32,
    /// Лакунарность (рост частоты октав).
    pub lacunarity: f32,
    /// Персистентность (затухание амплитуд октав).
    pub persistence: f32,
    /// Амплитуда шума, м.
    pub amplitude: f32,
}

/// Продольный профиль воды вдоль русла (параметр `t` от 0..1 — вдоль дуги).
#[derive(Debug, Clone, PartialEq)]
pub enum Longitudinal {
    /// Горизонтальная вода (озеро-линза, речка в низине).
    Flat { water: f32 },
    /// Линейное понижение от истока к устью (наклонная река).
    Linear { start: f32, end: f32 },
    /// Узлы `(t, water)` с кусочно-линейной интерполяцией (каскады/водопады).
    Nodes(Vec<Vec2>),
}

impl Longitudinal {
    /// Высота воды в точке с параметром `t`.
    pub fn water(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Flat { water } => *water,
            Self::Linear { start, end } => start + (end - start) * t,
            Self::Nodes(nodes) => {
                let n = nodes.len();
                if n == 0 {
                    return 0.0;
                }
                if n == 1 {
                    return nodes[0].y;
                }
                let lo = nodes[0];
                if t <= lo.x {
                    return lo.y;
                }
                let hi = nodes[n - 1];
                if t >= hi.x {
                    return hi.y;
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
        }
    }
}

/// Конфигурация генерации ландшафта.
#[derive(Resource, Debug, Clone)]
pub struct LandscapeConfig {
    /// Глобальный seed — вся случайность детерминирована от него.
    pub seed: Seed,
    /// Имя пресета (для панели информации и логов).
    pub label: &'static str,
    /// Размер квадратной карты, м (по умолчанию 100).
    pub size_m: f32,
    /// Шаг сетки рельефа, м (0.5 — хорошее разрешение для 100×100).
    pub resolution: f32,

    /// Базовая плоскость.
    pub base: BasePlane,
    /// Общий горизонтальный уровень воды (пруд, берег); `None` — нет.
    pub water_level: Option<f32>,
    /// Линии-деформации рельефа.
    pub features: Vec<TerrainFeature>,

    /// Шум каменистости.
    pub noise: TerrainNoise,

    /// Амплитуда шума точек, м.
    pub point_jitter: f32,
}

impl Default for LandscapeConfig {
    fn default() -> Self {
        Self {
            seed: Seed::default(),
            label: "",
            size_m: 100.0,
            resolution: 0.5,
            base: BasePlane::flat(4.0),
            water_level: None,
            features: Vec::new(),
            noise: TerrainNoise {
                octaves: 5,
                frequency: 0.05,
                lacunarity: 2.0,
                persistence: 0.5,
                amplitude: 0.4,
            },
            point_jitter: 0.03,
        }
    }
}
