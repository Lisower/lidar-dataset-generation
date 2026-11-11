//! Трава: пучки травинок (объёмная визуальная модель + облако точек).
//!
//! Пучок — набор тонких вытянутых четырёхугольников-травинок, расходящихся
//! из одного основания и клонящихся от центра. Облако точек даёт не каждая
//! травинка, а весь пучок целиком — несколько точек, описывающих его объём.
//! Так на поле из тысяч пучков облако остаётся «читабельным» для лидара,
//! а не превращается в сплошную массу.

use bevy::math::{Vec2, Vec3};
use rand::RngExt;

use generation_core::geometry::Polygon;
use generation_core::{PointCloud, Seed};

use super::config::{PlantConfig, PlantPreset};
use super::part::Part;

/// Один пучок травы (одиночный объект или элемент поля).
#[derive(Debug, Clone)]
pub struct GrassTuft {
    /// Часть-пучок: полигоны травинок + облако точек.
    pub part: Part,
    /// Полное облако точек пучка.
    pub point_cloud: PointCloud,
    /// Высота пучка, м.
    pub height: f32,
    /// Радиус пучка, м.
    pub radius: f32,
    /// Число травинок.
    pub blade_count: usize,
}

/// Пучок, расставленный на поле вокруг центра.
#[derive(Debug, Clone)]
pub struct PlacedTuft {
    /// Положение центра пучка на карте (ось Z — вверх).
    pub position: Vec2,
    /// Поворот пучка вокруг вертикали, радианы.
    pub rotation: f32,
    /// Сам пучок.
    pub tuft: GrassTuft,
}

/// Сгенерировать пучок травы по конфигурации (детерминированно от `seed`).
pub fn generate_tuft(config: &PlantConfig, seed: Seed) -> GrassTuft {
    if config.preset == PlantPreset::Wheat {
        return generate_wheat_tuft(config, seed);
    }
    let mut rng = seed.with_label("tuft").rng();
    let g = &config.grass;

    let blade_count = rng.random_range(g.blades_min..=g.blades_max);
    let tuft_height = rng.random_range(g.tuft_height_min..=g.tuft_height_max);

    let mut faces: Vec<Polygon> = Vec::with_capacity(blade_count);
    let half_w = g.blade_width * 0.5;

    // Плотная сердцевина + редкая окраина: распределение по радиусу ближе к центру.
    for i in 0..blade_count {
        let angle = std::f32::consts::TAU * i as f32 / blade_count as f32
            + rng.random_range(-0.4..0.4);
        let dist =
            rng.random_range(0.0f32..1.0).powf(1.6) * g.tuft_radius.max(0.01);
        let base = Vec3::new(angle.cos() * dist, angle.sin() * dist, 0.0);

        // Индивидуальная высота и наклон травинки.
        let h = tuft_height * rng.random_range(0.55..1.05);
        let lean_ang = rng.random_range(-0.6..0.6) + angle;
        let lean = rng.random_range(0.0..1.0) * g.blade_bend;
        let tip = base + Vec3::new(lean_ang.cos(), lean_ang.sin(), 0.0) * (lean * h * 0.55);
        let tip = tip + Vec3::Z * h;

        // Травинка — плоский четырёхугольник (ширина у основания, к кончику уже).
        let perp = Vec3::new(-angle.sin(), angle.cos(), 0.0);
        let tip_w = half_w * rng.random_range(0.15..0.45);
        let b0 = base + perp * half_w;
        let b1 = base - perp * half_w;
        let t0 = tip + perp * tip_w;
        let t1 = tip - perp * tip_w;
        faces.push(Polygon::quad(b0, b1, t1, t0).oriented(Vec3::Z));
    }

    // Облако точек: весь пучок, а не каждая травинка. Несколько точек в объёме
    // пучка — плотнее к основанию и к центру, чтобы лидар видел «кустик».
    let pts = g.tuft_cloud_points.clamp(4, 64);
    let mut cloud: Vec<Vec3> = Vec::with_capacity(pts);
    for _ in 0..pts {
        let a = rng.random_range(0.0..std::f32::consts::TAU);
        let dist = rng.random_range(0.0f32..1.0).powf(1.5) * g.tuft_radius.max(0.01);
        let h = tuft_height * rng.random_range(0.15..1.0);
        cloud.push(Vec3::new(a.cos() * dist, a.sin() * dist, h));
    }

    let part = Part::new("пучок травы", g.color, faces, true, cloud);
    let point_cloud = part.collect_cloud();

    GrassTuft {
        part,
        point_cloud,
        height: tuft_height,
        radius: g.tuft_radius,
        blade_count,
    }
}

/// Сгенерировать пучок пшеницы: тонкие стебли с колосьями.
fn generate_wheat_tuft(config: &PlantConfig, seed: Seed) -> GrassTuft {
    let mut rng = seed.with_label("wheat_tuft").rng();
    let g = &config.grass;

    let blade_count = rng.random_range(g.blades_min..=g.blades_max);
    let tuft_height = rng.random_range(g.tuft_height_min..=g.tuft_height_max);

    let mut faces: Vec<Polygon> = Vec::with_capacity(blade_count * 10);

    for i in 0..blade_count {
        let angle = std::f32::consts::TAU * i as f32 / blade_count as f32
            + rng.random_range(-0.35..0.35);

        let dist =
            rng.random_range(0.0f32..1.0).sqrt() * g.tuft_radius.max(0.01);

        let base = Vec3::new(
            angle.cos() * dist,
            angle.sin() * dist,
            0.0,
        );

        let h = tuft_height * rng.random_range(0.82..1.05);

        let lean_angle = angle + rng.random_range(-0.35..0.35);
        let lean = rng.random_range(0.0..1.0) * g.blade_bend;

        let top = base
            + Vec3::new(lean_angle.cos(), lean_angle.sin(), 0.0)
                * (lean * h * 0.25)
            + Vec3::Z * h;

        // Тонкий вертикальный стебель.
        let stem_half_width = g.blade_width * 0.5;
        let perp = Vec3::new(-lean_angle.sin(), lean_angle.cos(), 0.0);

        let s0 = base + perp * stem_half_width;
        let s1 = base - perp * stem_half_width;
        let s2 = top - perp * stem_half_width * 0.65;
        let s3 = top + perp * stem_half_width * 0.65;

        faces.push(
            Polygon::quad(s0, s1, s2, s3)
                .oriented(Vec3::Z),
        );

        // Два узких листа на стебле.
        for leaf_index in 0..2 {
            let leaf_height = if leaf_index == 0 { 0.38 } else { 0.58 };
            let leaf_side = if leaf_index == 0 { 1.0 } else { -1.0 };

            let leaf_base = base
                + (top - base) * leaf_height;

            let leaf_length = h * rng.random_range(0.10..0.16);
            let leaf_width = g.blade_width * 1.5;

            let leaf_tip = leaf_base
                + perp * (leaf_side * leaf_length * 0.45)
                + Vec3::Z * (leaf_length * 0.55);

            let l0 = leaf_base - perp * leaf_width;
            let l1 = leaf_base + perp * leaf_width;
            let l2 = leaf_tip;

            faces.push(
                Polygon::triangle(l0, l1, l2)
                    .oriented(Vec3::Z),
            );
        }

        // Колос: отдельные колоски располагаются попеременно
        // по сторонам стебля и сужаются к верхушке.
        let ear_height = h * rng.random_range(0.14..0.18);
        let spikelets = 8usize;

        for segment in 0..spikelets {
            let t = segment as f32 / (spikelets - 1) as f32;

            let center = top + Vec3::Z * (ear_height * t);

            // Нижняя и верхняя части колоса немного уже середины.
            let shape = (std::f32::consts::PI * t).sin();
            let spikelet_length =
                g.blade_width * (1.3 + shape * 1.0);

            let spikelet_height = ear_height * 0.16;

            // Чередуем колоски слева и справа.
            let side = if segment % 2 == 0 { 1.0 } else { -1.0 };

            let inner = center - perp * (spikelet_length * 0.15 * side);
            let outer = center + perp * (spikelet_length * side);

            let p0 = inner - Vec3::Z * (spikelet_height * 0.35);
            let p1 = outer;
            let p2 = inner + Vec3::Z * spikelet_height;

            faces.push(
                Polygon::triangle(p0, p1, p2)
                    .oriented(Vec3::Z),
            );
            // Тонкая ость, выходящая из колоска вверх и наружу.
            let awn_start = outer;
            let awn_end = outer
                + perp * (side * g.blade_width * 0.8)
                + Vec3::Z * (ear_height * 0.32);

            let awn_half_width = g.blade_width * 0.10;

            let a0 = awn_start - perp * awn_half_width;
            let a1 = awn_start + perp * awn_half_width;
            let a2 = awn_end + perp * awn_half_width * 0.25;
            let a3 = awn_end - perp * awn_half_width * 0.25;

            faces.push(
                Polygon::quad(a0, a1, a2, a3)
                    .oriented(Vec3::Z),
            );
        }
    }

    let pts = g.tuft_cloud_points.clamp(4, 64);
    let mut cloud: Vec<Vec3> = Vec::with_capacity(pts);

    for _ in 0..pts {
        let angle = rng.random_range(0.0..std::f32::consts::TAU);
        let dist =
            rng.random_range(0.0f32..1.0).sqrt() * g.tuft_radius.max(0.01);

        let height = tuft_height * rng.random_range(0.2..1.15);

        cloud.push(Vec3::new(
            angle.cos() * dist,
            angle.sin() * dist,
            height,
        ));
    }

    let part = Part::new(
        "пшеница",
        g.color,
        faces,
        true,
        cloud,
    );

    let point_cloud = part.collect_cloud();

    GrassTuft {
        part,
        point_cloud,
        height: tuft_height * 1.15,
        radius: g.tuft_radius,
        blade_count,
    }
}

/// Сгенерировать поле пучков травы вокруг центра (детерминированно от `seed`).
///
/// Пучки расставляются в круге радиуса [`GrassConfig::field_radius`], не
/// перекрывая друг друга. Проверка на пересечение идёт через равномерную
/// сетку, поэтому тысяча пучков расставляется быстро (O(n·опытов), не O(n²)).
pub fn generate_field(config: &PlantConfig, seed: Seed) -> Vec<PlacedTuft> {
    use std::collections::HashMap;
    use std::f32::consts::TAU;

    let mut rng = seed.with_label("field").rng();
    let g = &config.grass;
    let count = rng.random_range(g.tufts_min..=g.tufts_max);

    let mut tufts: Vec<PlacedTuft> = Vec::with_capacity(count);
    // Минимальный зазор между центрами пучков.
    let min_gap = (g.tuft_radius.max(0.1) * 0.75) + g.tuft_radius * 0.5;
    // Размер ячейки сетки: два центра, оказавшиеся ближе `min_gap`, гарантированно
    // лежат в соседних (±1) ячейках — хватает окрестности 3×3.
    let cell = min_gap;

    let cell_of = |p: Vec2| -> (i32, i32) {
        let x = (p.x.div_euclid(cell)) as i32;
        let y = (p.y.div_euclid(cell)) as i32;
        (x, y)
    };
    let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::with_capacity(count);

    for i in 0..count {
        for _attempt in 0..24 {
            let angle = rng.random_range(0.0..TAU);
            let dist = rng.random_range(0.0f32..1.0).sqrt() * g.field_radius;
            let position = Vec2::new(angle.cos() * dist, angle.sin() * dist);

            // Не наезжаем на уже поставленные пучки (только в соседних ячейках).
            let (cx, cy) = cell_of(position);
            let mut ok = true;
            'buckets: for by in (cy - 1)..=(cy + 1) {
                for bx in (cx - 1)..=(cx + 1) {
                    if let Some(indices) = grid.get(&(bx, by)) {
                        for &idx in indices {
                            if position.distance(tufts[idx].position) < min_gap {
                                ok = false;
                                break 'buckets;
                            }
                        }
                    }
                }
            }
            if !ok {
                continue;
            }

            let tuft = generate_tuft(config, seed.child(i as u64 + 1));
            let idx = tufts.len();
            tufts.push(PlacedTuft {
                position,
                rotation: rng.random_range(0.0..TAU),
                tuft,
            });
            grid.entry(cell_of(position)).or_default().push(idx);
            break;
        }
    }
    tufts
}
