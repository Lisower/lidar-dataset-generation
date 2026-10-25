//! Ландшафт: поле высот, вода и облака точек.
//!
//! Пайплайн генерации (см. тред и ответы экспертов):
//!
//! 1. структурная высота: базовая плоскость + линии-деформации
//!    (уступы/русла/чаши);
//! 2. fBm-шум каменистости с маскированием у воды — берега чистые, стенки
//!    каньона и уступы шумятся в полную силу;
//! 3. небольшая термо-/гидро-эрозия (см. [`super::erosion`]) — рельеф
//!    становится естественнее, «где вода» при этом не меняется;
//! 4. поверхность воды (горизонтальная плоскость и/или ленты вдоль русел);
//! 5. облака точек по той же сетке: под водой точек грунта нет, крутые стены
//!    уплотняются промежуточными точками вплоть до уреза.

use bevy::math::{Vec2, Vec3};
use generation_core::{PointClass, PointCloud, Seed};
use rand::RngExt;

use super::config::{LandscapeConfig, Longitudinal};
use super::erosion::erode;
use super::features::{smoothstep, TerrainFeature};
use super::noise::NoiseSampler;

/// Перепад уровня воды между соседними узлами (м), выше которого
/// добавляются точки «стенки» (водопад, порог).
const WATERFALL_JUMP: f32 = 0.8;

/// Поле высот на сетке карты.
#[derive(Debug, Clone)]
pub struct Heightfield {
    /// Размер карты, м.
    pub size_m: f32,
    /// Шаг сетки, м.
    pub step: f32,
    /// Число узлов по стороне.
    pub n: usize,
    /// Высоты узлов, `n×n`, индекс `j·n + i`.
    pub values: Vec<f32>,
}

impl Heightfield {
    /// Индекс узла `(i, j)`.
    pub fn index(&self, i: usize, j: usize) -> usize {
        j * self.n + i
    }

    /// Число узлов.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Пусто ли поле.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Координаты узла в плоскости карты (центр карты — в начале координат).
    pub fn node_xy(&self, i: usize, j: usize) -> Vec2 {
        let half = self.size_m * 0.5;
        Vec2::new(-half + i as f32 * self.step, -half + j as f32 * self.step)
    }

    /// Высота узла.
    pub fn get(&self, i: usize, j: usize) -> f32 {
        self.values[self.index(i, j)]
    }

    /// Билинейная интерполяция высоты в произвольной точке плоскости
    /// карты `(x, y)` (центр карты — в начале координат).
    pub fn height_at(&self, x: f32, y: f32) -> f32 {
        sample_height(&self.values, self.n, self.step, self.size_m, x, y)
    }

    /// Минимальная и максимальная высоты.
    pub fn min_max(&self) -> (f32, f32) {
        let mut lo = f32::INFINITY;
        let mut hi = f32::NEG_INFINITY;
        for &v in &self.values {
            lo = lo.min(v);
            hi = hi.max(v);
        }
        (lo, hi)
    }

    /// Итератор узлов: `(i, j, координата)`.
    pub fn nodes(&self) -> impl Iterator<Item = (usize, usize, Vec2)> + '_ {
        (0..self.n).flat_map(move |j| {
            (0..self.n).map(move |i| (i, j, self.node_xy(i, j)))
        })
    }
}

/// Поле уровня воды по узлам: `levels[idx]` — высота водной поверхности
/// или `None`, если узел сухой.
#[derive(Debug, Clone, Default)]
pub struct WaterField {
    /// Высоты водной поверхности, `n×n`.
    pub levels: Vec<Option<f32>>,
}

/// Сгенерированный ландшафт: рельеф, вода и облака точек.
#[derive(Debug)]
pub struct Landscape {
    /// Конфигурация, из которой он получен.
    pub config: LandscapeConfig,
    /// Seed генерации.
    pub seed: Seed,
    /// Поле высот (грунт).
    pub heightfield: Heightfield,
    /// Поле воды.
    pub water: WaterField,
    /// Облако точек грунта (без точек под водой).
    pub ground_cloud: PointCloud,
    /// Облако точек воды (поверхности и стенки водопадов).
    pub water_cloud: PointCloud,
    /// Сплошные вертикальные «занавесы» воды на каскадах (общие для облака
    /// точек и визуального меша воды).
    pub water_walls: Vec<FallsCurtain>,
    /// Минимальная и максимальная высоты рельефа.
    pub min_height: f32,
    pub max_height: f32,
    /// Есть ли вода на карте.
    pub has_water: bool,
}

/// Координата узла на сетке карты.
fn node_xy(i: usize, j: usize, size_m: f32, step: f32) -> Vec2 {
    let half = size_m * 0.5;
    Vec2::new(-half + i as f32 * step, -half + j as f32 * step)
}

/// Уровень воды-«кандидата» в точке: общий горизонтальный уровень и/или
/// вода вдоль русел (в зоне их влияния).
fn water_candidate(config: &LandscapeConfig, p: Vec2) -> Option<f32> {
    let mut w = config.water_level;
    for f in &config.features {
        if let Some(cw) = f.water_at(p) {
            w = Some(cw);
        }
    }
    w
}

/// Сгенерировать ландшафт по конфигурации (детерминированно от seed).
pub fn generate(config: &LandscapeConfig) -> Landscape {
    let seed = config.seed;
    let size_m = config.size_m;
    let step = config.resolution.max(0.05);
    let n = (size_m / step).round() as usize + 1;
    let count = n * n;

    let features = &config.features;
    let sampler = NoiseSampler::new(&config.noise, seed);

    // --- 1. структурная высота (база + линии-деформации) ---
    let mut h_structured = vec![0f32; count];
    let mut candidates = vec![None; count];
    let mut points = vec![Vec2::ZERO; count];

    for j in 0..n {
        for i in 0..n {
            let idx = j * n + i;
            let p = node_xy(i, j, size_m, step);
            points[idx] = p;
            let mut h = config.base.height(p);
            for f in features {
                h = f.solve(p, h);
            }
            h_structured[idx] = h;
            candidates[idx] = water_candidate(config, p);
        }
    }

    // --- 2. fBm-шум с маскированием у воды ---
    let mut heights = vec![0f32; count];
    for idx in 0..count {
        let h0 = h_structured[idx];
        let mask = match candidates[idx] {
            Some(w) => smoothstep(0.0, 0.6, h0 - w),
            None => 1.0,
        };
        heights[idx] = h0 + mask * sampler.sample(points[idx]);
    }

    // --- 2.5. небольшая термо-/гидро-эрозия (детерминированно, без RNG) ---
    // Делает склоны и русла естественнее, но оставляет структуру (уступы,
    // каньоны, каскады). Вода считается по уже эродированной поверхности.
    erode(&mut heights, n, n, step);

    // --- 3. вода: где поверхность ниже уровня воды ---
    let mut water_levels = vec![None; count];
    let mut has_water = false;
    for idx in 0..count {
        if let Some(w) = candidates[idx] {
            if heights[idx] < w {
                water_levels[idx] = Some(w);
                has_water = true;
            }
        }
    }

    // --- 4. облака точек на той же сетке ---
    let mut ground = PointCloud::new(PointClass::Ground);
    let mut water = PointCloud::new(PointClass::Water);
    for idx in 0..count {
        let p = points[idx];
        match water_levels[idx] {
            Some(w) => water.push(Vec3::new(p.x, p.y, w)),
            None => ground.push(Vec3::new(p.x, p.y, heights[idx])),
        }
    }

    // Стенки водопадов/порогов: между соседними узлами с резким перепадом
    // уровня воды добавляется вертикальный «столбик» точек.
    add_waterfall_walls(&mut water, &water_levels, n, n, step);
    // Гарантированные сплошные «занавесы» на вертикальных обрывах профиля
    // русла (водопады/каскады) — поперёк всей ширины дна.
    let falls_walls = channel_falls_curtains(config);
    add_channel_falls_walls(&mut water, &falls_walls, &heights, n, step, size_m);

    // Вертикальные/почти вертикальные грани рельефа (уступы, обрывы каньона):
    // точечные узлы на горизонтальной сетке там «размазываются» в полосы.
    // Добавляем промежуточные точки вдоль крутых рёбер, чтобы облако на
    // стенах было плотной сеткой, а не редкими полосами.
    densify_ground_faces(&mut ground, &heights, &water_levels, n, n, step);

    // Лёгкий детерминированный шум, чтобы сетка не «светилась».
    jitter_cloud(&mut ground, seed.with_label("грунт"), config.point_jitter);
    jitter_cloud(&mut water, seed.with_label("вода"), config.point_jitter);

    let heightfield = Heightfield {
        size_m,
        step,
        n,
        values: heights,
    };
    let (min_height, max_height) = heightfield.min_max();

    let water_field = WaterField {
        levels: water_levels,
    };

    Landscape {
        config: config.clone(),
        seed,
        heightfield,
        water: water_field,
        ground_cloud: ground,
        water_cloud: water,
        water_walls: falls_walls,
        min_height,
        max_height,
        has_water,
    }
}

/// Вертикальные «стенки» воды в местах резкого перепада её уровня (водопады,
/// пороги наклонных рек). Облако получает промежуточные точки между двумя
/// соседними узлами, высота которых отличается больше чем на
/// [`WATERFALL_JUMP`].
fn add_waterfall_walls(
    cloud: &mut PointCloud,
    levels: &[Option<f32>],
    n: usize,
    m: usize,
    step: f32,
) {
    let half_x = (n as f32 - 1.0) * 0.5 * step;
    let half_y = (m as f32 - 1.0) * 0.5 * step;
    let x_at = |i: usize| -half_x + i as f32 * step;
    let y_at = |j: usize| -half_y + j as f32 * step;

    for j in 0..m {
        for i in 0..n {
            let idx = j * n + i;
            let (w1, p1) = match levels[idx] {
                Some(w) if i + 1 < n => (w, Vec3::new(x_at(i), y_at(j), w)),
                _ => continue,
            };
            // Правое ребро.
            let ni = i + 1;
            let nidx = j * n + ni;
            if let Some(w2) = levels[nidx] {
                if (w2 - w1).abs() > WATERFALL_JUMP {
                    let p2 = Vec3::new(x_at(ni), y_at(j), w2);
                    add_vertical_points(cloud, p1, p2, step);
                }
            }
            // Верхнее ребро.
            if j + 1 >= m {
                continue;
            }
            let nj = j + 1;
            let nidx = nj * n + i;
            if let Some(w2) = levels[nidx] {
                if (w2 - w1).abs() > WATERFALL_JUMP {
                    let p2 = Vec3::new(x_at(i), y_at(nj), w2);
                    add_vertical_points(cloud, p1, p2, step);
                }
            }
        }
    }
}

/// Промежуточные точки между двумя точками поверхности воды (вертикальный
/// столбик на стенке водопада), шаг по высоте ~ `step`.
fn add_vertical_points(cloud: &mut PointCloud, a: Vec3, b: Vec3, step: f32) {
    let delta = b - a;
    let dist = delta.length();
    let count = ((dist / step).ceil() as usize).max(1);
    for k in 1..count {
        let f = k as f32 / count as f32;
        cloud.push(a + delta * f);
    }
}

/// Гарантированный сплошной «занавес» воды на вертикальных обрывах
/// продольного профиля русла (водопады/каскады).
///
/// Обычные «стенки» ([`add_waterfall_walls`]) заводятся по соседним мокрым
/// узлам на сетке; у изгибов русла у каскада две части русла могут не
/// соприкасаться на узлах, и стена получается редкой. Здесь обрыв находится
/// прямо в профиле `Longitudinal::Nodes` (две точки почти с одинаковым `t`,
/// но с большим перепадом высоты) и по нему строится вертикальный занавес
/// воды поперёк русла на всю ширину дна.
#[derive(Debug, Clone, Copy)]
pub struct FallsCurtain {
    /// Точка на оси русла у обрыва (плоскость XY карты).
    pub p: Vec2,
    /// Единичная нормаль к руслу (поперёк, в плоскости XY).
    pub n: Vec2,
    /// Единичный вектор потока (вниз по течению, в плоскости XY).
    pub flow: Vec2,
    /// Полуширина занавеса = полуширина дна, м.
    pub hw: f32,
    /// Нижний уровень воды у обрыва, м.
    pub lo: f32,
    /// Верхний уровень воды у обрыва, м.
    pub hi: f32,
}

/// Все вертикальные обрывы воды (водопады/каскады) в конфигурации как
/// сплошные «занавесы» поперёк русла. Используется и облаком точек, и мешем
/// воды — чтобы вертикальная стена воды была и в облаке, и в визуальной
/// модели.
pub fn channel_falls_curtains(config: &LandscapeConfig) -> Vec<FallsCurtain> {
    let mut out = Vec::new();
    for feature in &config.features {
        let TerrainFeature::Channel(channel) = feature else {
            continue;
        };
        if !channel.wet {
            continue;
        }
        let Longitudinal::Nodes(nodes) = &channel.water else {
            continue;
        };
        for pair in nodes.windows(2) {
            let a = pair[0];
            let b = pair[1];
            let dt = (b.x - a.x).abs();
            let dy = (b.y - a.y).abs();
            // Только настоящие вертикальные обрывы (t почти одинаковый).
            if dt > 0.05 || dy < WATERFALL_JUMP * 0.75 {
                continue;
            }
            let (lo, hi) = if a.y <= b.y { (a.y, b.y) } else { (b.y, a.y) };
            let t = a.x;
            let p = channel.curve.point_at(t);
            let flow = channel.curve.tangent_at(t);
            let n = Vec2::new(-flow.y, flow.x);
            let hw = channel.width_at(t);
            out.push(FallsCurtain { p, n, flow, hw, lo, hi });
        }
    }
    out
}

/// Билинейный семплер высоты поля `values` (сетка `n×n`, шаг `step`, размер
/// карты `size_m`) в точке плоскости `(x, y)` (центр — в начале координат).
fn sample_height(values: &[f32], n: usize, step: f32, size_m: f32, x: f32, y: f32) -> f32 {
    let half = size_m * 0.5;
    let fx = ((x + half) / step).clamp(0.0, n as f32 - 1.0);
    let fy = ((y + half) / step).clamp(0.0, n as f32 - 1.0);
    let i0 = (fx.floor() as usize).min(n - 1);
    let j0 = (fy.floor() as usize).min(n - 1);
    let i1 = (i0 + 1).min(n - 1);
    let j1 = (j0 + 1).min(n - 1);
    let tx = (fx - i0 as f32).clamp(0.0, 1.0);
    let ty = (fy - j0 as f32).clamp(0.0, 1.0);
    let v00 = values[j0 * n + i0];
    let v10 = values[j0 * n + i1];
    let v01 = values[j1 * n + i0];
    let v11 = values[j1 * n + i1];
    let a = v00 + (v10 - v00) * tx;
    let b = v01 + (v11 - v01) * tx;
    a + (b - a) * ty
}

/// Насколько продвинуть «занавес» по потоку (в метрах, вдоль `c.flow`), чтобы
/// дно под НИМ ушло ниже нижнего уровня воды `lo` по всей ширине русла —
/// тогда низ вертикальной стены не упирается в наклонный грунт и не прячется
/// за ним. Считаем по нескольким колонкам поперёк русла и берём максимум:
/// уступ может идти под углом, и по краям дно уходит позже, чем по центру.
/// Колонки, которые не расчищаются за [`MAX_CLEAR`] (берега, склоны), не
/// считаем — их стена всё равно не должна покрывать.
const MAX_CLEAR: f32 = 2.5;

pub fn falls_clear_offset<F>(c: &FallsCurtain, terrain: &F) -> f32
where
    F: Fn(f32, f32) -> f32,
{
    let mut required = 0.0f32;
    let mut off = -c.hw;
    while off <= c.hw + 1e-3 {
        let mut s = 0.0;
        let mut found = -1.0f32;
        while s <= MAX_CLEAR {
            let x = c.p.x + c.n.x * off + c.flow.x * s;
            let y = c.p.y + c.n.y * off + c.flow.y * s;
            if terrain(x, y) <= c.lo - 0.2 {
                found = s;
                break;
            }
            s += 0.25;
        }
        if found >= 0.0 {
            required = required.max(found);
        }
        off += 1.0;
    }
    required
}

/// Добавить «занавесы» каскадов в облако воды: одиночный вертикальный занавес
/// поперёк русла, выдвинутый по потоку настолько ([`falls_clear_offset`]),
/// чтобы его низ (`lo`) не оказывался за грунтом. Для каждой колонки поперёк
/// русла, под которой дно ушло ниже `lo`, — вертикальный столбик точек от
/// `lo` до `hi` с шагом `step`.
fn add_channel_falls_walls(
    cloud: &mut PointCloud,
    curtains: &[FallsCurtain],
    heights: &[f32],
    n: usize,
    step: f32,
    size_m: f32,
) {
    if curtains.is_empty() {
        return;
    }
    let terrain = |x: f32, y: f32| sample_height(heights, n, step, size_m, x, y);
    for c in curtains {
        let s_clear = falls_clear_offset(c, &terrain);
        let mut off = -c.hw;
        while off <= c.hw + 1e-3 {
            let xy = c.p + c.n * off + c.flow * s_clear;
            // Колонка участвует, только если дно под ней уже ушло ниже lo.
            if terrain(xy.x, xy.y) < c.lo - 0.2 {
                let low = Vec3::new(xy.x, xy.y, c.lo);
                let high = Vec3::new(xy.x, xy.y, c.hi);
                add_vertical_points(cloud, low, high, step);
            }
            off += step;
        }
    }
}

/// Уплотнение облака грунта на крутых гранях рельефа.
///
/// На горизонтальной сетке (тот же шаг, что и на равнине) у почти
/// вертикальной стенки узлов, попадающих на её XY-проекцию, почти нет —
/// облако на обрыве превращается в редкие полосы или вовсе пропадает.
/// Чтобы стены были покрыты плотной сеткой, между соседними узлами
/// добавляем промежуточные точки вдоль ребра с шагом `step` по поверхности.
///
/// Правила:
///
/// - «сухое–сухое» ребро уплотняется как обычно (`dist > step`);
/// - «сухое–мокрое» ребро уплотняется ТОЛЬКО по надводной части стенки:
///   от сухой вершины вниз до уреза, чтобы береговые склоны тоже были
///   в облаке (под водой точек грунта не добавляем);
/// - подводный узел источником не бывает.
fn densify_ground_faces(
    cloud: &mut PointCloud,
    heights: &[f32],
    levels: &[Option<f32>],
    n: usize,
    m: usize,
    step: f32,
) {
    let half_x = (n as f32 - 1.0) * 0.5 * step;
    let half_y = (m as f32 - 1.0) * 0.5 * step;
    let x_at = |i: usize| -half_x + i as f32 * step;
    let y_at = |j: usize| -half_y + j as f32 * step;

    for j in 0..m {
        for i in 0..n {
            let idx = j * n + i;
            if levels[idx].is_some() {
                continue;
            }
            let p1 = Vec3::new(x_at(i), y_at(j), heights[idx]);
            // Правое ребро.
            if i + 1 < n {
                let nidx = j * n + i + 1;
                match levels[nidx] {
                    None => {
                        let p2 = Vec3::new(x_at(i + 1), y_at(j), heights[nidx]);
                        densify_edge(cloud, p1, p2, step);
                    }
                    Some(w) if w < heights[idx] => {
                        // Надводная часть стенки к соседнему подводному узлу.
                        let p2 = Vec3::new(x_at(i + 1), y_at(j), w);
                        densify_edge(cloud, p1, p2, step);
                    }
                    Some(_) => {}
                }
            }
            // Верхнее ребро.
            if j + 1 < m {
                let nidx = (j + 1) * n + i;
                match levels[nidx] {
                    None => {
                        let p2 = Vec3::new(x_at(i), y_at(j + 1), heights[nidx]);
                        densify_edge(cloud, p1, p2, step);
                    }
                    Some(w) if w < heights[idx] => {
                        // Надводная часть стенки к соседнему подводному узлу.
                        let p2 = Vec3::new(x_at(i), y_at(j + 1), w);
                        densify_edge(cloud, p1, p2, step);
                    }
                    Some(_) => {}
                }
            }
        }
    }
}

/// Промежуточные точки вдоль ребра `a→b`, чтобы шаг ВДОЛЬ поверхности не
/// превышал шаг сетки (на равнине `dist == step`, тут — всегда ≥).
fn densify_edge(cloud: &mut PointCloud, a: Vec3, b: Vec3, step: f32) {
    let delta = b - a;
    let dist = delta.length();
    let intervals = (dist / step).ceil() as usize;
    if intervals <= 1 {
        return;
    }
    for k in 1..intervals {
        let f = k as f32 / intervals as f32;
        cloud.push(a + delta * f);
    }
}

/// Мелкий детерминированный шум точек, чтобы сетка не «светилась».
fn jitter_cloud(cloud: &mut PointCloud, seed: Seed, amplitude: f32) {
    if amplitude <= 0.0 {
        return;
    }
    let mut rng = seed.rng();
    for p in &mut cloud.points {
        let d = Vec3::new(
            rng.random_range(-amplitude..amplitude),
            rng.random_range(-amplitude..amplitude),
            rng.random_range(-amplitude..amplitude),
        );
        p.position += d;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::preset::{LandscapePreset, resolve};

    #[test]
    fn deterministic_generation() {
        for preset in LandscapePreset::ALL {
            for seed in 0..2u64 {
                let a = generate(&resolve(preset, Seed::new(seed)));
                let b = generate(&resolve(preset, Seed::new(seed)));
                assert_eq!(a.heightfield.values, b.heightfield.values, "{preset:?}");
                assert_eq!(a.water.levels, b.water.levels, "{preset:?}");
            }
        }
    }

    #[test]
    fn no_ground_under_water() {
        for preset in LandscapePreset::ALL {
            let land = generate(&resolve(preset, Seed::new(1234)));
            let n = land.heightfield.n;
            let step = land.heightfield.step;
            let half = land.heightfield.size_m * 0.5;
            // Поправка на детерминированный джиттер точек (±point_jitter).
            let tol = land.config.point_jitter + 1e-2;
            // Инвариант: ни одна точка грунта не стоит НИЖЕ местного уровня
            // воды. Точки береговых стенок над урезом (в подводном столбце,
            // но выше воды) допустимы.
            for pt in &land.ground_cloud.points {
                let i =
                    ((((pt.position.x + half) / step).round() as isize).clamp(0, n as isize - 1))
                        as usize;
                let j =
                    ((((pt.position.y + half) / step).round() as isize).clamp(0, n as isize - 1))
                        as usize;
                if let Some(w) = land.water.levels[j * n + i] {
                    assert!(
                        pt.position.z >= w - tol,
                        "{preset:?}: точка грунта под водой (z={} < w={w})",
                        pt.position.z
                    );
                }
            }
        }
    }

    #[test]
    fn plain_has_no_water() {
        let l = generate(&resolve(LandscapePreset::Plain, Seed::new(1)));
        assert!(!l.has_water);
        assert!(l.water_cloud.is_empty());
    }

    #[test]
    fn pond_has_water_and_land() {
        let l = generate(&resolve(LandscapePreset::Pond, Seed::new(2)));
        assert!(l.has_water);
        assert!(!l.water_cloud.is_empty());
        assert!(!l.ground_cloud.is_empty());
    }

    #[test]
    fn river_bank_walls_have_ground_points() {
        // Берега рек/водопадных русел — сухие склоны (вода только на дне),
        // поэтому на надводных берегах лежит облако точек грунта, а не
        // «пустая» залитая кромка.
        for preset in [LandscapePreset::River, LandscapePreset::Waterfalls] {
            let mut min_bank_points = usize::MAX;
            for seed in 0..8u64 {
                let land = generate(&resolve(preset, Seed::new(seed)));
                let n = land.heightfield.n;
                let step = land.heightfield.step;
                let half = land.heightfield.size_m * 0.5;
                let wet: Vec<bool> = land.water.levels.iter().map(|o| o.is_some()).collect();
                let is_wet = |idx: usize| wet[idx];
                let mut bank_points = 0usize;
                for pt in &land.ground_cloud.points {
                    let i = ((((pt.position.x + half) / step).round() as isize)
                        .clamp(0, n as isize - 1)) as usize;
                    let j = ((((pt.position.y + half) / step).round() as isize)
                        .clamp(0, n as isize - 1)) as usize;
                    let idx = j * n + i;
                    if wet[idx] {
                        continue;
                    }
                    // Точка на сухом береговом узле — рядом с водой.
                    let near_water = (i > 0 && is_wet(idx - 1))
                        || (i + 1 < n && is_wet(idx + 1))
                        || (j > 0 && is_wet(idx - n))
                        || (j + 1 < n && is_wet(idx + n));
                    if near_water {
                        bank_points += 1;
                    }
                }
                min_bank_points = min_bank_points.min(bank_points);
            }
            assert!(
                min_bank_points > 40,
                "{preset:?}: слишком мало точек на надводных берегах: {min_bank_points}"
            );
        }
    }

    #[test]
    fn large_pond_has_shallows_and_no_shore_cliff() {
        for seed in 0..8 {
            let land = generate(&resolve(LandscapePreset::Pond, Seed::new(seed)));
            let hf = &land.heightfield;
            let area = land.water.levels.iter().flatten().count() as f32 * hf.step.powi(2);
            assert!(
                area > 1500.0 && area < 5000.0,
                "seed {seed}: площадь {area}"
            );
            let mut shallow = 0;
            let mut shore_edges = 0;
            for (idx, water) in land.water.levels.iter().enumerate() {
                let Some(w) = water else { continue };
                if w - hf.values[idx] < 0.45 {
                    shallow += 1;
                }
                let i = idx % hf.n;
                let j = idx / hf.n;
                for (ni, nj) in [
                    (i + 1, j),
                    (i.wrapping_sub(1), j),
                    (i, j + 1),
                    (i, j.wrapping_sub(1)),
                ] {
                    if ni < hf.n && nj < hf.n {
                        let next = hf.index(ni, nj);
                        if land.water.levels[next].is_none() {
                            shore_edges += 1;
                            assert!(hf.values[next] - w < 0.3, "seed {seed}: обрыв у берега");
                        }
                    }
                }
            }
            assert!(
                shallow > 40 && shore_edges > 100,
                "seed {seed}: нет пологого берега"
            );
        }
    }

    #[test]
    fn waterfalls_have_curtains_but_rivers_do_not() {
        // «Занавесы» каскадов заводятся только там, где у русла есть явные
        // вертикальные обрывы в продольном профиле (водопады).
        for seed in 0..12u64 {
            let land = generate(&resolve(LandscapePreset::Waterfalls, Seed::new(seed)));
            assert!(
                !land.water_walls.is_empty(),
                "seed {seed}: у водопадов должны быть «занавесы» воды"
            );
            for c in &land.water_walls {
                assert!(c.hi > c.lo, "высота занавеса: {c:?}");
                assert!(c.hw > 0.0 && c.n.length_squared() > 0.5, "геометрия: {c:?}");
            }
        }
        for preset in [LandscapePreset::River, LandscapePreset::Gorge, LandscapePreset::SlopingRiver] {
            let land = generate(&resolve(preset, Seed::new(3)));
            assert!(
                land.water_walls.is_empty(),
                "{preset:?}: у плавной воды не должно быть «занавесов»"
            );
        }
    }
}
