//! Плагин приложения-генератора растений.
//!
//! Строит сцену (свет, земля, панель информации), генерирует первую сцену
//! растений из [`GlobalSeed`] и дальше живёт в цикле:
//!
//! - `R` / `Shift + R` ([`RegenerationRequest`]) — перегенерация;
//! - `V` ([`ViewMode`]) — переключение «визуальные модели / облако точек»;
//! - `P` — следующий пресет растения (дерево → хвойное → куст → трава...);
//! - `M` — переключение режима генерации: одиночный / множественный.
//!
//! Управление `V`/`R` обеспечивает
//! [`GenerationControlsPlugin`](generation_core::GenerationControlsPlugin);
//! этот плагин лишь реагирует на его сообщения и ресурсы (`GlobalSeed`,
//! [`ViewMode`]), а `P` и `M` обрабатывает сам.

use bevy::asset::Assets;
use bevy::mesh::Mesh;
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::prelude::*;
use bevy_utils::cameras::OrbitCamera;
use bevy_utils::visuals::materials::material;
use bevy_utils::visuals::meshes::box_mesh;
use generation_core::general::RegenerationRequest;
use generation_core::point_clouds::visual::{point_cloud_material, point_cloud_mesh};
use generation_core::seeds::GlobalSeed;
use generation_core::{PointClass, PointCloud, Seed, ViewMode};

use crate::generation::{GenerationMode, PlantConfig, PlantPreset, PlantScene, resolve};
use crate::visual::{PointCloudEntity, VisualMesh, spawn_plant_scene};

/// Подсказка об управлении.
const HELP: &str =
    "[P] пресет  ·  [M] режим  ·  [V] вид  ·  [R] новый вид  ·  [Shift+R] тот же вид  ·  [Tab] камера";

/// Состояние текущей сцены растений.
#[derive(Resource, Default)]
pub struct PlantsState {
    /// Сгенерированная сцена (`None` — ещё не сгенерировано).
    pub scene: Option<PlantScene>,
    /// Корневая сущность сцены.
    pub root: Option<Entity>,
}

/// Маркер текстовой панели информации о растениях.
#[derive(Component)]
pub struct PlantsInfoText;

/// Плагин генератора растений с визуализацией.
pub struct PlantsPlugin {
    /// Начальный пресет (seed берётся из [`GlobalSeed`]).
    pub preset: PlantPreset,
}

impl Default for PlantsPlugin {
    fn default() -> Self {
        Self {
            preset: PlantPreset::DeciduousTree,
        }
    }
}

impl Plugin for PlantsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<RegenerationRequest>()
            .init_resource::<GlobalSeed>()
            .init_resource::<ViewMode>()
            .init_resource::<GenerationMode>()
            .init_resource::<PlantsState>()
            .insert_resource(self.preset)
            .add_systems(Startup, setup_scene)
            .add_systems(
                Update,
                (
                    cycle_preset,
                    toggle_generation_mode,
                    regenerate,
                    update_info,
                    apply_view_mode,
                    apply_light_shadows,
                )
                    .chain(),
            );
    }
}

/// `P` — следующий пресет растения, перегенерировать с тем же seed.
fn cycle_preset(
    keys: Res<ButtonInput<KeyCode>>,
    mut preset: ResMut<PlantPreset>,
    mut writer: MessageWriter<RegenerationRequest>,
) {
    if keys.just_pressed(KeyCode::KeyP) {
        *preset = preset.next();
        // Тот же seed — удобно сравнивать пресеты на одной «местности».
        writer.write(RegenerationRequest::same());
    }
}

/// `M` — переключение режима генерации (одиночный / множественный).
fn toggle_generation_mode(
    keys: Res<ButtonInput<KeyCode>>,
    mut mode: ResMut<GenerationMode>,
    mut writer: MessageWriter<RegenerationRequest>,
) {
    if keys.just_pressed(KeyCode::KeyM) {
        *mode = mode.toggled();
        writer.write(RegenerationRequest::same());
        info!("Режим генерации: {}", mode.label());
    }
}

/// `R` / `Shift + R` — перегенерация; затем спавн новой сцены.
#[allow(clippy::too_many_arguments)]
fn regenerate(
    mut commands: Commands,
    mut requests: MessageReader<RegenerationRequest>,
    mut global_seed: ResMut<GlobalSeed>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    preset: Res<PlantPreset>,
    mode: Res<GenerationMode>,
    mut state: ResMut<PlantsState>,
    mut orbit: Option<ResMut<OrbitCamera>>,
) {
    let mut regenerate = false;
    for request in requests.read() {
        if !request.same_seed {
            global_seed.regenerate();
        }
        regenerate = true;
    }
    if !regenerate {
        return;
    }

    let orbit = orbit.as_deref_mut();
    regenerate_plants(
        &mut commands,
        &mut meshes,
        &mut materials,
        global_seed.0,
        *preset,
        *mode,
        &mut state,
        orbit,
    );
}

/// Сцена: свет, земля, панель информации. Затем — первая сцена растений.
#[allow(clippy::too_many_arguments)]
fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    global_seed: Res<GlobalSeed>,
    preset: Res<PlantPreset>,
    mode: Res<GenerationMode>,
    mut state: ResMut<PlantsState>,
    mut orbit: Option<ResMut<OrbitCamera>>,
) {
    // Солнце с тенями.
    commands.spawn((
        DirectionalLight {
            illuminance: 5_000.0,
            color: Color::srgb(1.0, 0.96, 0.88),
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -1.0, 0.8, 0.0)),
    ));

    // Рассеянная подсветка.
    commands.spawn((
        DirectionalLight {
            illuminance: 1_000.0,
            color: Color::srgb(0.64, 0.74, 0.95),
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, 0.9, -0.5, 0.0)),
    ));
    commands.insert_resource(GlobalAmbientLight {
        brightness: 500.0,
        ..default()
    });

    // Земля: плоская «зелёная» плита (верх — на уровне y = 0).
    // В режиме облака точек скрывается (маркер VisualMesh) и заменяется
    // облаком точек земли.
    let ground = meshes.add(box_mesh(Vec3::new(200.0, 0.4, 200.0)));
    let ground_material = materials.add(material(Color::srgb(0.24, 0.46, 0.22)));
    commands.spawn((
        Mesh3d(ground),
        MeshMaterial3d(ground_material),
        VisualMesh,
        Transform::from_xyz(0.0, -0.2, 0.0),
    ));

    // Облако точек земли (ровная сетка на уровне y = 0).
    let ground_cloud = meshes.add(point_cloud_mesh(&ground_point_cloud()));
    let ground_cloud_material = materials.add(point_cloud_material());
    commands.spawn((
        Mesh3d(ground_cloud),
        MeshMaterial3d(ground_cloud_material),
        PointCloudEntity,
        Visibility::default(),
    ));

    spawn_info_panel(&mut commands, &asset_server);

    regenerate_plants(
        &mut commands,
        &mut meshes,
        &mut materials,
        global_seed.0,
        *preset,
        *mode,
        &mut state,
        orbit.as_deref_mut(),
    );
}

/// Панель информации в левом верхнем углу.
fn spawn_info_panel(commands: &mut Commands, asset_server: &AssetServer) {
    let font = asset_server.load("fonts/Roboto-Regular.ttf");
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                top: Val::Px(12.0),
                padding: UiRect::all(Val::Px(12.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.06, 0.07, 0.7)),
        ))
        .with_children(|parent| {
            parent.spawn((
                PlantsInfoText,
                Text::new(""),
                TextFont {
                    font: font.into(),
                    font_size: FontSize::Px(15.0),
                    ..default()
                },
                TextColor(Color::srgb(0.92, 0.95, 0.97)),
            ));
        });
}

/// Перегенерировать сцену растений: убрать старую, сгенерировать новую
/// из `seed` и подстроить камеру под габариты сцены.
#[allow(clippy::too_many_arguments)]
fn regenerate_plants(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    seed: Seed,
    preset: PlantPreset,
    mode: GenerationMode,
    state: &mut PlantsState,
    orbit: Option<&mut OrbitCamera>,
) {
    if let Some(root) = state.root.take() {
        commands.entity(root).despawn();
    }

    let config: PlantConfig = resolve(preset, seed);
    let scene = PlantScene::generate(&config, mode);
    let spawned = spawn_plant_scene(commands, meshes, materials, &scene);

    if let Some(orbit) = orbit {
        // Высота в мировых осях Y = локальной Z сцены.
        orbit.focus = Vec3::new(0.0, scene.max_height * 0.4, 0.0);
        orbit.radius = (scene.extent * 1.4).clamp(4.0, 400.0);
        orbit.pitch = 0.55;
    }

    state.scene = Some(scene);
    state.root = Some(spawned.root);

    info!("Сцена растений сгенерирована: seed = {seed}");
}

/// Обновить панель информации при смене сцены, режима или вида.
fn update_info(
    mut text: Query<&mut Text, With<PlantsInfoText>>,
    state: Res<PlantsState>,
    view_mode: Res<ViewMode>,
) {
    if !state.is_changed() && !view_mode.is_changed() {
        return;
    }
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let Some(scene) = &state.scene else {
        return;
    };

    let mut lines = Vec::new();
    lines.push(format!(
        "Растения · {} · seed {}",
        scene.preset.label(),
        scene.seed
    ));
    lines.push(format!("Режим: {} · объектов: {}", scene.mode.label(), scene.object_count()));

    // Состав по типу.
    match scene.preset.plant_type() {
        crate::generation::PlantType::Grass => {
            let blades: usize = scene.tufts.iter().map(|t| t.tuft.blade_count).sum();
            lines.push(format!("Пучков травы: {} · травинок: {}", scene.tufts.len(), blades));
        }
        crate::generation::PlantType::Tree | crate::generation::PlantType::Bush => {
            let branches: usize = scene.plants.iter().map(|p| p.plant.branch_count).sum();
            let leaves: usize = scene.plants.iter().map(|p| p.plant.leaf_count).sum();
            lines.push(format!("Веток: {branches} · листьев/игл: {leaves}"));
        }
    }

    lines.push(format!(
        "Облако точек: {} · размах {:.1} м · высота {:.1} м",
        scene.cloud_points, scene.extent, scene.max_height
    ));

    lines.push(String::new());
    lines.push(HELP.to_string());
    lines.push(format!("Режим вида: {}", view_mode.label()));

    text.0 = lines.join("\n");
}

/// Применить текущий [`ViewMode`] к сущностям.
#[allow(clippy::type_complexity)]
fn apply_view_mode(
    view_mode: Res<ViewMode>,
    mut params: ParamSet<(
        Query<&mut Visibility, With<VisualMesh>>,
        Query<&mut Visibility, With<PointCloudEntity>>,
    )>,
) {
    let show_visual = *view_mode == ViewMode::Visual;
    for mut visibility in params.p0() {
        let target = if show_visual {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if *visibility != target {
            *visibility = target;
        }
    }
    for mut visibility in params.p1() {
        let target = if show_visual {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
        if *visibility != target {
            *visibility = target;
        }
    }
}

/// В режиме облака точек тени отключаем.
fn apply_light_shadows(view_mode: Res<ViewMode>, mut light: Query<&mut DirectionalLight>) {
    let enabled = *view_mode == ViewMode::Visual;
    for mut light in &mut light {
        if light.shadow_maps_enabled != enabled {
            light.shadow_maps_enabled = enabled;
        }
    }
}

/// Облако точек земли: ровная сетка точек на уровне y = 0.
fn ground_point_cloud() -> PointCloud {
    let extent = 60.0; // полуразмер, м
    let step = 1.5; // шаг сетки, м
    let mut cloud = PointCloud::new(PointClass::Ground);
    let mut z = -extent;
    while z <= extent {
        let mut x = -extent;
        while x <= extent {
            cloud.push(Vec3::new(x, 0.0, z));
            x += step;
        }
        z += step;
    }
    cloud
}
