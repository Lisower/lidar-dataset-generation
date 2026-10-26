//! Unit-тесты генерации зданий.
//!
//! Генерация полностью детерминирована от seed: одни и те же входные данные
//! всегда дают один и тот же результат. Проверяем детерминизм, диапазоны
//! габаритов/этажей, достижимость всех типов крыш и то, что здание стоит на
//! земле (нет подземных точек).

use generation_core::Seed;

use crate::generation::{Building, BuildingConfig, RoofStyle};

#[test]
fn boxed_cloud_is_on_requested_outer_face_for_both_axis_signs() {
    use crate::generation::Part;
    use bevy::prelude::*;
    let cfg = BuildingConfig {
        point_jitter: 0.0,
        ..Default::default()
    };
    let size = Vec3::new(4.0, 0.4, 3.0);
    for outward in [
        Vec3::X,
        Vec3::NEG_X,
        Vec3::Y,
        Vec3::NEG_Y,
        Vec3::Z,
        Vec3::NEG_Z,
    ] {
        let part = Part::boxed(
            "test",
            size,
            outward,
            Color::WHITE,
            Transform::IDENTITY,
            &cfg,
        );
        assert!(!part.cloud.is_empty());
        let expected = outward.abs().dot(size) * 0.5;
        for point in &part.cloud.points {
            assert!(
                (point.position.dot(outward) - expected).abs() < 1e-5,
                "outward={outward:?}, point={:?}, expected={expected}",
                point.position
            );
        }
    }
}

#[test]
fn wall_cloud_is_on_outer_face_for_every_facade() {
    use crate::generation::elements::{Facade, WallCtx, wall_part};
    use bevy::prelude::*;
    let cfg = BuildingConfig {
        point_jitter: 0.0,
        ..Default::default()
    };
    for facade in Facade::ALL {
        let ctx = WallCtx {
            facade,
            center: Vec3::ZERO,
            thickness: cfg.wall_thickness,
        };
        let wall = wall_part(&ctx, 5.0, 3.0, Color::WHITE, &cfg, &[]);
        assert!(!wall.cloud.is_empty());
        for point in &wall.cloud.points {
            assert!(
                (point.position.dot(facade.outward()) - cfg.wall_thickness * 0.5).abs() < 1e-5,
                "facade={facade:?}, point={:?}",
                point.position
            );
        }
    }
}

/// Конфигурация с явным seed.
fn config(seed: u64) -> BuildingConfig {
    BuildingConfig {
        seed: Seed::new(seed),
        ..Default::default()
    }
}

#[test]
fn deterministic_by_seed() {
    let a = Building::generate(&config(42));
    let b = Building::generate(&config(42));
    assert_eq!(a.size, b.size);
    assert_eq!(a.floors, b.floors);
    assert_eq!(a.roof, b.roof);
    assert_eq!(a.point_cloud, b.point_cloud);
}

#[test]
fn different_seeds_differ() {
    let a = Building::generate(&config(1));
    let b = Building::generate(&config(2));
    assert_ne!(
        a.point_cloud, b.point_cloud,
        "разные seed дали одинаковое здание"
    );
}

#[test]
fn sizes_in_range() {
    let cfg = &config(7);
    for seed in 0..40u64 {
        let mut c = cfg.clone();
        c.seed = Seed::new(seed);
        let b = Building::generate(&c);
        for s in [b.size.x, b.size.y] {
            assert!(
                s >= cfg.min_size - 1e-3 && s <= cfg.max_size + 1e-3,
                "сторона {s} вне [{}, {}]",
                cfg.min_size,
                cfg.max_size
            );
        }
        let (long, short) = if b.size.x >= b.size.y {
            (b.size.x, b.size.y)
        } else {
            (b.size.y, b.size.x)
        };
        assert!(
            long <= short * cfg.max_ratio + 1e-3,
            "отношение сторон больше max_ratio"
        );
    }
}

#[test]
fn floors_in_range() {
    for seed in 0..40u64 {
        let b = Building::generate(&config(seed));
        assert!(
            (config(seed).min_floors..=config(seed).max_floors).contains(&b.floors),
            "этажей {}",
            b.floors
        );
    }
}

#[test]
fn all_roof_types_reachable() {
    let mut seen = std::collections::HashSet::new();
    for seed in 0..400u64 {
        let b = Building::generate(&config(seed));
        seen.insert(b.roof);
        if seen.len() == RoofStyle::ALL.len() {
            break;
        }
    }
    assert_eq!(
        seen.len(),
        RoofStyle::ALL.len(),
        "не все типы крыш достижимы: {seen:?}"
    );
}

#[test]
fn base_is_on_ground() {
    // Нижняя точка облака — на уровне земли (z = 0), ничего под землёй.
    for seed in 0..40u64 {
        let b = Building::generate(&config(seed));
        let min_z = b
            .point_cloud
            .points
            .iter()
            .map(|p| p.position.z)
            .fold(f32::INFINITY, f32::min);
        assert!(
            min_z >= -0.02,
            "seed {seed}: нижняя точка {min_z} ниже земли"
        );
    }
}

#[test]
fn point_cloud_not_empty() {
    for seed in 0..20u64 {
        let b = Building::generate(&config(seed));
        assert!(!b.point_cloud.points.is_empty());
    }
}

#[test]
fn total_height_exceeds_walls() {
    // `total_height` (с крышей) должен быть больше высоты стен.
    for seed in 0..30u64 {
        let b = Building::generate(&config(seed));
        assert!(b.total_height() > b.size.z);
    }
}

#[test]
fn roof_geometry_valid() {
    // Каждый полигон крыши — невырожденный (ненулевая нормаль, все точки в
    // одной плоскости) для всех типов и крайних значений наклона и сдвига
    // гребня.
    use crate::generation::roof::roof_faces;
    for style in RoofStyle::ALL {
        for &pitch in &[5.0, 14.0, 27.0, 40.0] {
            for &(len, wid) in &[(12.0, 8.0), (8.0, 8.0), (5.0, 4.0)] {
                let cfg = config(3);
                let wy = wid * 0.5 + 0.4;
                let ridge = if matches!(style, RoofStyle::Gable | RoofStyle::HalfHip) {
                    cfg.max_ridge_shift * wy
                } else {
                    0.0
                };
                let polys = roof_faces(style, len, wid, 0.4, pitch, &cfg, ridge);
                assert!(!polys.is_empty(), "{style:?}: пустой набор граней");
                for poly in &polys {
                    let pts = poly.points();
                    assert!(pts.len() >= 3, "{style:?}: полигон с <3 точками");
                    let n = poly.normal();
                    assert!(
                        n.length() > 0.001,
                        "{style:?} {pitch}° len={len} wid={wid}: вырожденный полигон {pts:?}"
                    );
                    let base = pts[0];
                    for p in pts.iter().skip(1) {
                        let d = (*p - base).dot(n).abs();
                        assert!(
                            d < 0.01,
                            "{style:?}: точка вне плоскости полигона: {p:?} vs {base:?} n={n:?}"
                        );
                    }
                }
            }
        }
    }
}
