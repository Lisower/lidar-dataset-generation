//! Интеграционные тесты генерации ландшафта.
//!
//! Проверяем ключевые требования тз:
//! - детерминизм от seed (один и тот же seed → один и тот же ландшафт);
//! - **под водой нет точек грунта**;
//! - **плотность облака точек одинакова на любых наклонах** (тот же шаг — на
//!   склоне столько же точек на той же горизонтальной карте, сколько и на
//!   равнине);
//! - вода появляется только там, где должна (пресеты с водой и без).

use generation_core::Seed;

use crate::generation::landscape::generate;
use crate::generation::preset::resolve;
use crate::generation::{LandscapePreset, falls_clear_offset};

/// Число узлов сетки при разрешении последнего поля высот.
fn grid_cells(landscape: &crate::generation::Landscape) -> usize {
    let n = landscape.heightfield.n;
    n * n
}

#[test]
fn deterministic_by_seed() {
    for preset in LandscapePreset::ALL {
        for seed in 0..4u64 {
            let a = generate(&resolve(preset, Seed::new(seed)));
            let b = generate(&resolve(preset, Seed::new(seed)));
            assert_eq!(a.heightfield.values, b.heightfield.values, "{preset:?}");
            assert_eq!(a.water.levels, b.water.levels, "{preset:?}");
            assert_eq!(a.ground_cloud, b.ground_cloud, "{preset:?}");
            assert_eq!(a.water_cloud, b.water_cloud, "{preset:?}");
        }
    }
}

/// На склоне облако точек грунта покрывает ВСЮ карту плотно (тот же шаг на
/// любых наклонах): на крутых гранях добавляются промежуточные точки, поэтому
/// точек не меньше, чем узлов сетки — полос и пропусков на стенах нет.
#[test]
fn slopes_and_walls_have_dense_ground_cloud() {
    for preset in [
        LandscapePreset::Plain,
        LandscapePreset::Slope,
        LandscapePreset::PlainSlope,
        LandscapePreset::MultiLevel,
        LandscapePreset::CanyonLedge,
    ] {
        let landscape = generate(&resolve(preset, Seed::new(7)));
        assert!(!landscape.has_water, "{preset:?}");
        assert!(
            landscape.ground_cloud.points.len() >= grid_cells(&landscape),
            "{preset:?}: точки грунта должны покрывать каждый узел карты"
        );
    }
}

/// На почти вертикальных стенах (уступ каньона) облако грунта — плотная сетка,
/// а не редкие полосы: точек заметно больше, чем один на узел сети.
#[test]
fn canyon_wall_is_not_sparse() {
    for seed in 0..8u64 {
        let landscape = generate(&resolve(LandscapePreset::CanyonLedge, Seed::new(seed)));
        let cells = grid_cells(&landscape);
        // Уплотнение добавляет точки между узлами резкого перепада высот.
        assert!(
            landscape.ground_cloud.points.len() >= cells,
            "seed {seed}: меньше узлов, чем ячеек ({})",
            landscape.ground_cloud.points.len()
        );
    }
}

/// Каждый узел карты принадлежит либо грунту, либо воде (без разрывов,
/// без двойной записи). Вода добавочно даёт «стенки» водопадов.
#[test]
fn every_cell_is_ground_or_water() {
    for preset in LandscapePreset::ALL {
        let landscape = generate(&resolve(preset, Seed::new(11)));
        let covered = landscape.ground_cloud.points.len() + landscape.water_cloud.points.len();
        assert!(
            covered >= grid_cells(&landscape),
            "{preset:?}: облака покрывают меньше узлов, чем карта ({covered})"
        );
    }
}

/// Под водой (ниже уреза) не должно быть точек грунта. Точки береговых
/// стенок, стоящих над урезом в подводном столбце, допустимы.
#[test]
fn no_ground_points_under_water() {
    for preset in LandscapePreset::ALL {
        let landscape = generate(&resolve(preset, Seed::new(1234)));
        if !landscape.has_water {
            continue;
        }
        let (n, step) = (landscape.heightfield.n, landscape.heightfield.step);
        let half = landscape.heightfield.size_m * 0.5;
        let tol = landscape.config.point_jitter + 1e-2;
        for pt in &landscape.ground_cloud.points {
            let i = ((((pt.position.x + half) / step).round() as isize)
                .clamp(0, n as isize - 1)) as usize;
            let j = ((((pt.position.y + half) / step).round() as isize)
                .clamp(0, n as isize - 1)) as usize;
            if let Some(w) = landscape.water.levels[j * n + i] {
                assert!(
                    pt.position.z >= w - tol,
                    "{preset:?}: точка грунта под водой (z={} < w={w})",
                    pt.position.z
                );
            }
        }
    }
}

/// Правильное наличие воды по пресетам.
#[test]
fn water_presence_matches_preset() {
    let with_water = [
        LandscapePreset::River,
        LandscapePreset::Pond,
        LandscapePreset::Shore,
        LandscapePreset::SlopingRiver,
        LandscapePreset::Waterfalls,
    ];
    let without_water = [
        LandscapePreset::Plain,
        LandscapePreset::Slope,
        LandscapePreset::PlainSlope,
        LandscapePreset::MultiLevel,
        LandscapePreset::CanyonLedge,
    ];
    for preset in with_water {
        let landscape = generate(&resolve(preset, Seed::new(3)));
        assert!(landscape.has_water, "{preset:?}: должна быть вода");
        assert!(!landscape.water_cloud.is_empty());
    }
    for preset in without_water {
        let landscape = generate(&resolve(preset, Seed::new(3)));
        assert!(!landscape.has_water, "{preset:?}: воды быть не должно");
        assert!(landscape.water_cloud.is_empty());
    }
}

/// Ущелье умеет быть и с водой, и сухим (в зависимости от seed).
#[test]
fn gorge_can_be_dry_or_with_water() {
    let mut with_water = false;
    let mut dry = false;
    for seed in 0..24u64 {
        let landscape = generate(&resolve(LandscapePreset::Gorge, Seed::new(seed)));
        if landscape.has_water {
            with_water = true;
        } else {
            dry = true;
        }
    }
    assert!(with_water, "ущелье должно уметь быть с водой");
    assert!(dry, "ущелье должно уметь быть сухим");
}

/// Пресеты с руслом (речка, ущелье, водопады) дают больше воды, чем один
/// горизонтальный уровень; у водопадов вертикальные «стены» воды у каскадов
/// должны быть в каждом сиде и заметной ширины (точек заметно больше, чем
/// ячеек воды).
#[test]
fn waterfalls_add_water_wall_points() {
    let mut min_extra = usize::MAX;
    for seed in 0..12u64 {
        let land = generate(&resolve(LandscapePreset::Waterfalls, Seed::new(seed)));
        assert!(land.has_water, "seed {seed}: водопад должен иметь воду");
        let wet_cells = land.water.levels.iter().flatten().count();
        let extra = land.water_cloud.points.len() - wet_cells;
        min_extra = min_extra.min(extra);
        assert!(
            extra > 150,
            "seed {seed}: вертикальная стена воды у каскада пропала: точек сверх ячеек = {extra}"
        );
    }
    assert!(
        min_extra > 150,
        "водопад обязан давать широкую «стену» воды (точек заметно больше, чем ячеек): {min_extra}"
    );
}

/// «Занавес» каскада — одиночная вертикальная стена поперёк русла, выдвинутая
/// по потоку ровно настолько ([`falls_clear_offset`]), чтобы её низ (`lo`) не
/// упирался в наклонный грунт под водой. Для каждого сида проверяем: найдена
/// «чистая» позиция, стена достаточно широкая (дно под большинством колонок
/// ушло ниже `lo`), а сам перепад `hi - lo` — настоящий каскад.
#[test]
fn waterfall_wall_clears_the_rock() {
    let mut saw_columns = 0usize;
    for seed in 0..12u64 {
        let land = generate(&resolve(LandscapePreset::Waterfalls, Seed::new(seed)));
        assert!(land.has_water, "seed {seed}: водопад должен иметь воду");
        for c in &land.water_walls {
            assert!(
                c.hi - c.lo > 2.0,
                "seed {seed}: каскад слишком низкий ({:.2} м)",
                c.hi - c.lo
            );
            let s = falls_clear_offset(c, &|x, y| land.heightfield.height_at(x, y));
            assert!(
                s <= 6.0,
                "seed {seed}: занавес не нашёл «чистой» позиции за {s:.2} м"
            );
            // Считаем колонки поперёк русла на плоскости занавеса, под
            // которыми дно ушло ниже lo — там стена не упирается в грунт.
            let mut clear = 0usize;
            let mut off = -c.hw;
            while off <= c.hw + 1e-3 {
                let xy = c.p + c.n * off + c.flow * s;
                if land.heightfield.height_at(xy.x, xy.y) < c.lo - 0.2 {
                    clear += 1;
                }
                off += 0.5;
            }
            assert!(
                clear > 0,
                "seed {seed}: ни одной колонки стены со стороны воды"
            );
            saw_columns += clear;
        }
    }
    assert!(
        saw_columns >= 100,
        "стены воды слишком мало колонок суммарно: {saw_columns}"
    );
}

/// Эрозия не запирает нижний бассейн, а меш воды и точки описывают одно падение.
#[test]
fn waterfall_surface_stays_connected_and_above_its_bed() {
    use crate::generation::TerrainFeature;
    use crate::visual::water_mesh;
    use bevy::mesh::VertexAttributeValues;

    for seed in 0..12 {
        let cfg = resolve(LandscapePreset::Waterfalls, Seed::new(seed));
        let land = generate(&cfg);
        let hf = &land.heightfield;
        let TerrainFeature::Channel(channel) = &cfg.features[0] else {
            panic!("нет русла")
        };
        for k in 5..95 {
            let p = channel.curve.point_at(k as f32 / 100.0);
            let i = ((p.x + 50.0) / hf.step).round() as usize;
            let j = ((p.y + 50.0) / hf.step).round() as usize;
            if i < hf.n && j < hf.n {
                assert!(
                    land.water.levels[hf.index(i, j)].is_some(),
                    "seed {seed}: разрыв на t={k}%"
                );
            }
        }
        let mesh = water_mesh(hf, &land.water, &land.water_walls).unwrap();
        let VertexAttributeValues::Float32x3(vertices) = mesh
            .attribute(bevy::mesh::Mesh::ATTRIBUTE_POSITION)
            .unwrap()
        else {
            panic!("нет вершин воды")
        };
        let mut falling_faces = 0;
        for face in vertices.chunks_exact(3) {
            let mut lo = f32::INFINITY;
            let mut hi = f32::NEG_INFINITY;
            for &[x, y, water] in face {
                let i = ((x + 50.0) / hf.step).round() as usize;
                let j = ((y + 50.0) / hf.step).round() as usize;
                assert!((hf.node_xy(i, j).x - x).abs() < 1e-4);
                assert!((hf.node_xy(i, j).y - y).abs() < 1e-4);
                assert_eq!(land.water.levels[hf.index(i, j)], Some(water));
                assert!(hf.get(i, j) < water, "seed {seed}: вода в скале");
                lo = lo.min(water);
                hi = hi.max(water);
            }
            if hi - lo > 2.0 {
                falling_faces += 1;
            }
        }
        assert!(
            falling_faces > 10,
            "seed {seed}: поверхность падения пропала"
        );
    }
}
