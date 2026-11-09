//! Интерактивная демонстрация и пакетный экспорт без графического окна.
use crate::trajectory::{SceneBounds, TrajectoryKind};
use crate::{ScanAccumulator, ScanFrame, ScanScene, ScannerConfig, ScannerPose};
use bevy::prelude::*;
use bevy_utils::cameras::{CameraPlugin, MainCamera, OrbitCamera};
use bevy_utils::preset_selector::PresetSelectorPlugin;
use buildings_generator::{Building, BuildingConfig};
use generation_core::{
    PointClass, PointCloud, PointCloudRenderPlugin, PointCloudTag, Seed,
    point_clouds::visual::{ATTRIBUTE_POINT_SIZE, point_cloud_mesh},
};
use generation_core::{UiModalOpen, general::RegenerationRequest, presets::PresetStore};
use rand::RngExt;
use std::{
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct DemoOptions {
    pub seed: Seed,
    pub source_step: f32,
    pub scanner: ScannerConfig,
    pub frames: usize,
    pub output: String,
    /// Полный id: buildings/cabin, plants/fir и т.д.
    pub preset: Option<String>,
    pub trajectory: TrajectoryKind,
    pub auto_fov: bool,
}

impl Default for DemoOptions {
    fn default() -> Self {
        Self {
            seed: Seed::default(),
            source_step: 0.3,
            scanner: ScannerConfig::default(),
            frames: 24,
            output: "assets/export/scan.xyz".into(),
            preset: None,
            trajectory: TrajectoryKind::Orbit,
            auto_fov: true,
        }
    }
}

#[derive(Resource)]
struct Demo {
    options: DemoOptions,
    scene: ScanScene,
    bounds: SceneBounds,
    preset_label: String,
    active_scanner: ScannerConfig,
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
    #[cfg(test)]
    fn new(options: DemoOptions) -> Self {
        Self::from_store(options, None).unwrap()
    }

    fn from_store(options: DemoOptions, store: Option<&PresetStore>) -> Result<Self, String> {
        let meta = if let Some(store) = store {
            if let Some(id) = &options.preset {
                Some(
                    store
                        .meta(id)
                        .ok_or_else(|| format!("Неизвестный пресет: {id}"))?,
                )
            } else {
                let mut choices: Vec<_> = store
                    .presets
                    .iter()
                    .filter(|p| store.selected.contains(&p.id))
                    .collect();
                choices.sort_by(|a, b| a.id.cmp(&b.id));
                if choices.is_empty() {
                    return Err("Выберите объекты в Ctrl+S".into());
                }
                let mut rng = options.seed.with_label("scan-source").rng();
                Some(choices[rng.random_range(0..choices.len())])
            }
        } else {
            None
        };
        let (scene, preset_label) = if let Some(meta) = meta {
            (
                crate::sources::from_preset(
                    meta,
                    &store.unwrap().root,
                    options.seed,
                    options.source_step,
                )?,
                meta.id.clone(),
            )
        } else {
            let building = Building::generate(&BuildingConfig {
                seed: options.seed,
                point_step: options.source_step,
                wall_point_step: options.source_step,
                point_jitter: 0.0,
                ..default()
            });
            (ScanScene::from_building(&building), "building".into())
        };
        let bounds = SceneBounds::from_cloud(&scene.cloud).ok_or("Пустая сцена")?;
        let path = bounds.trajectory(options.trajectory, options.frames, options.seed);
        let pose = path[0];
        let active_scanner = if options.auto_fov {
            bounds.fit_config(pose, &options.scanner)
        } else {
            options.scanner.clone()
        };
        let frame = scene
            .scan(pose, &active_scanner, options.seed.child(0))
            .expect("validated options");
        let mut accumulated = ScanAccumulator::new(scene.cloud.len());
        accumulated.add(&frame);
        Ok(Self {
            options,
            bounds,
            preset_label,
            active_scanner,
            scene,
            path,
            index: 0,
            pose,
            frame,
            accumulated,
            cumulative: false,
            show_context: true,
            running: false,
            timer: Timer::from_seconds(0.5, TimerMode::Repeating),
            status: String::new(),
        })
    }

    fn capture(&mut self, pose: ScannerPose, reset: bool) {
        self.status.clear();
        if reset {
            self.accumulated = ScanAccumulator::new(self.scene.cloud.len());
        }
        self.pose = pose;
        self.active_scanner = if self.options.auto_fov {
            self.bounds.fit_config(pose, &self.options.scanner)
        } else {
            self.options.scanner.clone()
        };
        self.frame = self
            .scene
            .scan(
                pose,
                &self.active_scanner,
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
            return point_cloud_mesh(self.result());
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
        let mut sizes: Vec<f32> = self
            .scene
            .cloud
            .points
            .iter()
            .map(|point| point.class.point_size())
            .collect();
        for point in &self.result().points {
            if point.class == PointClass::LowNoise {
                positions.push(point.position.to_array());
                let c = point.class.color().to_srgba();
                colors.push([c.red, c.green, c.blue, 1.0]);
                sizes.push(PointClass::LowNoise.point_size());
            }
        }
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
        mesh.insert_attribute(ATTRIBUTE_POINT_SIZE, sizes);
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
    if !matches!(
        Path::new(&options.output)
            .extension()
            .and_then(|s| s.to_str()),
        Some("xyz" | "ply")
    ) {
        return Err("output должен иметь расширение .xyz или .ply");
    }
    Ok(())
}

pub fn preset_selector() -> PresetSelectorPlugin {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let roots = generation_core::presets::resolve_data_roots(
        "",
        manifest.join("../..").join("presets"),
        manifest.join("settings"),
    );
    PresetSelectorPlugin {
        category: "",
        presets_root: roots.presets_root,
        settings_dir: roots.settings_dir.join("lidar-simulator"),
    }
}

/// Общий XYZ-экспорт generation-core; PLY оставлен для существующих сценариев.
pub fn export_scan(cloud: &PointCloud, path: impl AsRef<Path>) -> std::io::Result<()> {
    let path = path.as_ref();
    match path.extension().and_then(|s| s.to_str()) {
        Some("xyz") => generation_core::point_clouds::export::write_xyz(cloud, path),
        Some("ply") => export_ply(cloud, path),
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Ожидается .xyz или .ply",
        )),
    }
}

fn regenerate_selected(
    mut requests: MessageReader<RegenerationRequest>,
    store: Res<PresetStore>,
    mut demo: ResMut<Demo>,
    mut orbit: ResMut<OrbitCamera>,
) {
    if requests.read().count() == 0 {
        return;
    }
    let mut options = demo.options.clone();
    options.preset = None;
    match Demo::from_store(options, Some(&store)) {
        Ok(mut next) => {
            next.show_context = demo.show_context;
            orbit.focus = next.bounds.center();
            orbit.radius = next.bounds.radius() * 2.8;
            *demo = next;
        }
        Err(error) => demo.status = error,
    }
}

/// Только измеренные точки, без серого контекста. Класс LowNoise = 7.
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
        let class = point.class.code();
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
    let selector = preset_selector();
    let mut store = PresetStore::discover_all(&selector.presets_root);
    store.load_selection(&selector.settings_dir);
    let mut demo = Demo::from_store(options, Some(&store))?;
    demo.accumulated = ScanAccumulator::new(demo.scene.cloud.len());
    for index in 0..demo.path.len() {
        demo.capture(demo.path[index], false);
    }
    export_scan(&demo.accumulated.cloud, &demo.options.output)?;
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

pub fn run(options: DemoOptions) -> Result<(), Box<dyn std::error::Error>> {
    validate_options(&options)?;
    // Ассеты ищем рядом с exe/в рабочем каталоге (переносимая сборка), а путь
    // сборочной машины оставляем запасным для `cargo run`.
    let assets_root = generation_core::presets::resolve_dir(
        "assets",
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("buildings-generator")
            .join("assets"),
    );
    let selector = preset_selector();
    let mut store = PresetStore::discover_all(&selector.presets_root);
    store.load_selection(&selector.settings_dir);
    let demo = Demo::from_store(options, Some(&store))?;
    let focus = demo.bounds.center();
    let radius = demo.bounds.radius() * 2.8;
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.025, 0.035, 0.05)))
        .insert_resource(demo)
        .add_message::<RegenerationRequest>()
        .add_plugins((
            DefaultPlugins.set(AssetPlugin {
                file_path: assets_root.to_string_lossy().into_owned(),
                ..default()
            }),
            PointCloudRenderPlugin,
            CameraPlugin::new(focus, radius),
            selector,
        ))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (controls, regenerate_selected, refresh, draw_scanner).chain(),
        )
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
    assets: Res<AssetServer>,
) {
    commands.spawn((
        CloudView,
        Mesh3d(meshes.add(demo.mesh())),
        PointCloudTag,
        Visibility::default(),
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
    ui: Res<UiModalOpen>,
    store: Res<PresetStore>,
    mut orbit: ResMut<OrbitCamera>,
) {
    if ui.0 {
        return;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        let mut options = demo.options.clone();
        if !keys.pressed(KeyCode::ShiftLeft) && !keys.pressed(KeyCode::ShiftRight) {
            options.seed = Seed::from_entropy();
        }
        let show_context = demo.show_context;
        match Demo::from_store(options, Some(&store)) {
            Ok(mut next) => {
                next.show_context = show_context;
                orbit.focus = next.bounds.center();
                orbit.radius = next.bounds.radius() * 2.8;
                *demo = next;
            }
            Err(error) => demo.status = error,
        }
    }
    let mut recapture = false;
    if keys.just_pressed(KeyCode::KeyL) {
        demo.options.trajectory = if demo.options.trajectory == TrajectoryKind::Orbit {
            TrajectoryKind::Line
        } else {
            TrajectoryKind::Orbit
        };
        demo.path = demo.bounds.trajectory(
            demo.options.trajectory,
            demo.options.frames,
            demo.options.seed,
        );
        demo.index = 0;
        demo.pose = demo.path[0];
        recapture = true;
    }
    if keys.just_pressed(KeyCode::KeyF) {
        demo.options.auto_fov = !demo.options.auto_fov;
        recapture = true;
    }
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
        demo.status = match export_scan(demo.result(), &demo.options.output) {
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
            "LiDAR · seed {} · ракурс {}/{}\nИсходное облако: {} · шаг отдельных объектов {:.2} м\nШаг в плоскости 10 м: {:.2} м · Low Noise: {:.1}%\nРежим: {} · кадров накоплено: {}\nТочек в результате: {} · шумовых: {}\nОтображение: {} · высота сканера {:.1} м\n\nОтсканированные — цвет класса · серые — не отсканированы · Low Noise — цвет класса\n←/→ ракурс · Space кадр из камеры · T траектория · P пауза\nV всё/отсканированное · M кадр/накопление · C сброс\n[/] шаг · N шум · X сохранить XYZ/PLY\nR новый объект · Shift+R повтор · Tab камера · мышь вращение/зум\nCtrl+S пресеты · L орбита/прямая · F авто-FOV\nКамера — осмотр; Space — переснять с её позиции\n{} · {:?} · авто-FOV {} · {:.1}° × {:.1}°\n{}",
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
                .filter(|p| p.class == PointClass::LowNoise)
                .count(),
            if demo.show_context {
                "всё облако"
            } else {
                "только отсканированное"
            },
            demo.pose.position.y,
            demo.preset_label,
            demo.options.trajectory,
            demo.options.auto_fov,
            demo.active_scanner.horizontal_fov.to_degrees(),
            demo.active_scanner.vertical_fov.to_degrees(),
            demo.status
        );
    }
}

fn draw_scanner(demo: Res<Demo>, mut gizmos: Gizmos) {
    let color = Color::srgb(0.1, 0.85, 1.0);
    let segments = if demo.options.trajectory == TrajectoryKind::Orbit {
        demo.path.len()
    } else {
        demo.path.len().saturating_sub(1)
    };
    for i in 0..segments {
        gizmos.line(
            demo.path[i].position,
            demo.path[(i + 1) % demo.path.len()].position,
            Color::srgb(0.15, 0.3, 0.4),
        );
    }
    let pose = demo.pose;
    let h = (demo.active_scanner.horizontal_fov * 0.5).tan() * 3.0;
    let v = (demo.active_scanner.vertical_fov * 0.5).tan() * 3.0;
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
        let Some(VertexAttributeValues::Float32(sizes)) = mesh.attribute(ATTRIBUTE_POINT_SIZE)
        else {
            panic!("missing point sizes");
        };
        let captured = demo.frame.source_indices.len();
        assert!(captured > 0 && captured < demo.scene.cloud.len());
        assert_eq!(mesh.count_vertices(), demo.scene.cloud.len() + captured);
        assert_eq!(sizes.len(), mesh.count_vertices());
        let low_noise_size = PointClass::LowNoise.point_size();
        assert_eq!(
            sizes.iter().filter(|&&size| size == low_noise_size).count(),
            captured
        );
        assert!(low_noise_size >= 2.0 * PointClass::Building.point_size());
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
                .filter(|&&c| {
                    let color = PointClass::LowNoise.color().to_srgba();
                    c == [color.red, color.green, color.blue, 1.0]
                })
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
                    .filter(|&&c| {
                        let color = PointClass::LowNoise.color().to_srgba();
                        c == [color.red, color.green, color.blue, 1.0]
                    })
                    .count(),
                demo.result()
                    .points
                    .iter()
                    .filter(|p| p.class == PointClass::LowNoise)
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
