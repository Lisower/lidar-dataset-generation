//! Пресеты ландшафта: архетипы и детерминированный разброс от seed.
//!
//! Идея — как в `plants-generator`: генерация строит один и тот же тип данных
//! ([`LandscapeConfig`]), но в рамках архетипа ([`LandscapePreset`]) параметры
//! берутся из эталона и ограниченно «дрожат» от seed. `resolve(preset, seed)` —
//! чистая функция: один и тот же пресет и seed всегда дают одну и ту же
//! конфигурацию.
//!
//! ВОЗМОЖНЫЕ ПРЕСЕТЫ (из тз):
//! равнина, склон, равнина-склон, разноуровневая территория (2–3 уровня),
//! уступ каньона, ущелье (в т.ч. сухое), речка, пруд, берег реки/озера,
//! наклонная река, водопады. Грунт везде может быть менее/более скалистым
//! (частота/амплитуда шума).
//!
//! Линии рельефа и реки — настоящие извилистые кривые ([`winding_path`]),
//! идут от любой грани карты к любой (в т.ч. диагонально); террасы не
//! обязаны быть параллельными. Русла и ущелья могут менять ширину вдоль оси,
//! их профиль — треугольный или трапециевидный, а не квадратный.

use bevy::ecs::resource::Resource;
use bevy::math::Vec2;
use generation_core::Seed;
use rand::rngs::StdRng;
use rand::RngExt;

use super::config::{BasePlane, LandscapeConfig, Longitudinal, TerrainNoise};
use super::curve::{warped_contour, winding_path, Curve};
use super::features::{BowlFeature, ChannelFeature, StepFeature, TerrainFeature};
use super::profile::Profile;

/// Пресет ландшафта (архетип).
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LandscapePreset {
    /// Равнина: плоская база + лёгкий шум, без воды.
    #[default]
    Plain,
    /// Склон: наклонная базовая плоскость.
    Slope,
    /// Равнина — склон: плоская часть переходит в постоянный уклон.
    PlainSlope,
    /// Разноуровневая территория: 2–3 террасы на разных высотах.
    MultiLevel,
    /// Уступ каньона: обрыв между двумя уровнями.
    CanyonLedge,
    /// Ущелье: высокие стены с двух сторон, русло с водой или сухое.
    Gorge,
    /// Речка: извилистое русло в низине.
    River,
    /// Пруд: замкнутая чаша с водой.
    Pond,
    /// Берег реки/озера: суша плавно уходит под воду большого водоёма.
    Shore,
    /// Наклонная (горная) река с заметным падением вдоль русла.
    SlopingRiver,
    /// Водопады: один или несколько каскадов вдоль русла.
    Waterfalls,
}

impl LandscapePreset {
    /// Все пресеты в порядке переключения.
    pub const ALL: [Self; 11] = [
        Self::Plain,
        Self::Slope,
        Self::PlainSlope,
        Self::MultiLevel,
        Self::CanyonLedge,
        Self::Gorge,
        Self::River,
        Self::Pond,
        Self::Shore,
        Self::SlopingRiver,
        Self::Waterfalls,
    ];

    /// Название для панели информации.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Plain => "равнина",
            Self::Slope => "склон",
            Self::PlainSlope => "равнина-склон",
            Self::MultiLevel => "разноуровневая территория",
            Self::CanyonLedge => "уступ каньона",
            Self::Gorge => "ущелье",
            Self::River => "речка",
            Self::Pond => "пруд",
            Self::Shore => "берег реки/озера",
            Self::SlopingRiver => "наклонная река",
            Self::Waterfalls => "водопады",
        }
    }

    /// Следующий пресет по кругу.
    pub fn next(self) -> Self {
        match Self::ALL.iter().position(|&p| p == self) {
            Some(i) => Self::ALL[(i + 1) % Self::ALL.len()],
            None => Self::ALL[0],
        }
    }

    /// Разобрать пресет из имени (для CLI и тестов).
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "plain" | "равнина" | "ровнина" => Some(Self::Plain),
            "slope" | "склон" => Some(Self::Slope),
            "plainslope" | "равнина-склон" | "равнина_склон" => Some(Self::PlainSlope),
            "multilevel" | "разноуровневая" | "террасы" | "терраса" => Some(Self::MultiLevel),
            "canyon" | "уступ каньона" | "уступ" => Some(Self::CanyonLedge),
            "gorge" | "ущелье" | "каньон" => Some(Self::Gorge),
            "river" | "речка" | "река" => Some(Self::River),
            "pond" | "пруд" | "озеро" => Some(Self::Pond),
            "shore" | "берег" => Some(Self::Shore),
            "slopingriver" | "наклонная река" | "горная река" => Some(Self::SlopingRiver),
            "waterfall" | "waterfalls" | "водопад" | "водопады" => Some(Self::Waterfalls),
            _ => None,
        }
    }
}

/// Полностью разрешённая конфигурация пресета `preset` от `seed`.
pub fn resolve(preset: LandscapePreset, seed: Seed) -> LandscapeConfig {
    let mut cfg = match preset {
        LandscapePreset::Plain => plain(seed),
        LandscapePreset::Slope => slope(seed),
        LandscapePreset::PlainSlope => plain_slope(seed),
        LandscapePreset::MultiLevel => multi_level(seed),
        LandscapePreset::CanyonLedge => canyon_ledge(seed),
        LandscapePreset::Gorge => gorge(seed),
        LandscapePreset::River => river(seed),
        LandscapePreset::Pond => pond(seed),
        LandscapePreset::Shore => shore(seed),
        LandscapePreset::SlopingRiver => sloping_river(seed),
        LandscapePreset::Waterfalls => waterfalls(seed),
    };
    cfg.seed = seed;
    cfg.label = preset.label();
    cfg
}

/// Детерминированный поток случайности пресета.
fn rng_for(preset: LandscapePreset, seed: Seed) -> StdRng {
    seed.with_label(preset.label()).rng()
}

/// Диапазон случайной величины.
fn range<R: rand::Rng + ?Sized>(rng: &mut R, lo: f32, hi: f32) -> f32 {
    rng.random_range(lo..=hi)
}

/// Базовый шум каменистости с «дыханием» от seed. Разброс частоты и
/// амплитуды сознательно широкий и смещён в сторону бóльших значений —
/// поверхность получается каменистой, а не ровной.
fn rocky(preset: LandscapePreset, seed: Seed, amp: (f32, f32), freq: (f32, f32)) -> TerrainNoise {
    let mut rng = rng_for(preset, seed);
    TerrainNoise {
        octaves: 6,
        frequency: range(&mut rng, freq.0, freq.1),
        lacunarity: 2.0,
        persistence: 0.5,
        amplitude: range(&mut rng, amp.0, amp.1),
    }
}

/// Случайная точка на грани карты (любое направление входа/выхода линии).
fn edge_point(rng: &mut StdRng) -> Vec2 {
    let half = 48.0;
    match rng.random_range(0usize..4) {
        0 => Vec2::new(rng.random_range(-half..half), half),
        1 => Vec2::new(rng.random_range(-half..half), -half),
        2 => Vec2::new(half, rng.random_range(-half..half)),
        _ => Vec2::new(-half, rng.random_range(-half..half)),
    }
}

/// Две случайные точки на гранях, удалённые друг от друга не меньше
/// `min_span` — чтобы линия/русло действительно пересекало карту.
fn random_span(rng: &mut StdRng, min_span: f32) -> (Vec2, Vec2) {
    loop {
        let a = edge_point(rng);
        let b = edge_point(rng);
        if a.distance(b) >= min_span {
            return (a, b);
        }
    }
}

/// Слегка изогнутая «линия шва» через `center` под углом `angle`: концы далеко
/// за картой, кривизна ограничена `amplitude` (террасы, уступы, берега).
fn seam_through(center: Vec2, angle: f32, amplitude: f32, seed: Seed) -> Curve {
    let d = Vec2::new(angle.cos(), angle.sin());
    let extent = 500.0;
    winding_path(
        center - d * extent,
        center + d * extent,
        amplitude.max(0.0),
        2,
        24,
        seed,
    )
}

/// Узлы переменной полуширины русла вдоль оси `(t, полуширина)` вокруг
/// среднего значения — русло/ущелье разной ширины в разных частях.
fn width_nodes(rng: &mut StdRng, mean: f32, spread: f32, min: f32) -> Option<Vec<Vec2>> {
    let n = rng.random_range(3usize..=5);
    let nodes: Vec<Vec2> = (0..=n)
        .map(|k| {
            let t = k as f32 / n as f32;
            let w = (mean + rng.random_range(-1.0..1.0) * spread).max(min);
            Vec2::new(t, w)
        })
        .collect();
    Some(nodes)
}

/// Равнина.
fn plain(seed: Seed) -> LandscapeConfig {
    let mut rng = rng_for(LandscapePreset::Plain, seed);
    LandscapeConfig {
        seed,
        base: BasePlane::flat(range(&mut rng, 2.0, 5.0)),
        noise: rocky(LandscapePreset::Plain, seed, (0.5, 1.1), (0.05, 0.1)),
        ..default_config()
    }
}

/// Склон: наклон плоскости — в случайном направлении.
fn slope(seed: Seed) -> LandscapeConfig {
    let mut rng = rng_for(LandscapePreset::Slope, seed);
    let g = range(&mut rng, 0.18, 0.32) * if rng.random::<bool>() { 1.0 } else { -1.0 };
    let use_x = rng.random::<bool>();
    let (gx, gy) = if use_x { (g, 0.0) } else { (0.0, g) };
    let mut cfg = LandscapeConfig {
        seed,
        base: BasePlane { z0: 15.0, gx, gy },
        noise: rocky(LandscapePreset::Slope, seed, (0.5, 1.2), (0.07, 0.14)),
        ..default_config()
    };
    cfg.base.z0 = range(&mut rng, 10.0, 18.0);
    cfg
}

/// Равнина — склон: одна слегка изогнутая линия, с одной стороны плоско,
/// с другой — уклон.
fn plain_slope(seed: Seed) -> LandscapeConfig {
    let mut rng = rng_for(LandscapePreset::PlainSlope, seed);
    let y0 = range(&mut rng, -35.0, 35.0);
    let angle = range(&mut rng, -0.4, 0.4);
    let slope = range(&mut rng, 0.12, 0.24);
    let band = range(&mut rng, 12.0, 22.0);
    let step = TerrainFeature::Step(StepFeature {
        curve: seam_through(
            Vec2::new(0.0, y0),
            angle,
            range(&mut rng, 1.0, 2.5),
            seed.with_label("равнина-склон-линия"),
        ),
        profile: Profile::plain_to_slope(slope, band),
    });

    LandscapeConfig {
        seed,
        base: BasePlane::flat(range(&mut rng, 5.0, 9.0)),
        features: vec![step],
        noise: rocky(LandscapePreset::PlainSlope, seed, (0.5, 1.0), (0.06, 0.12)),
        ..default_config()
    }
}

/// Разноуровневая территория: 2–3 террасы на разных высотах. Террасы не
/// обязаны быть параллельными — у каждой свой угол и лёгкая кривизна.
fn multi_level(seed: Seed) -> LandscapeConfig {
    let mut rng = rng_for(LandscapePreset::MultiLevel, seed);
    let levels = if rng.random::<f32>() < 0.55 { 2 } else { 3 };
    // Разные высоты уступов; направление подъёма — вверх или вниз.
    let dir = if rng.random::<bool>() { 1.0 } else { -1.0 };
    // Случайные позиции линий (по «вдоль»-координате).
    let mut positions: Vec<f32> = (0..levels)
        .map(|_| range(&mut rng, -42.0, 42.0))
        .collect();
    positions.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let mut features = Vec::with_capacity(levels);
    for (k, &off) in positions.iter().enumerate() {
        let drop = dir * range(&mut rng, 3.0, 6.0);
        let width = range(&mut rng, 3.0, 10.0);
        let angle = range(&mut rng, -0.6, 0.6);
        let amp = range(&mut rng, 1.5, 3.5);
        features.push(TerrainFeature::Step(StepFeature {
            curve: seam_through(
                Vec2::new(0.0, off),
                angle,
                amp,
                seed.with_label("терраса").child(k as u64),
            ),
            profile: Profile::step_flat(drop, width, true),
        }));
    }

    LandscapeConfig {
        seed,
        base: BasePlane::flat(range(&mut rng, 4.0, 7.0)),
        features,
        noise: rocky(LandscapePreset::MultiLevel, seed, (0.6, 1.2), (0.07, 0.13)),
        ..default_config()
    }
}

/// Уступ каньона: длинный извилистый обрыв между двумя уровнями.
fn canyon_ledge(seed: Seed) -> LandscapeConfig {
    let mut rng = rng_for(LandscapePreset::CanyonLedge, seed);
    let (start, end) = random_span(&mut rng, 80.0);
    let drop = -range(&mut rng, 7.0, 13.0);
    let width = range(&mut rng, 1.2, 3.0);
    let feature = TerrainFeature::Step(StepFeature {
        curve: winding_path(
            start,
            end,
            range(&mut rng, 1.5, 3.5),
            rng.random_range(2usize..=4),
            24,
            seed.with_label("уступ-ось"),
        ),
        profile: Profile::step_flat(drop, width, false),
    });

    LandscapeConfig {
        seed,
        base: BasePlane::flat(range(&mut rng, 13.0, 19.0)),
        features: vec![feature],
        // Скалистый обрыв: шум помельче и посильнее.
        noise: rocky(LandscapePreset::CanyonLedge, seed, (0.7, 1.5), (0.09, 0.16)),
        ..default_config()
    }
}

/// Ущелье: высокие стены с двух сторон; русло может быть с водой или сухим.
/// Зазор — гладкий V/корыто (без «борозд»), ширина меняется вдоль оси,
/// стены — сухие пологие пандусы до базового плато.
fn gorge(seed: Seed) -> LandscapeConfig {
    let mut rng = rng_for(LandscapePreset::Gorge, seed);
    let (start, end) = random_span(&mut rng, 60.0);
    let base_h = range(&mut rng, 14.0, 18.0);
    let water = range(&mut rng, 2.0, 3.5).min(base_h - 1.0);
    let half_width = range(&mut rng, 5.0, 8.0);
    // Чаще — с водой (классическое ущелье с рекой на дне), но бывает и сухим.
    let wet = rng.random::<f32>() < 0.65;
    let width_spread = range(&mut rng, 2.5, 4.0);
    let channel = TerrainFeature::Channel(ChannelFeature {
        curve: winding_path(
            start,
            end,
            range(&mut rng, 4.0, 8.0),
            rng.random_range(3usize..=5),
            24,
            seed.with_label("ущелье-ось"),
        ),
        half_width,
        width_nodes: width_nodes(&mut rng, half_width, width_spread, 2.5),
        bank: range(&mut rng, 6.0, 10.0),
        bank_top: base_h + range(&mut rng, 2.0, 4.0),
        depth: range(&mut rng, 5.0, 7.0),
        wet,
        water: Longitudinal::Flat { water },
    });

    LandscapeConfig {
        seed,
        base: BasePlane::flat(base_h),
        features: vec![channel],
        noise: rocky(LandscapePreset::Gorge, seed, (0.7, 1.5), (0.09, 0.16)),
        ..default_config()
    }
}

/// Речка: широкое извилистое русло, идёт от любой грани карты к любой.
/// Вода — в русле (дно), берега — сухие пологие склоны невысокой подрезки.
fn river(seed: Seed) -> LandscapeConfig {
    let mut rng = rng_for(LandscapePreset::River, seed);
    let (start, end) = random_span(&mut rng, 60.0);
    let base_h = range(&mut rng, 4.2, 5.8);
    // Вода держится заметно ниже поймы, чтобы берега были невысокими.
    let water = range(&mut rng, 2.4, 3.6).min(base_h - 0.5);
    let half_width = range(&mut rng, 3.0, 5.0);
    let width_spread = range(&mut rng, 1.5, 2.5);
    let channel = TerrainFeature::Channel(ChannelFeature {
        curve: winding_path(
            start,
            end,
            range(&mut rng, 6.0, 10.0),
            rng.random_range(3usize..=5),
            24,
            seed.with_label("речка-ось"),
        ),
        half_width,
        width_nodes: width_nodes(&mut rng, half_width, width_spread, 1.5),
        bank: range(&mut rng, 7.0, 11.0),
        bank_top: base_h + range(&mut rng, 1.5, 3.0),
        depth: range(&mut rng, 1.4, 2.0),
        wet: true,
        water: Longitudinal::Flat { water },
    });

    LandscapeConfig {
        seed,
        base: BasePlane::flat(base_h),
        features: vec![channel],
        noise: rocky(LandscapePreset::River, seed, (0.4, 0.9), (0.06, 0.11)),
        ..default_config()
    }
}

/// Пруд: крупная замкнутая «живая» чаша с пологими берегами и водой
/// до самой кромки (подводная отмель, без сухих «колец»-ступенек).
fn pond(seed: Seed) -> LandscapeConfig {
    let mut rng = rng_for(LandscapePreset::Pond, seed);
    let center = Vec2::new(range(&mut rng, -16.0, 16.0), range(&mut rng, -16.0, 16.0));
    let rx = range(&mut rng, 12.0, 18.0);
    let ry = range(&mut rng, 15.0, 24.0);
    let contour_pts = warped_contour(
        center,
        rx,
        ry,
        0.14,
        96,
        seed.with_label("пруд-контур"),
    );
    let contour = Curve::new_closed(contour_pts);
    // Окружающая суша заметно выше уровня воды — у самой кромки всегда есть
    // заметный вертикальный обрыв (запас больше амплитуды шума, поэтому шум
    // не «съедает» стенку даже в минимумах).
    let base_h = range(&mut rng, 6.0, 7.2);
    let water = range(&mut rng, 3.2, 4.2).min(base_h - 1.0);
    let bowl = TerrainFeature::Bowl(BowlFeature {
        contour,
        reach: rx.max(ry) * 1.2,
        water_level: water,
        max_depth: range(&mut rng, 2.4, 3.4),
        bottom: range(&mut rng, 2.2, 3.2),
        shore: range(&mut rng, 0.15, 0.28),
        // Ширина залитой полки до внешнего обрыва, м.
        rim: range(&mut rng, 3.5, 6.5),
    });

    LandscapeConfig {
        seed,
        base: BasePlane::flat(base_h),
        water_level: Some(water),
        features: vec![bowl],
        noise: rocky(LandscapePreset::Pond, seed, (0.35, 0.8), (0.06, 0.11)),
        ..default_config()
    }
}

/// Берег реки/озера: наклонная равнина, одна часть суша, другая под водой;
/// извилистая линия у уреза делает береговую линию неровной. Ось наклона —
/// случайная (X или Y), береговая линия всегда видна.
fn shore(seed: Seed) -> LandscapeConfig {
    let mut rng = rng_for(LandscapePreset::Shore, seed);
    let water = range(&mut rng, 0.8, 1.6);
    let g = range(&mut rng, 0.07, 0.14);
    let use_x = rng.random::<bool>();
    // Координата уреза вдоль оси наклона — гарантированно внутри карты,
    // чтобы вода занимала заметный угол и береговая линия была видна.
    let coord = range(&mut rng, -38.0, -10.0);
    let z0 = water - coord * g;
    let (gx, gy) = if use_x { (g, 0.0) } else { (0.0, g) };

    // Извилистая «канавка» вдоль уреза — заливы и мысы. Линия идёт вдоль
    // перпендикулярной оси на всю карту.
    let (a, b) = if use_x {
        (Vec2::new(coord, -50.0), Vec2::new(coord, 50.0))
    } else {
        (Vec2::new(-50.0, coord), Vec2::new(50.0, coord))
    };
    let dip = TerrainFeature::Step(StepFeature {
        curve: winding_path(
            a,
            b,
            range(&mut rng, 3.0, 6.0),
            rng.random_range(2usize..=4),
            24,
            seed.with_label("берег-линия"),
        ),
        profile: Profile::dip(range(&mut rng, 0.8, 1.4), range(&mut rng, 5.0, 9.0)),
    });

    LandscapeConfig {
        seed,
        base: BasePlane { z0, gx, gy },
        water_level: Some(water),
        features: vec![dip],
        noise: rocky(LandscapePreset::Shore, seed, (0.4, 0.8), (0.055, 0.1)),
        ..default_config()
    }
}

/// Наклонная (горная) река: вода заметно падает вдоль русла, дно
/// треугольное/трапециевидное; ось наклона и стороны карты — случайные.
fn sloping_river(seed: Seed) -> LandscapeConfig {
    let mut rng = rng_for(LandscapePreset::SlopingRiver, seed);
    let g = range(&mut rng, 0.16, 0.24);
    let use_x = rng.random::<bool>();
    let sign = if rng.random::<bool>() { 1.0 } else { -1.0 };
    let (gx, gy) = if use_x { (g * sign, 0.0) } else { (0.0, g * sign) };
    let half = 50.0;
    let base = BasePlane { z0: 16.0, gx, gy };
    // Исток — на высоком краю, устье — на низком (вдоль оси наклона).
    let (start, end) = if use_x {
        (
            Vec2::new(sign * half, range(&mut rng, -20.0, 20.0)),
            Vec2::new(-sign * half, range(&mut rng, -20.0, 20.0)),
        )
    } else {
        (
            Vec2::new(range(&mut rng, -20.0, 20.0), sign * half),
            Vec2::new(range(&mut rng, -20.0, 20.0), -sign * half),
        )
    };
    let start_w = base.height(start) - range(&mut rng, 1.0, 2.0);
    let end_w = base.height(end) - range(&mut rng, 1.0, 2.0);
    let half_width = range(&mut rng, 3.0, 5.0);
    let width_spread = range(&mut rng, 1.5, 2.5);

    let channel = TerrainFeature::Channel(ChannelFeature {
        curve: winding_path(
            start,
            end,
            range(&mut rng, 5.0, 8.0),
            rng.random_range(3usize..=5),
            24,
            seed.with_label("наклонная-ось"),
        ),
        half_width,
        width_nodes: width_nodes(&mut rng, half_width, width_spread, 1.5),
        // Берега — сама наклонная равнина, отдельный пандус не нужен.
        bank: 0.0,
        bank_top: 0.0,
        depth: range(&mut rng, 2.0, 3.0),
        wet: true,
        water: Longitudinal::Linear {
            start: start_w,
            end: end_w,
        },
    });

    LandscapeConfig {
        seed,
        base,
        features: vec![channel],
        noise: rocky(LandscapePreset::SlopingRiver, seed, (0.5, 1.2), (0.07, 0.14)),
        ..default_config()
    }
}

/// Водопады: русло с одним или несколькими каскадами (вертикальными обрывами
/// уровня воды), от любой грани к любой; по бокам — сухие склоны с точками.
fn waterfalls(seed: Seed) -> LandscapeConfig {
    let mut rng = rng_for(LandscapePreset::Waterfalls, seed);
    let (start, end) = random_span(&mut rng, 60.0);
    let base_h = range(&mut rng, 9.5, 12.5);
    let w_top = range(&mut rng, 6.5, 9.5).min(base_h - 0.5);
    let drop1 = range(&mut rng, 6.0, 9.0);
    let second = rng.random::<f32>() < 0.7;
    let drop2 = if second { range(&mut rng, 2.0, 4.0) } else { 0.0 };

    // Узлы продольного профиля: плато → обрыв → бассейн → (обрыв) → спуск.
    let mut nodes: Vec<Vec2> = vec![
        Vec2::new(0.00, w_top),
        Vec2::new(0.34, w_top - 1.0),
        Vec2::new(0.42, w_top - 1.0), // край уступа
        Vec2::new(0.4201, w_top - 1.0 - drop1), // вертикальный обрыв
        Vec2::new(0.55, w_top - 1.0 - drop1),
    ];
    if second {
        let base_w = w_top - 1.0 - drop1;
        nodes.push(Vec2::new(0.62, base_w - 1.0));
        nodes.push(Vec2::new(0.6201, base_w - 1.0 - drop2));
        nodes.push(Vec2::new(0.80, base_w - 1.0 - drop2));
        nodes.push(Vec2::new(1.00, base_w - 1.0 - drop2 - 0.5));
    } else {
        nodes.push(Vec2::new(0.78, w_top - 1.0 - drop1 - 1.0));
        nodes.push(Vec2::new(1.00, w_top - 1.0 - drop1 - 1.5));
    }

    let half_width = range(&mut rng, 5.0, 7.0);
    let channel = TerrainFeature::Channel(ChannelFeature {
        curve: winding_path(
            start,
            end,
            range(&mut rng, 3.0, 5.0),
            rng.random_range(2usize..=4),
            24,
            seed.with_label("водопад-ось"),
        ),
        // Постоянная широкая лента: обе части русла (верх и низ водопада)
        // всегда соприкасаются у обрыва — «стена» воды заметная во всех сидах.
        half_width,
        width_nodes: None,
        bank: range(&mut rng, 8.0, 12.0),
        bank_top: base_h + range(&mut rng, 2.0, 3.0),
        depth: range(&mut rng, 1.8, 2.8),
        wet: true,
        water: Longitudinal::Nodes(nodes),
    });

    LandscapeConfig {
        seed,
        base: BasePlane::flat(base_h),
        features: vec![channel],
        noise: rocky(LandscapePreset::Waterfalls, seed, (0.5, 1.2), (0.07, 0.14)),
        ..default_config()
    }
}

/// Базовая конфигурация по умолчанию (остальные поля заполняются в пресетах).
fn default_config() -> LandscapeConfig {
    LandscapeConfig {
        seed: Seed::default(),
        label: "",
        size_m: 100.0,
        resolution: 0.5,
        base: BasePlane::flat(4.0),
        water_level: None,
        features: Vec::new(),
        noise: TerrainNoise {
            octaves: 6,
            frequency: 0.05,
            lacunarity: 2.0,
            persistence: 0.5,
            amplitude: 0.4,
        },
        point_jitter: 0.03,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_is_deterministic() {
        for preset in LandscapePreset::ALL {
            for seed in 0..4u64 {
                let a = resolve(preset, Seed::new(seed));
                let b = resolve(preset, Seed::new(seed));
                assert_eq!(a.features, b.features, "{preset:?}");
                assert_eq!(a.base, b.base, "{preset:?}");
                assert_eq!(a.noise, b.noise, "{preset:?}");
            }
        }
    }

    #[test]
    fn water_presets_have_water_level() {
        for p in [LandscapePreset::Pond, LandscapePreset::Shore] {
            let c = resolve(p, Seed::new(1));
            assert!(c.water_level.is_some(), "{p:?}");
        }
        for p in [
            LandscapePreset::Plain,
            LandscapePreset::Slope,
            LandscapePreset::PlainSlope,
            LandscapePreset::MultiLevel,
            LandscapePreset::CanyonLedge,
        ] {
            let c = resolve(p, Seed::new(1));
            assert!(c.water_level.is_none(), "{p:?}");
        }
    }

    #[test]
    fn every_preset_generates_without_panic_and_finite() {
        for preset in LandscapePreset::ALL {
            let cfg = resolve(preset, Seed::new(42));
            assert!(cfg.size_m > 0.0);
            assert!(cfg.resolution > 0.0);
        }
    }

    #[test]
    fn channels_are_smooth_wide_and_varied() {
        // У русел широкое дно без «борозд» (гладкий профиль bed>=1),
        // переменная полуширина, а у рек/ущелья — сухие берега (bank_top).
        for preset in [
            LandscapePreset::Gorge,
            LandscapePreset::River,
            LandscapePreset::SlopingRiver,
            LandscapePreset::Waterfalls,
        ] {
            let cfg = resolve(preset, Seed::new(5));
            let channel = cfg.features.iter().find_map(|f| match f {
                TerrainFeature::Channel(c) => Some(c),
                _ => None,
            });
            let c = channel.expect("пресет должен содержать русло");
            assert!(c.half_width >= 3.0, "{preset:?}: русло толще");
            if let Some(nodes) = &c.width_nodes {
                assert!(nodes.len() >= 2, "{preset:?}");
                let min_w = nodes.iter().map(|v| v.y).fold(f32::MAX, f32::min);
                assert!(min_w > 0.0, "{preset:?}: ширина не может быть <= 0");
            }
        }
    }

    #[test]
    fn gorge_can_be_dry_or_wet() {
        let mut wet = false;
        let mut dry = false;
        for seed in 0..24u64 {
            let l = resolve(LandscapePreset::Gorge, Seed::new(seed));
            let c = l
                .features
                .iter()
                .find_map(|f| match f {
                    TerrainFeature::Channel(c) => Some(c),
                    _ => None,
                })
                .unwrap();
            if c.wet {
                wet = true;
            } else {
                dry = true;
            }
        }
        assert!(wet, "ущелье должно уметь быть с водой");
        assert!(dry, "ущелье должно уметь быть сухим");
    }
}
