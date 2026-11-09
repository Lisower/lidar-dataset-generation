use super::*;

use std::path::Path;

fn cloud(positions: &[Vec3]) -> PointCloud {
    let mut cloud = PointCloud::new(PointClass::Building);
    cloud.extend_with_class(positions.iter().copied(), PointClass::Building);
    cloud
}

fn config() -> ScannerConfig {
    ScannerConfig {
        noise_probability: 0.0,
        ..default()
    }
}

fn front() -> ScannerPose {
    ScannerPose::looking_at(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO)
}

fn wall(z: f32) -> Vec<[Vec3; 3]> {
    let a = Vec3::new(-2.0, -2.0, z);
    let b = Vec3::new(2.0, -2.0, z);
    let c = Vec3::new(2.0, 2.0, z);
    let d = Vec3::new(-2.0, 2.0, z);
    vec![[a, b, c], [a, c, d]]
}

#[test]
fn geometry_hides_back_points_without_hiding_the_surface() {
    let scene = ScanScene::new(
        cloud(&[Vec3::ZERO, Vec3::NEG_Z, Vec3::new(3.0, 0.0, -1.0)]),
        wall(0.0),
    );
    let frame = scene.scan(front(), &config(), Seed(1)).unwrap();
    assert_eq!(frame.source_indices, vec![0, 2]);
    // Проверка с обратной стороны: треугольники двусторонние.
    let back = ScannerPose::looking_at(Vec3::new(0.0, 0.0, -5.0), Vec3::ZERO);
    assert!(
        scene
            .scan(back, &config(), Seed(1))
            .unwrap()
            .source_indices
            .contains(&1)
    );
}

#[test]
fn field_of_view_and_range_filter_points() {
    let scene = ScanScene::new(
        cloud(&[
            Vec3::ZERO,
            Vec3::new(0.0, 0.0, 6.0),
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(0.0, 10.0, 0.0),
            Vec3::new(0.0, 0.0, -10.0),
            Vec3::new(0.0, 0.0, 4.95),
        ]),
        vec![],
    );
    let cfg = ScannerConfig {
        max_range: 10.0,
        ..config()
    };
    assert_eq!(
        scene.scan(front(), &cfg, Seed(1)).unwrap().source_indices,
        vec![0]
    );
}

#[test]
fn scanner_step_is_independent_of_source_density_and_preserves_classes() {
    let mut source = cloud(&[
        Vec3::new(0.01, 0.1, 0.0),
        Vec3::new(0.04, 0.1, 0.0),
        Vec3::new(0.5, 0.1, 0.0),
    ]);
    source.points[0].class = PointClass::Vegetation;
    let scene = ScanScene::new(source, vec![]);
    let coarse = scene.scan(front(), &config(), Seed(1)).unwrap();
    let fine = scene
        .scan(
            front(),
            &ScannerConfig {
                sample_step: 0.01,
                ..config()
            },
            Seed(1),
        )
        .unwrap();
    assert_eq!(coarse.source_indices, vec![0, 2]);
    assert_eq!(coarse.cloud.points[0].class, PointClass::Vegetation);
    assert_eq!(fine.source_indices.len(), 3);
    assert_eq!(scene.cloud.len(), 3);
}

#[test]
fn low_noise_is_added_reproducibly_and_does_not_replace_returns() {
    let scene = ScanScene::new(cloud(&[Vec3::ZERO, Vec3::X]), vec![]);
    let cfg = ScannerConfig {
        noise_probability: 1.0,
        ..config()
    };
    let a = scene.scan(front(), &cfg, Seed(42)).unwrap();
    let b = scene.scan(front(), &cfg, Seed(42)).unwrap();
    assert_eq!(a.cloud, b.cloud);
    assert_eq!(a.cloud.len(), 4);
    assert_eq!(a.cloud.points[..2], scene.cloud.points);
    for (noise, source) in a.cloud.points[2..].iter().zip(&scene.cloud.points) {
        assert_eq!(noise.class, PointClass::LowNoise);
        assert!((noise.position - source.position).abs().max_element() <= cfg.noise_extent);
    }
    let clean = scene.scan(front(), &config(), Seed(42)).unwrap();
    assert_eq!(clean.cloud.len(), 2);
    let mut accumulated = ScanAccumulator::new(scene.cloud.len());
    accumulated.add(&a);
    accumulated.add(&b);
    assert_eq!(accumulated.cloud.len(), 6);
    assert_eq!(
        accumulated
            .cloud
            .points
            .iter()
            .filter(|p| p.class == PointClass::LowNoise)
            .count(),
        4
    );
}

#[test]
fn trajectory_accumulates_union_without_duplicate_source_points() {
    let scene = ScanScene::new(cloud(&[Vec3::Z, Vec3::NEG_Z]), wall(0.0));
    let poses = orbit_trajectory(Vec3::ZERO, 5.0, 0.0, 2);
    let mut accumulated = ScanAccumulator::new(2);
    let a = scene.scan(poses[0], &config(), Seed(1)).unwrap();
    let b = scene.scan(poses[1], &config(), Seed(2)).unwrap();
    assert_eq!(a.source_indices, vec![0]);
    assert_eq!(b.source_indices, vec![1]);
    accumulated.add(&a);
    accumulated.add(&a);
    assert_eq!(accumulated.cloud.len(), 1);
    accumulated.add(&b);
    assert_eq!(accumulated.seen, vec![true, true]);
    assert_eq!(accumulated.cloud.len(), 2);
    assert_eq!(accumulated.frames, 3);
    assert!(orbit_trajectory(Vec3::ZERO, 5.0, 2.0, 0).is_empty());
}

#[test]
fn bvh_keeps_nearest_blockers_and_allows_rays_outside_bounds() {
    let mut triangles = vec![];
    for i in 0..20 {
        triangles.extend(wall(-(i as f32)));
    }
    let scene = ScanScene::new(
        cloud(&[
            Vec3::ZERO,
            Vec3::new(0.0, 0.0, -20.0),
            Vec3::new(3.0, 0.0, 0.0),
        ]),
        triangles,
    );
    assert_eq!(
        scene
            .scan(front(), &config(), Seed(1))
            .unwrap()
            .source_indices,
        vec![0, 2]
    );
}

#[test]
fn building_cloud_uses_world_up_and_preserves_hierarchy() {
    let building = Building::generate(&buildings_generator::BuildingConfig {
        point_jitter: 0.0,
        ..default()
    });
    let scene = ScanScene::from_building(&building);
    let mut expected = building.point_cloud.clone();
    expected.transform_by(Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_2));
    assert_eq!(scene.cloud.len(), expected.len());
    for (a, b) in scene.cloud.points.iter().zip(&expected.points) {
        assert!(a.position.distance(b.position) < 0.001);
    }
    let frames = orbit_trajectory(
        Vec3::new(0.0, building.total_height() * 0.5, 0.0),
        building.footprint_diagonal() * 1.5,
        building.total_height() + 4.0,
        4,
    );
    let mut accumulated = ScanAccumulator::new(scene.cloud.len());
    let mut first = 0;
    for (index, pose) in frames.into_iter().enumerate() {
        let frame = scene.scan(pose, &config(), Seed(1)).unwrap();
        if index == 0 {
            first = frame.cloud.len();
        }
        accumulated.add(&frame);
    }
    assert!(first > 0);
    assert!(accumulated.cloud.len() > first);
    assert!(accumulated.cloud.len() <= scene.cloud.len());
}

#[test]
fn generated_buildings_show_front_walls_from_all_four_sides() {
    // Размечаем реальные точки стен, а не лишь проверяем ненулевое число
    // отсчётов (ими раньше могла оказаться одна крыша).
    fn wall_mask(part: &Part, mask: &mut Vec<bool>) {
        mask.extend(std::iter::repeat_n(part.name == "стена", part.cloud.len()));
        for child in &part.children {
            wall_mask(child, mask);
        }
    }
    let mut roof_styles = std::collections::HashSet::new();
    for seed in 0..40 {
        let building = Building::generate(&buildings_generator::BuildingConfig {
            seed: Seed(seed),
            point_step: 0.3,
            wall_point_step: 0.3,
            point_jitter: 0.0,
            ..default()
        });
        if !roof_styles.insert(building.roof.label()) {
            continue;
        }
        let scene = ScanScene::from_building(&building);
        let mut mask = Vec::new();
        wall_mask(&building.root, &mut mask);
        assert_eq!(mask.len(), scene.cloud.len());
        let target = Vec3::new(0.0, building.size.z * 0.5, 0.0);
        for outward in [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z] {
            let pose = ScannerPose::looking_at(
                target + outward * building.footprint_diagonal() * 2.0,
                target,
            );
            let frame = scene.scan(pose, &config(), Seed(seed)).unwrap();
            let wall_points = frame.source_indices.iter().filter(|&&i| mask[i]).count();
            assert!(
                wall_points > 20,
                "seed={seed}, roof={:?}, side={outward:?}, wall_points={wall_points}",
                building.roof
            );
            // Ни одной точки противоположной стены через объём здания.
            assert!(
                frame
                    .source_indices
                    .iter()
                    .filter(|&&i| mask[i])
                    .all(|&i| scene.cloud.points[i].position.dot(outward) > 0.0)
            );
        }
    }
    assert_eq!(roof_styles.len(), buildings_generator::RoofStyle::ALL.len());
}

#[test]
fn side_scan_captures_front_wall_but_not_roof_top_or_back_wall() {
    // Закрытый объём с точно известными видимыми поверхностями.
    let positions = [
        Vec3::new(2.0, 1.5, 0.0),
        Vec3::new(-2.0, 1.5, 0.0),
        Vec3::new(0.0, 1.5, 2.0),
        Vec3::new(0.0, 1.5, -2.0),
        Vec3::new(0.0, 3.0, 0.0),
        Vec3::new(1.0, 3.0, 1.0),
    ];
    let triangles = generation_core::geometry::box_polygons(Vec3::new(4.0, 3.0, 4.0))
        .into_iter()
        .flat_map(|face| {
            face.triangulate()
                .map(|p| p + Vec3::Y * 1.5)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>()
        .chunks_exact(3)
        .map(|t| [t[0], t[1], t[2]])
        .collect();
    let scene = ScanScene::new(cloud(&positions), triangles);
    let target = Vec3::new(0.0, 1.5, 0.0);
    for (expected, outward) in [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z]
        .into_iter()
        .enumerate()
    {
        let frame = scene
            .scan(
                ScannerPose::looking_at(target + outward * 10.0, target),
                &config(),
                Seed(1),
            )
            .unwrap();
        assert_eq!(frame.source_indices, vec![expected], "side={outward:?}");
    }
    // Сверху точки кровли видны, центры стен — нет.
    let above = ScannerPose::looking_at(Vec3::new(0.0, 10.0, 0.0), target);
    assert_eq!(
        scene
            .scan(above, &config(), Seed(1))
            .unwrap()
            .source_indices,
        vec![4, 5]
    );
}

#[test]
fn invalid_settings_are_errors() {
    let scene = ScanScene::new(PointCloud::default(), vec![]);
    for cfg in [
        ScannerConfig {
            sample_step: 0.0,
            ..config()
        },
        ScannerConfig {
            sample_step: f32::NAN,
            ..config()
        },
        ScannerConfig {
            horizontal_fov: std::f32::consts::PI,
            ..config()
        },
        ScannerConfig {
            noise_probability: 1.1,
            ..config()
        },
        ScannerConfig {
            noise_extent: -1.0,
            ..config()
        },
        ScannerConfig {
            min_range: 10.0,
            max_range: 1.0,
            ..config()
        },
    ] {
        assert!(scene.scan(front(), &cfg, Seed(1)).is_err());
    }
    assert!(
        scene
            .scan(
                ScannerPose {
                    position: Vec3::NAN,
                    ..front()
                },
                &config(),
                Seed(1)
            )
            .is_err()
    );
    assert!(
        demo::validate_options(&demo::DemoOptions {
            frames: 0,
            ..default()
        })
        .is_err()
    );
}

#[test]
fn headless_single_frame_exports_ply_without_context_points() {
    // Временная папка теста — в `target/` воркспейса: системный TEMP в этой
    // среде может быть недоступен для записи (см. generation-core/presets).
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("lidar-sim-tests");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("scan-{}.ply", std::process::id()));
    demo::run_headless(demo::DemoOptions {
        preset: Some("buildings/cabin".into()),
        source_step: 0.3,
        frames: 1,
        scanner: ScannerConfig {
            noise_probability: 1.0,
            ..config()
        },
        output: path.to_string_lossy().into_owned(),
        ..default()
    })
    .unwrap();
    let output = std::fs::read_to_string(&path).unwrap();
    let (header, body) = output.split_once("end_header\n").unwrap();
    let points: Vec<_> = body.lines().collect();
    assert!(!points.is_empty());
    assert!(header.contains(&format!("element vertex {}", points.len())));
    assert_eq!(
        points.iter().filter(|p| p.ends_with(" 7")).count() * 2,
        points.len()
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn projection_uses_nearest_return_and_depends_on_incidence_and_distance() {
    let pose = ScannerPose::looking_at(Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO);
    let cfg = ScannerConfig {
        sample_step: 0.1,
        ..config()
    };
    let plane = |angle: f32| {
        let rotation = Quat::from_rotation_y(angle);
        let points: Vec<_> = (0..81)
            .flat_map(|i| {
                (0..81).map(move |j| {
                    rotation * Vec3::new(i as f32 * 0.05 - 2.0, j as f32 * 0.05 - 2.0, 0.0)
                })
            })
            .collect();
        ScanScene::new(cloud(&points), vec![])
    };
    let frontal = plane(0.0).scan(pose, &cfg, Seed(1)).unwrap().cloud.len();
    let grazing = plane(80.0_f32.to_radians())
        .scan(pose, &cfg, Seed(1))
        .unwrap()
        .cloud
        .len();
    let far = plane(0.0)
        .scan(
            ScannerPose::looking_at(Vec3::Z * 20.0, Vec3::ZERO),
            &cfg,
            Seed(1),
        )
        .unwrap()
        .cloud
        .len();
    assert!(
        grazing < frontal / 2,
        "frontal={frontal}, grazing={grazing}"
    );
    assert!(far < frontal / 2);
    // Совпадающая проекция на разных глубинах: остаётся ближний возврат.
    let scene = ScanScene::new(cloud(&[Vec3::NEG_Z, Vec3::ZERO]), vec![]);
    assert_eq!(
        scene.scan(pose, &cfg, Seed(1)).unwrap().source_indices,
        vec![1]
    );
}

#[test]
fn randomized_paths_are_reproducible_straight_and_fit_scene() {
    use crate::trajectory::{SceneBounds, TrajectoryKind};
    let bounds = SceneBounds {
        min: Vec3::splat(-10.0),
        max: Vec3::splat(10.0),
    };
    for kind in [TrajectoryKind::Line, TrajectoryKind::Orbit] {
        for seed in 0..20 {
            let path = bounds.trajectory(kind, 12, Seed(seed));
            let repeat = bounds.trajectory(kind, 12, Seed(seed));
            let different = bounds.trajectory(kind, 12, Seed(seed + 1));
            assert_ne!(path[0].position, different[0].position);
            for (a, b) in path.iter().zip(&repeat) {
                assert_eq!(a.position, b.position);
                assert!(a.position.is_finite() && a.rotation.is_normalized());
                assert!(
                    (a.rotation * Vec3::NEG_Z).dot((bounds.center() - a.position).normalize())
                        > 0.999
                );
                let cfg = bounds.fit_config(*a, &config());
                cfg.validate().unwrap();
                for x in [bounds.min.x, bounds.max.x] {
                    for y in [bounds.min.y, bounds.max.y] {
                        for z in [bounds.min.z, bounds.max.z] {
                            let local = a.rotation.inverse() * (Vec3::new(x, y, z) - a.position);
                            assert!(local.x.abs() <= -local.z * (cfg.horizontal_fov * 0.5).tan());
                            assert!(local.y.abs() <= -local.z * (cfg.vertical_fov * 0.5).tan());
                        }
                    }
                }
            }
            if kind == TrajectoryKind::Line {
                let delta = path[1].position - path[0].position;
                for segment in path.windows(2) {
                    assert!((segment[1].position - segment[0].position - delta).length() < 0.001);
                }
            }
        }
        assert!(bounds.trajectory(kind, 0, Seed(1)).is_empty());
        assert_eq!(bounds.trajectory(kind, 1, Seed(1)).len(), 1);
    }
}

#[test]
fn all_generator_categories_scan_and_export_shared_xyz() {
    use crate::trajectory::{SceneBounds, TrajectoryKind};
    use generation_core::presets::PresetStore;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../presets");
    let store = PresetStore::discover_all(&root);
    let ids: std::collections::BTreeSet<_> = store.presets.iter().map(|m| &m.id).collect();
    assert_eq!(
        ids.len(),
        store.presets.len(),
        "id must be unique across categories"
    );
    for id in [
        "buildings/cabin",
        "plants/cactus-small",
        "landscapes/plain",
        "terrains/open-field",
    ] {
        let meta = store.meta(id).unwrap_or_else(|| panic!("missing {id}"));
        let scene = sources::from_preset(meta, &root, Seed(42), 1.0).unwrap();
        let bounds = SceneBounds::from_cloud(&scene.cloud).unwrap();
        let pose = bounds.trajectory(TrajectoryKind::Orbit, 1, Seed(42))[0];
        let cfg = bounds.fit_config(
            pose,
            &ScannerConfig {
                noise_probability: 1.0,
                ..config()
            },
        );
        let frame = scene.scan(pose, &cfg, Seed(42)).unwrap();
        assert!(!frame.source_indices.is_empty(), "{id}");
        assert_eq!(frame.cloud.len(), frame.source_indices.len() * 2);
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/scanner-tests")
            .join(format!("{}.xyz", id.replace('/', "-")));
        demo::export_scan(&frame.cloud, &path).unwrap();
        let output = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            output,
            generation_core::point_clouds::export::to_xyz(&frame.cloud)
        );
        assert_eq!(
            output.lines().filter(|s| s.starts_with("7 ")).count(),
            frame.source_indices.len()
        );
        std::fs::remove_file(path).unwrap();
    }
}
