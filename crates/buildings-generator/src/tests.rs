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
                let ridge = if matches!(
                    style,
                    RoofStyle::Gable | RoofStyle::HalfHip | RoofStyle::HalfHipGable
                ) {
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
                    RoofStyle::HalfHip,
                    length,
                    width,
                    0.4,
                    pitch,
                    &config(3),
                    ridge,
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
fn gable_half_hip_has_large_gables_and_small_upper_hips() {
    use crate::generation::roof::roof_faces;
    for (length, width) in [(12.0, 8.0), (8.0, 12.0), (5.0, 5.0)] {
        for pitch in [5.0_f32, 27.0, 40.0] {
            let wy = width * 0.5 + 0.4;
            for ridge in [-0.35 * wy, 0.0, 0.35 * wy] {
                let faces = roof_faces(
                    RoofStyle::HalfHipGable,
                    length,
                    width,
                    0.4,
                    pitch,
                    &config(3),
                    ridge,
                );
                let rise = pitch.to_radians().tan() * wy;
                assert_eq!(faces[2].points().len(), 3);
                assert_eq!(faces[3].points().len(), 3);
                assert!(faces[2].points().iter().any(|p| (p.z - rise).abs() < 0.001));
                for gable in &faces[4..6] {
                    assert_eq!(gable.points().len(), 4);
                    assert!(gable.points().iter().filter(|p| p.z == 0.0).count() == 2);
                }
            }
        }
    }
}

#[test]
fn compound_presets_have_empty_notches_and_no_shared_wall_points() {
    use crate::generation::BuildingPreset;
    for preset in [BuildingPreset::LShaped, BuildingPreset::WithAnnex] {
        let mut roofs_seen = std::collections::HashSet::new();
        let mut corners_seen = std::collections::HashSet::new();
        for seed in 0..80 {
            let mut cfg = config(seed);
            cfg.preset = preset;
            cfg.point_jitter = 0.0;
            let b = Building::generate(&cfg);
            let repeated = Building::generate(&cfg);
            assert_eq!(b.point_cloud, repeated.point_cloud);
            assert_eq!(b.config.preset, preset);
            roofs_seen.insert(b.roof);
            let roof = b.root.children.last().unwrap();
            let wing = &roof.children[1];
            let (min, max) = wing.faces.iter().flat_map(|f| f.points()).fold(
                (
                    bevy::math::Vec3::splat(f32::INFINITY),
                    bevy::math::Vec3::splat(f32::NEG_INFINITY),
                ),
                |(min, max), &p| (min.min(p), max.max(p)),
            );
            let sx = if min.x + max.x > 0.0 { 1.0 } else { -1.0 };
            let sy = if min.y + max.y < 0.0 { 1.0 } else { -1.0 };
            corners_seen.insert((sx > 0.0, sy > 0.0));
            let joint = b.size.x * 0.12;
            let ah = if preset == BuildingPreset::LShaped {
                b.size.z
            } else {
                b.floor_height * 0.65
            };
            for point in &b.point_cloud.points {
                let p = point.position * bevy::math::Vec3::new(sx, sy, 1.0);
                assert!(p.is_finite());
                assert!(p.z >= -0.02);
                assert!(
                    !(p.x > joint + 0.15 && p.y > 0.15),
                    "filled footprint notch: {p:?}"
                );
                assert!(
                    !((p.x - joint).abs() < 0.001
                        && p.y < -0.2
                        && p.y > -b.size.y * 0.5 + 0.2
                        && p.z > 0.1
                        && p.z < ah - 0.1),
                    "internal shared wall: {p:?}"
                );
            }
            for section in &roof.children {
                for face in &section.faces {
                    let n = face.normal();
                    assert!(n.is_finite() && n.length() > 0.99);
                    assert!(
                        face.points()
                            .iter()
                            .all(|p| (*p - face.points()[0]).dot(n).abs() < 0.001)
                    );
                }
            }
        }
        assert_eq!(roofs_seen.len(), RoofStyle::ALL.len());
        assert_eq!(corners_seen.len(), 4);
    }
}

#[test]
fn tall_presets_generate_complete_repeatable_buildings() {
    use crate::generation::BuildingPreset;
    for (preset, min_floors, max_floors) in [
        (BuildingPreset::ApartmentBlock, 6, 16),
        (BuildingPreset::MultiEntranceApartment, 6, 16),
        (BuildingPreset::Skyscraper, 32, 60),
    ] {
        let mut cfg = config(42);
        cfg.preset = preset;
        let b = Building::generate(&cfg);
        let repeated = Building::generate(&cfg);
        assert_eq!(b.point_cloud, repeated.point_cloud);
        assert_eq!(b.config.preset, preset);
        assert!((min_floors..=max_floors).contains(&b.floors));
        assert_eq!(b.root.children.len(), b.floors as usize + 1);
        assert_eq!(b.roof, RoofStyle::Flat);
        assert!(b.parapet && !b.dormer);
        assert!((b.size.z - b.floor_height * b.floors as f32).abs() < 0.001);
        let max_z = b
            .point_cloud
            .points
            .iter()
            .map(|p| p.position.z)
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(max_z > b.size.z);
        assert!(max_z <= b.total_height() + 0.02);
        assert!(
            b.point_cloud
                .points
                .iter()
                .all(|p| p.position.is_finite() && p.position.z >= -0.02)
        );
        if preset == BuildingPreset::Skyscraper {
            assert!(b.size.z > b.size.x.max(b.size.y) * 3.0);
            assert_eq!(b.config.balcony_probability, 0.0);
        }
    }
}

#[test]
fn tall_buildings_repeat_upper_floor_layouts() {
    use crate::generation::BuildingPreset;
    for preset in [
        BuildingPreset::ApartmentBlock,
        BuildingPreset::MultiEntranceApartment,
        BuildingPreset::Skyscraper,
    ] {
        let mut balconies_seen = false;
        for seed in 0..12 {
            let mut cfg = config(seed);
            cfg.preset = preset;
            let b = Building::generate(&cfg);
            let first = &b.root.children[1];
            balconies_seen |= first
                .children
                .iter()
                .any(|wall| wall.children.iter().any(|p| p.name == "балкон"));
            let cloud = first.collect_cloud();
            for floor in &b.root.children[2..b.floors as usize] {
                assert_eq!(
                    floor.collect_cloud(),
                    cloud,
                    "{preset:?}, seed={seed}: floor layouts differ"
                );
            }
        }
        if preset != BuildingPreset::Skyscraper {
            assert!(balconies_seen);
        }
    }
}

#[test]
fn multiple_entrances_have_separate_ground_floor_openings() {
    use crate::generation::BuildingPreset;
    let mut counts = std::collections::HashSet::new();
    for seed in 0..20 {
        let mut cfg = config(seed);
        cfg.preset = BuildingPreset::MultiEntranceApartment;
        cfg.point_jitter = 0.0;
        let b = Building::generate(&cfg);
        counts.insert(b.entrance_count);
        let ground = &b.root.children[0];
        let entrances: Vec<_> = ground
            .children
            .iter()
            .filter(|wall| wall.children.iter().any(|p| p.name == "дверь"))
            .collect();
        assert_eq!(
            entrances.len(),
            1,
            "all entrances should face the same long facade"
        );
        let wall = entrances[0];
        let doors: Vec<_> = wall.children.iter().filter(|p| p.name == "дверь").collect();
        assert_eq!(doors.len(), b.entrance_count as usize);
        assert!((2..=4).contains(&b.entrance_count));
        assert!(wall.transform.translation.y.abs() > b.size.y * 0.4);
        for (i, door) in doors.iter().enumerate() {
            let x = door.transform.translation.x;
            assert!(x.abs() + b.config.door_width < b.size.x * 0.5);
            if i > 0 {
                assert!(x - doors[i - 1].transform.translation.x > b.config.door_width + 1.0);
            }
            for point in &wall.cloud.points {
                let p = point.position;
                assert!(
                    !((p.x - x).abs() < b.config.door_width * 0.5
                        && p.z < -b.floor_height * 0.5 + b.config.door_height),
                    "wall points cover an entrance: {p:?}"
                );
            }
            for window in wall.children.iter().filter(|p| p.name == "окно") {
                assert!(
                    (window.transform.translation.x - x).abs()
                        > (b.config.window_width + b.config.door_width) * 0.5
                );
            }
        }
    }
    assert_eq!(counts, [2, 3, 4].into_iter().collect());
    let preset: BuildingConfig = ron::from_str(include_str!(
        "../../../presets/buildings/multi-storey/multi-entrance/config.ron"
    ))
    .unwrap();
    assert_eq!(preset.preset, BuildingPreset::MultiEntranceApartment);
    assert_eq!(preset.enclosed_balcony_probability, 0.35);
}

#[test]
fn enclosed_balcony_probability_can_force_either_style() {
    for probability in [0.0, 1.0] {
        let mut cfg = config(9);
        cfg.min_floors = 3;
        cfg.max_floors = 3;
        cfg.balcony_probability = 1.0;
        cfg.enclosed_balcony_probability = probability;
        let b = Building::generate(&cfg);
        let balconies: Vec<_> = b
            .root
            .children
            .iter()
            .filter(|p| p.name == "этаж")
            .flat_map(|p| &p.children)
            .flat_map(|p| &p.children)
            .filter(|p| p.name == "балкон")
            .collect();
        assert_eq!(balconies.len(), 2);
        for balcony in balconies {
            assert_eq!(
                balcony
                    .children
                    .iter()
                    .any(|p| p.name == "балкон-остекление"),
                probability == 1.0
            );
            assert_eq!(
                balcony
                    .children
                    .iter()
                    .any(|p| p.name == "балкон-перекрытие"),
                probability == 1.0
            );
        }
    }
}

#[test]
fn outbuildings_use_their_roof_sets_and_clear_wide_gate_openings() {
    use crate::generation::BuildingPreset;
    for (preset, styles) in [
        (
            BuildingPreset::Hangar,
            vec![RoofStyle::Gable, RoofStyle::Barrel],
        ),
        (
            BuildingPreset::Garage,
            vec![RoofStyle::Flat, RoofStyle::Gable, RoofStyle::Shed],
        ),
        (BuildingPreset::Barn, vec![RoofStyle::Gable]),
    ] {
        let mut seen = std::collections::HashSet::new();
        for seed in 0..40 {
            let mut cfg = config(seed);
            cfg.preset = preset;
            cfg.point_jitter = 0.0;
            let b = Building::generate(&cfg);
            assert_eq!(b.config.preset, preset);
            assert_eq!(b.floors, 1);
            assert!(!b.dormer && !b.parapet);
            assert!(styles.contains(&b.roof));
            seen.insert(b.roof);
            assert!(b.config.door_width + 0.2 < b.size.x.min(b.size.y));
            assert!(b.config.door_height + 0.14 < b.size.z);
            let floor = &b.root.children[0];
            assert!(
                floor
                    .children
                    .iter()
                    .all(|w| w.children.iter().all(|p| p.name != "балкон"))
            );
            let gates: Vec<_> = floor
                .children
                .iter()
                .flat_map(|w| {
                    w.children
                        .iter()
                        .filter(|p| p.name == "ворота")
                        .map(move |gate| (w, gate))
                })
                .collect();
            assert_eq!(gates.len(), 1);
            let (wall, gate) = gates[0];
            assert!(!wall.children.iter().any(|p| p.name == "окно"));
            let along_x = wall.transform.translation.y.abs() > 0.1;
            let facade_length = if along_x { b.size.x } else { b.size.y };
            assert!((facade_length - b.size.x.min(b.size.y)).abs() < 0.001);
            for point in &wall.cloud.points {
                let p = point.position;
                let along = if along_x { p.x } else { p.y };
                assert!(
                    !(along.abs() < b.config.door_width * 0.5
                        && p.z < -b.floor_height * 0.5 + b.config.door_height),
                    "{preset:?}: wall points cover the gate"
                );
            }
            assert_eq!(
                gate.children
                    .iter()
                    .filter(|p| p.name == "ворота-раскос")
                    .count(),
                if preset == BuildingPreset::Barn { 4 } else { 0 }
            );
            assert!(
                b.point_cloud
                    .points
                    .iter()
                    .all(|p| p.position.is_finite() && p.position.z >= -1e-4)
            );
            if seed == 0 {
                assert_eq!(b.point_cloud, Building::generate(&cfg).point_cloud);
            }
        }
        assert_eq!(seen, styles.into_iter().collect());
    }
}

#[test]
fn outbuilding_presets_deserialize() {
    use crate::generation::BuildingPreset;
    for (text, preset) in [
        (
            include_str!("../../../presets/buildings/outbuildings/hangar/config.ron"),
            BuildingPreset::Hangar,
        ),
        (
            include_str!("../../../presets/buildings/outbuildings/garage/config.ron"),
            BuildingPreset::Garage,
        ),
        (
            include_str!("../../../presets/buildings/outbuildings/barn/config.ron"),
            BuildingPreset::Barn,
        ),
    ] {
        let cfg: BuildingConfig = ron::from_str(text).unwrap();
        assert_eq!(cfg.preset, preset);
    }
}
