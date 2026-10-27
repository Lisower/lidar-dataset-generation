//! Плагин приложения-генератора зданий.
//!
//! Строит сцену (свет, земля, панель информации), генерирует первое здание
//! из [`GlobalSeed`] и дальше живёт в цикле:
//!
//! - `R` / `Shift + R` ([`RegenerationRequest`]) — перегенерация;
//! - `V` ([`ViewMode`]) — переключение «визуальные модели / облако точек».
//!
//! Управление клавишами обеспечивает [`GenerationControlsPlugin`]; этот плагин
//! лишь реагирует на его сообщения и ресурсы (`GlobalSeed`, `ViewMode`).
//! При отсутствии плагина камер ([`CameraPlugin`](bevy_utils::cameras::CameraPlugin))
//! плагин работает, но не подстраивает камеру под габариты здания.

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

use crate::generation::{Building, BuildingConfig, BuildingPreset};
use crate::visual::{PointCloudEntity, VisualMesh, spawn_building};

/// Подсказка об управлении.
const HELP: &str =
    "[V] вид  ·  [R] новый вид  ·  [Shift+R] тот же вид  ·  [Tab] камера";

/// Состояние текущего здания: данные и корень его дерева сущностей.
#[derive(Resource, Default)]
pub struct BuildingState {
    /// Сгенерированное здание (данные; `None` — ещё не сгенерировано).
    pub building: Option<Building>,
    /// Корневая сущность дерева здания ( `None` — ещё не создана).
    pub root: Option<Entity>,
}

/// Маркер текстовой панели информации о здании.
#[derive(Component)]
pub struct BuildingInfoText;

/// Плагин генератора зданий с визуализацией.
pub struct BuildingPlugin {
    /// Базовая конфигурация генерации (seed берётся из [`GlobalSeed`]).
    pub config: BuildingConfig,
}

impl Default for BuildingPlugin {
    fn default() -> Self {
        Self {
            config: BuildingConfig::default(),
        }
    }
}

impl Plugin for BuildingPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<RegenerationRequest>()
            .init_resource::<GlobalSeed>()
            .init_resource::<ViewMode>()
            .init_resource::<BuildingState>()
            .insert_resource(self.config.clone())
            .add_systems(Startup, setup_scene)
            .add_systems(
                Update,
                (regenerate, update_info, apply_view_mode, apply_light_shadows).chain(),
            );
    }
}

/// `R` / `Shift + R` — перегенерация здания; `V` обрабатывает
/// [`GenerationControlsPlugin`], а этот плагин применяет новый [`ViewMode`]
/// к сущностям.
fn regenerate(
    mut commands: Commands,
    mut requests: MessageReader<RegenerationRequest>,
    mut global_seed: ResMut<GlobalSeed>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut config: ResMut<BuildingConfig>,
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<BuildingState>,
    mut orbit: Option<ResMut<OrbitCamera>>,
) {
    let mut regenerate = keys.just_pressed(KeyCode::KeyP);
    if regenerate {
        let index = BuildingPreset::ALL.iter().position(|p| *p == config.preset).unwrap_or(0);
        config.preset = BuildingPreset::ALL[(index + 1) % BuildingPreset::ALL.len()];
    }
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
    regenerate_building(
        &mut commands,
        &mut meshes,
        &mut materials,
        global_seed.0,
        &config,
        &mut state,
        orbit,
    );
}

/// Сцена: свет, земля, панель информации. Затем — первое здание.
fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    global_seed: Res<GlobalSeed>,
    config: Res<BuildingConfig>,
    mut state: ResMut<BuildingState>,
    mut orbit: Option<ResMut<OrbitCamera>>,
) {
    // Солнце с тенями.
    commands.spawn((
        DirectionalLight {
            illuminance: 5_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -1.0, 0.8, 0.0)),
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

    // Облако точек земли (ровная сетка на уровне y = 0) — для режима
    // облака точек.
    let ground_cloud = meshes.add(point_cloud_mesh(&ground_point_cloud()));
    let ground_cloud_material = materials.add(point_cloud_material());
    commands.spawn((
        Mesh3d(ground_cloud),
        MeshMaterial3d(ground_cloud_material),
        PointCloudEntity,
        Visibility::default(),
    ));

    // Панель информации.
    spawn_info_panel(&mut commands, &asset_server);

    // Первое здание.
    regenerate_building(
        &mut commands,
        &mut meshes,
        &mut materials,
        global_seed.0,
        &config,
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
                BuildingInfoText,
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

/// Облако точек земли: ровная сетка точек на уровне y = 0 (в режиме облака
/// точек земля показывается точками, а не мешем).
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

/// Перегенерировать здание: убрать старое, сгенерировать новое из `seed`
/// и подстроить камеру под его габариты.
fn regenerate_building(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    seed: Seed,
    config: &BuildingConfig,
    state: &mut BuildingState,
    orbit: Option<&mut OrbitCamera>,
) {
    if let Some(root) = state.root.take() {
        commands.entity(root).despawn();
    }

    let mut gen_config = config.clone();
    gen_config.seed = seed;
    let building = Building::generate(&gen_config);
    let spawned = spawn_building(commands, meshes, materials, &building);

    if let Some(orbit) = orbit {
        // Здание стоит вертикально вдоль мирового Y.
        orbit.focus = Vec3::new(0.0, building.total_height() * 0.45, 0.0);
        orbit.radius = (building.footprint_diagonal().max(building.total_height()) * 1.7).clamp(8.0, 400.0);
        orbit.pitch = 0.55;
    }

    state.building = Some(building);
    state.root = Some(spawned.root);

    info!("Здание сгенерировано: seed = {seed}");
}

/// Обновить панель информации при смене здания или режима отображения.
fn update_info(
    mut text: Query<&mut Text, With<BuildingInfoText>>,
    state: Res<BuildingState>,
    view_mode: Res<ViewMode>,
) {
    if !state.is_changed() && !view_mode.is_changed() {
        return;
    }
    let Ok(mut text) = text.single_mut() else {
        return;
    };

    let Some(building) = &state.building else {
        return;
    };

    let mut lines = Vec::new();
    lines.push(format!("Preset: {} [P]", building.config.preset.label()));
    lines.push(format!("Здание · seed {}", building.seed));
    lines.push(format!(
        "Размеры {:.1} × {:.1} м · {} · высота стен {:.1} м",
        building.size.x,
        building.size.y,
        floors_word(building.floors),
        building.size.z,
    ));
    lines.push(format!(
        "Крыша: {} (наклон {:.0}°), свес {:.2} м",
        building.roof.label(),
        building.roof_pitch_deg,
        building.overhang,
    ));

    let mut extras = Vec::new();
    if building.parapet {
        extras.push("парапет");
    }
    if building.dormer {
        extras.push("мансарда");
    }
    lines.push(if extras.is_empty() {
        "Доп. элементы: нет".to_string()
    } else {
        format!("Доп. элементы: {}", extras.join(", "))
    });
    lines.push(format!(
        "Облако точек: {} точек",
        building.point_cloud.points.len()
    ));
    lines.push(String::new());
    lines.push(HELP.to_string());
    lines.push(format!("Режим: {}", view_mode.label()));

    text.0 = lines.join("\n");
}

/// Применить текущий [`ViewMode`] к сущностям здания.
fn apply_view_mode(
    view_mode: Res<ViewMode>,
    mut params: ParamSet<(
        // Два запроса на один и тот же компонент — чтобы не ссорились
        // между собой, собираем их в `ParamSet`.
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

/// В режиме облака точек тени отключаем: точки не должны «давать» тени
/// (а на облаке они и не нужны). В визуальном режиме тени включены, чтобы
/// здание выглядело объёмно.
fn apply_light_shadows(
    view_mode: Res<ViewMode>,
    mut light: Query<&mut DirectionalLight>,
) {
    let enabled = *view_mode == ViewMode::Visual;
    for mut light in &mut light {
        if light.shadow_maps_enabled != enabled {
            light.shadow_maps_enabled = enabled;
        }
    }
}

/// Русское слово «этаж» с правильным склонением по числу.
fn floors_word(floors: u32) -> String {
    match floors % 10 {
        1 if floors % 100 != 11 => format!("{floors} этаж"),
        2..=4 if !(12..=14).contains(&(floors % 100)) => format!("{floors} этажа"),
        _ => format!("{floors} этажей"),
    }
}
