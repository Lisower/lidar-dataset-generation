use super::*;

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
        assert_eq!(noise.class, PointClass::Noise);
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
            .filter(|p| p.class == PointClass::Noise)
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
    let path = std::env::temp_dir().join(format!("lidar-scan-test-{}.ply", std::process::id()));
    demo::run_headless(demo::DemoOptions {
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
