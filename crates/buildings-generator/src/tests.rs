//! Unit-тесты генерации зданий.
//!
//! Генерация полностью детерминирована от seed: одни и те же входные данные
//! всегда дают один и тот же результат. Проверяем детерминизм, диапазоны
//! габаритов/этажей, достижимость всех типов крыш и то, что здание стоит на
//! земле (нет подземных точек).

use generation_core::Seed;

use crate::generation::{Building, BuildingConfig, RoofStyle};

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
    assert_ne!(a.point_cloud, b.point_cloud, "разные seed дали одинаковое здание");
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

#[test]
fn half_hip_has_sloping_ends_and_planar_sides() {
    use crate::generation::roof::roof_faces;
    for (length, width) in [(12.0, 8.0), (8.0, 12.0), (5.0, 5.0)] {
        for pitch in [5.0, 27.0, 40.0] {
            let wy = width * 0.5 + 0.4;
            for ridge in [-0.35 * wy, 0.0, 0.35 * wy] {
                let faces = roof_faces(
                    RoofStyle::HalfHip, length, width, 0.4, pitch, &config(3), ridge,
                );
                // Hips occupy a substantial part of the roof height and depth.
                let lx = length * 0.5 + 0.4;
                let hip = faces[4].points();
                let max_z = hip.iter().map(|p| p.z).fold(0.0_f32, f32::max);
                let rise = pitch.to_radians().tan() * wy;
                assert!(max_z / rise >= 0.6);
                let min_x = hip.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
                assert!((lx - min_x) / lx >= 0.3);
                for (face, sign) in [(&faces[4], 1.0), (&faces[5], -1.0)] {
                    let n = face.normal();
                    assert!(n.x * sign > 0.01 && n.z > 0.01);
                }
                for face in &faces[..8] {
                    let points = face.points();
                    let n = face.normal();
                    assert!(n.length() > 0.001);
                    for p in points.iter().skip(1) {
                        assert!((*p - points[0]).dot(n).abs() < 0.001);
                    }
                }
                assert!(faces[0].normal().dot(faces[1].normal()) > 0.999);
                assert!(faces[2].normal().dot(faces[3].normal()) > 0.999);
            }
        }
    }
}

#[test]
fn compound_presets_have_empty_notches_and_no_shared_wall_points() {
    use crate::generation::BuildingPreset;
    for preset in [BuildingPreset::LShaped, BuildingPreset::WithAnnex] {
        for seed in 0..12 {
            let mut cfg = config(seed);
            cfg.preset = preset;
            cfg.point_jitter = 0.0;
            let b = Building::generate(&cfg);
            let repeated = Building::generate(&cfg);
            assert_eq!(b.point_cloud, repeated.point_cloud);
            assert_eq!(b.config.preset, preset);
            assert_eq!(b.roof, RoofStyle::Flat);
            let joint = b.size.x * 0.12;
            let ah = if preset == BuildingPreset::LShaped { b.size.z } else { b.floor_height * 0.65 };
            for point in &b.point_cloud.points {
                let p = point.position;
                assert!(p.is_finite());
                assert!(p.z >= -0.02);
                assert!(!(p.x > joint + 0.15 && p.y > 0.15), "filled footprint notch: {p:?}");
                assert!(!((p.x - joint).abs() < 0.001 && p.y < -0.2 && p.y > -b.size.y*0.5+0.2 && p.z > 0.1 && p.z < ah-0.1), "internal shared wall: {p:?}");
            }
            let roofs = &b.root.children.last().unwrap().faces;
            let first = roofs[0].points()[0].z;
            let second = roofs[1].points()[0].z;
            if preset == BuildingPreset::LShaped { assert!((first-second).abs() < 0.001); }
            else { assert!(first > second + 0.5); }
        }
    }
}

#[test]
fn tall_presets_generate_complete_repeatable_buildings() {
    use crate::generation::BuildingPreset;
    for (preset,min_floors,max_floors) in [
        (BuildingPreset::ApartmentBlock,6,16),
        (BuildingPreset::Skyscraper,32,60),
    ] {
        let mut cfg = config(42);
        cfg.preset = preset;
        let b = Building::generate(&cfg);
        let repeated = Building::generate(&cfg);
        assert_eq!(b.point_cloud,repeated.point_cloud);
        assert_eq!(b.config.preset,preset);
        assert!((min_floors..=max_floors).contains(&b.floors));
        assert_eq!(b.root.children.len(),b.floors as usize+1);
        assert_eq!(b.roof,RoofStyle::Flat);
        assert!(b.parapet && !b.dormer);
        assert!((b.size.z-b.floor_height*b.floors as f32).abs()<0.001);
        let max_z=b.point_cloud.points.iter().map(|p|p.position.z).fold(f32::NEG_INFINITY,f32::max);
        assert!(max_z>b.size.z);
        assert!(max_z<=b.total_height()+0.02);
        assert!(b.point_cloud.points.iter().all(|p|p.position.is_finite() && p.position.z>=-0.02));
        if preset==BuildingPreset::Skyscraper {
            assert!(b.size.z>b.size.x.max(b.size.y)*3.0);
            assert_eq!(b.config.balcony_probability,0.0);
        }
    }
}
