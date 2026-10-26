//! Интерактивная демонстрация и пакетный экспорт без графического окна.
use crate::{ScanAccumulator, ScanFrame, ScanScene, ScannerConfig, ScannerPose, orbit_trajectory};
use bevy::prelude::*;
use bevy_utils::cameras::{CameraPlugin, MainCamera};
use buildings_generator::{Building, BuildingConfig};
use generation_core::{
    PointClass, PointCloud, Seed,
    point_clouds::visual::{point_cloud_material, point_cloud_mesh},
};
use std::{
    io::{BufWriter, Write},
    path::Path,
};

#[derive(Clone, Debug)]
pub struct DemoOptions {
    pub seed: Seed,
    pub source_step: f32,
    pub scanner: ScannerConfig,
    pub frames: usize,
    pub output: String,
}

impl Default for DemoOptions {
    fn default() -> Self {
        Self {
            seed: Seed::default(),
            source_step: 0.08,
            scanner: ScannerConfig::default(),
            frames: 72,
            output: "assets/export/building-scan.ply".into(),
        }
    }
}

#[derive(Resource)]
struct Demo {
    options: DemoOptions,
    scene: ScanScene,
    path: Vec<ScannerPose>,
    index: usize,
    pose: ScannerPose,
    frame: ScanFrame,
    accumulated: ScanAccumulator,
    cumulative: bool,
    show_context: bool,
    running: bool,
    timer: Timer,
    status: String,
}

impl Demo {
    fn new(options: DemoOptions) -> Self {
        let building = Building::generate(&BuildingConfig {
            seed: options.seed,
            point_step: options.source_step,
            wall_point_step: options.source_step,
            // Исходная поверхность точна; шум измерений добавляет сканер.
            point_jitter: 0.0,
            ..default()
        });
        let target = Vec3::new(0.0, building.total_height() * 0.45, 0.0);
        let path = orbit_trajectory(
            target,
            building.footprint_diagonal() * 1.5,
            building.total_height() + 4.0,
            options.frames,
        );
        let scene = ScanScene::from_building(&building);
        let pose = path[0];
        let frame = scene
            .scan(pose, &options.scanner, options.seed.child(0))
            .expect("validated options");
        let mut accumulated = ScanAccumulator::new(scene.cloud.len());
        accumulated.add(&frame);
        Self {
            options,
            scene,
            path,
            index: 0,
            pose,
            frame,
            accumulated,
            cumulative: false,
            show_context: true,
            running: false,
            timer: Timer::from_seconds(0.15, TimerMode::Repeating),
            status: String::new(),
        }
    }

    fn capture(&mut self, pose: ScannerPose, reset: bool) {
        self.status.clear();
        if reset {
            self.accumulated = ScanAccumulator::new(self.scene.cloud.len());
        }
        self.pose = pose;
        self.frame = self
            .scene
            .scan(
                pose,
                &self.options.scanner,
                self.options.seed.child(self.accumulated.frames as u64),
            )
            .expect("validated options");
        self.accumulated.add(&self.frame);
    }

    fn result(&self) -> &PointCloud {
        if self.cumulative {
            &self.accumulated.cloud
        } else {
            &self.frame.cloud
        }
    }

    fn mesh(&self) -> Mesh {
        if !self.show_context {
            let mut mesh = point_cloud_mesh(self.result());
            let colors: Vec<[f32; 4]> = self
                .result()
                .points
                .iter()
                .map(|point| {
                    if point.class == PointClass::Noise {
                        [1.0, 0.55, 0.05, 1.0]
                    } else {
                        let c = point.class.color().to_srgba();
                        [c.red, c.green, c.blue, 1.0]
                    }
                })
                .collect();
            mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
            return mesh;
        }
        let mut mask = vec![false; self.scene.cloud.len()];
        if self.cumulative {
            mask.clone_from(&self.accumulated.seen);
        } else {
            for &index in &self.frame.source_indices {
                mask[index] = true;
            }
        }
        let mut mesh = point_cloud_mesh(&self.scene.cloud);
        let mut colors: Vec<[f32; 4]> = self
            .scene
            .cloud
            .points
            .iter()
            .zip(mask)
            .map(|(point, seen)| {
                if seen {
                    let c = point.class.color().to_srgba();
                    [c.red, c.green, c.blue, 1.0]
                } else {
                    [0.38, 0.40, 0.43, 1.0]
                }
            })
            .collect();
        let mut positions: Vec<[f32; 3]> =
            self.scene.cloud.positions().map(|p| p.to_array()).collect();
        for point in &self.result().points {
            if point.class == PointClass::Noise {
                positions.push(point.position.to_array());
                colors.push([1.0, 0.55, 0.05, 1.0]);
            }
        }
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
        mesh
    }
}

pub fn validate_options(options: &DemoOptions) -> Result<(), &'static str> {
    options.scanner.validate()?;
    if !options.source_step.is_finite() || options.source_step < 0.02 {
        return Err("source-step должен быть не меньше 0.02 м");
    }
    if !(1..=10000).contains(&options.frames) {
        return Err("frames должен быть от 1 до 10000");
    }
    Ok(())
}

/// Только измеренные точки, без серого контекста. Класс Noise = 7 (Low Noise).
pub fn export_ply(cloud: &PointCloud, path: impl AsRef<Path>) -> std::io::Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let mut out = BufWriter::new(std::fs::File::create(path)?);
    writeln!(
        out,
        "ply\nformat ascii 1.0\nelement vertex {}\nproperty float x\nproperty float y\nproperty float z\nproperty uchar classification\nend_header",
        cloud.len()
    )?;
    for point in &cloud.points {
        let class = match point.class {
            PointClass::Ground => 2,
            PointClass::Vegetation => 5,
            PointClass::Building => 6,
            PointClass::Noise => 7,
            PointClass::Water => 9,
            PointClass::Other => 1,
        };
        writeln!(
            out,
            "{} {} {} {}",
            point.position.x, point.position.y, point.position.z, class
        )?;
    }
    out.flush()
}

pub fn run_headless(options: DemoOptions) -> Result<(), Box<dyn std::error::Error>> {
    validate_options(&options)?;
    let mut demo = Demo::new(options);
    demo.accumulated = ScanAccumulator::new(demo.scene.cloud.len());
    for index in 0..demo.path.len() {
        demo.capture(demo.path[index], false);
    }
    export_ply(&demo.accumulated.cloud, &demo.options.output)?;
    println!(
        "Кадров: {}, исходных точек: {}, увидено: {}, итог с Low Noise: {}. Файл: {}",
        demo.accumulated.frames,
        demo.scene.cloud.len(),
        demo.accumulated.seen.iter().filter(|&&seen| seen).count(),
        demo.accumulated.cloud.len(),
        demo.options.output
    );
    Ok(())
}

pub fn run(options: DemoOptions) -> Result<(), &'static str> {
    validate_options(&options)?;
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.025, 0.035, 0.05)))
        .insert_resource(Demo::new(options))
        .add_plugins((
            DefaultPlugins.set(AssetPlugin {
                file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/../buildings-generator/assets")
                    .into(),
                ..default()
            }),
            CameraPlugin::new(Vec3::new(0.0, 4.0, 0.0), 55.0),
        ))
        .add_systems(Startup, setup)
        .add_systems(Update, (controls, refresh, draw_scanner).chain())
        .run();
    Ok(())
}

#[derive(Component)]
struct CloudView;
#[derive(Component)]
struct Info;

fn setup(
    mut commands: Commands,
    demo: Res<Demo>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    assets: Res<AssetServer>,
) {
    commands.spawn((
        CloudView,
        Mesh3d(meshes.add(demo.mesh())),
        MeshMaterial3d(materials.add(point_cloud_material())),
    ));
    commands.spawn((
        Info,
        Text::new(""),
        TextFont {
            font: assets.load("fonts/Roboto-Regular.ttf").into(),
            font_size: FontSize::Px(16.0),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(14.0),
            top: Val::Px(14.0),
            ..default()
        },
    ));
}

fn controls(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut demo: ResMut<Demo>,
    camera: Query<&Transform, With<MainCamera>>,
) {
    if keys.just_pressed(KeyCode::KeyR) {
        let mut options = demo.options.clone();
        if !keys.pressed(KeyCode::ShiftLeft) && !keys.pressed(KeyCode::ShiftRight) {
            options.seed = Seed::from_entropy();
        }
        let show_context = demo.show_context;
        *demo = Demo::new(options);
        demo.show_context = show_context;
    }
    let mut recapture = false;
    if keys.just_pressed(KeyCode::BracketLeft) {
        demo.options.scanner.sample_step = (demo.options.scanner.sample_step / 1.25).max(0.02);
        recapture = true;
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        demo.options.scanner.sample_step = (demo.options.scanner.sample_step * 1.25).min(5.0);
        recapture = true;
    }
    if keys.just_pressed(KeyCode::KeyN) {
        demo.options.scanner.noise_probability = if demo.options.scanner.noise_probability > 0.0 {
            0.0
        } else {
            0.02
        };
        recapture = true;
    }
    if keys.just_pressed(KeyCode::ArrowLeft) || keys.just_pressed(KeyCode::ArrowRight) {
        let count = demo.path.len();
        demo.index = if keys.just_pressed(KeyCode::ArrowRight) {
            (demo.index + 1) % count
        } else {
            (demo.index + count - 1) % count
        };
        demo.pose = demo.path[demo.index];
        recapture = true;
    }
    if keys.just_pressed(KeyCode::Space) {
        if let Ok(camera) = camera.single() {
            demo.pose = ScannerPose {
                position: camera.translation,
                rotation: camera.rotation,
            };
            recapture = true;
        }
    }
    if keys.just_pressed(KeyCode::KeyC) {
        recapture = true;
    }
    if recapture {
        demo.running = false;
        demo.cumulative = false;
        let pose = demo.pose;
        demo.capture(pose, true);
    }
    if keys.just_pressed(KeyCode::KeyT) {
        demo.index = 0;
        demo.cumulative = true;
        demo.running = demo.path.len() > 1;
        demo.timer.reset();
        let pose = demo.path[0];
        demo.capture(pose, true);
    }
    if keys.just_pressed(KeyCode::KeyP)
        && demo.accumulated.frames > 0
        && demo.index + 1 < demo.path.len()
    {
        demo.running = !demo.running;
        demo.cumulative = true;
    }
    if keys.just_pressed(KeyCode::KeyM) {
        demo.cumulative = !demo.cumulative;
    }
    if keys.just_pressed(KeyCode::KeyV) {
        demo.show_context = !demo.show_context;
    }
    // Не трогаем ResMut при простом осмотре: меш не перестраивается каждый кадр.
    let advance = demo.running
        && demo
            .bypass_change_detection()
            .timer
            .tick(time.delta())
            .just_finished();
    if advance {
        demo.index += 1;
        let pose = demo.path[demo.index];
        demo.capture(pose, false);
        if demo.index + 1 == demo.path.len() {
            demo.running = false;
        }
    }
    if keys.just_pressed(KeyCode::KeyX) {
        demo.status = match export_ply(demo.result(), &demo.options.output) {
            Ok(()) => format!("Сохранено: {}", demo.options.output),
            Err(error) => format!("Ошибка сохранения: {error}"),
        };
    }
}

fn refresh(
    demo: Res<Demo>,
    cloud: Query<&Mesh3d, With<CloudView>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut text: Query<&mut Text, With<Info>>,
) {
    if !demo.is_changed() {
        return;
    }
    if let Ok(handle) = cloud.single() {
        if let Some(mut mesh) = meshes.get_mut(&handle.0) {
            *mesh = demo.mesh();
        }
    }
    if let Ok(mut text) = text.single_mut() {
        text.0 = format!(
            "LiDAR · seed {} · ракурс {}/{}\nИсходное облако: {} · сетка {:.2} м\nШаг сканера: {:.2} м · Low Noise: {:.1}%\nРежим: {} · кадров накоплено: {}\nТочек в результате: {} · шумовых: {}\nОтображение: {} · высота сканера {:.1} м\n\nКрасные — захвачены · серые — не захвачены · оранжевые — Low Noise\n←/→ ракурс · Space кадр из камеры · T траектория · P пауза\nV всё/отсканированное · M кадр/накопление · C сброс\n[/] шаг · N шум · X сохранить PLY\nR новое здание · Shift+R повтор · Tab камера · мышь вращение/зум\nКамера — осмотр; Space — переснять с её позиции\n{}",
            demo.options.seed,
            demo.index + 1,
            demo.path.len(),
            demo.scene.cloud.len(),
            demo.options.source_step,
            demo.options.scanner.sample_step,
            demo.options.scanner.noise_probability * 100.0,
            if demo.cumulative {
                "накопление"
            } else {
                "один кадр"
            },
            demo.accumulated.frames,
            demo.result().len(),
            demo.result()
                .points
                .iter()
                .filter(|p| p.class == PointClass::Noise)
                .count(),
            if demo.show_context {
                "всё облако"
            } else {
                "только отсканированное"
            },
            demo.pose.position.y,
            demo.status
        );
    }
}

fn draw_scanner(demo: Res<Demo>, mut gizmos: Gizmos) {
    let color = Color::srgb(0.1, 0.85, 1.0);
    for i in 0..demo.path.len() {
        gizmos.line(
            demo.path[i].position,
            demo.path[(i + 1) % demo.path.len()].position,
            Color::srgb(0.15, 0.3, 0.4),
        );
    }
    let pose = demo.pose;
    let h = (demo.options.scanner.horizontal_fov * 0.5).tan() * 3.0;
    let v = (demo.options.scanner.vertical_fov * 0.5).tan() * 3.0;
    let corners = [
        Vec3::new(-h, -v, -3.0),
        Vec3::new(h, -v, -3.0),
        Vec3::new(h, v, -3.0),
        Vec3::new(-h, v, -3.0),
    ]
    .map(|p| pose.position + pose.rotation * p);
    for i in 0..4 {
        gizmos.line(pose.position, corners[i], color);
        gizmos.line(corners[i], corners[(i + 1) % 4], color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    #[test]
    fn point_mesh_keeps_gray_context_and_appends_noise() {
        let demo = Demo::new(DemoOptions {
            source_step: 0.3,
            scanner: ScannerConfig {
                noise_probability: 1.0,
                ..default()
            },
            ..default()
        });
        let mesh = demo.mesh();
        let Some(VertexAttributeValues::Float32x4(colors)) = mesh.attribute(Mesh::ATTRIBUTE_COLOR)
        else {
            panic!("missing vertex colors");
        };
        let captured = demo.frame.source_indices.len();
        assert!(captured > 0 && captured < demo.scene.cloud.len());
        assert_eq!(mesh.count_vertices(), demo.scene.cloud.len() + captured);
        assert_eq!(
            colors
                .iter()
                .filter(|&&c| c == [0.38, 0.40, 0.43, 1.0])
                .count(),
            demo.scene.cloud.len() - captured
        );
        assert_eq!(
            colors
                .iter()
                .filter(|&&c| c == [1.0, 0.55, 0.05, 1.0])
                .count(),
            captured
        );
    }

    #[test]
    fn scan_only_mesh_matches_single_frame_and_accumulated_result() {
        let mut demo = Demo::new(DemoOptions {
            source_step: 0.3,
            scanner: ScannerConfig {
                noise_probability: 1.0,
                ..default()
            },
            ..default()
        });
        demo.show_context = false;
        let check = |demo: &Demo| {
            let mesh = demo.mesh();
            let Some(VertexAttributeValues::Float32x3(positions)) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                panic!("missing positions");
            };
            let expected: Vec<_> = demo.result().positions().map(|p| p.to_array()).collect();
            assert_eq!(positions, &expected);
            let Some(VertexAttributeValues::Float32x4(colors)) =
                mesh.attribute(Mesh::ATTRIBUTE_COLOR)
            else {
                panic!("missing colors");
            };
            assert!(!colors.contains(&[0.38, 0.40, 0.43, 1.0]));
            assert_eq!(
                colors
                    .iter()
                    .filter(|&&c| c == [1.0, 0.55, 0.05, 1.0])
                    .count(),
                demo.result()
                    .points
                    .iter()
                    .filter(|p| p.class == PointClass::Noise)
                    .count()
            );
        };
        check(&demo);
        let first = demo.frame.cloud.clone();
        demo.capture(demo.path[demo.path.len() / 2], false);
        demo.cumulative = true;
        assert!(demo.result().len() > first.len());
        check(&demo);
        let accumulated = demo.result().clone();
        demo.show_context = true;
        assert!(demo.mesh().count_vertices() > accumulated.len());
        assert_eq!(demo.result(), &accumulated);
        demo.show_context = false;
        demo.cumulative = false;
        check(&demo);
    }
}
