//! Плагин приложения-генератора ландшафта.
//!
//! Строит сцену (свет, панель информации), генерирует первый ландшафт из
//! [`GlobalSeed`] и дальше живёт в цикле:
//!
//! - `R` / `Shift + R` ([`RegenerationRequest`]) — перегенерация;
//! - `V` ([`ViewMode`]) — переключение «визуальные модели / облако точек»;
//! - `P` — следующий пресет ландшафта (равнина → склон → … → водопады);
//! - `Tab` — переключение камеры (обеспечивает `bevy-utils`).
//!
//! Управление `V`/`R` обеспечивает
//! [`GenerationControlsPlugin`](generation_core::GenerationControlsPlugin);
//! этот плагин лишь реагирует на его сообщения и ресурсы (`GlobalSeed`,
//! [`ViewMode`]), а `P` обрабатывает сам.

use bevy::asset::Assets;
use bevy::mesh::Mesh;
use bevy::pbr::StandardMaterial;
use bevy::prelude::*;
use bevy_utils::cameras::OrbitCamera;
use generation_core::general::RegenerationRequest;
use generation_core::seeds::GlobalSeed;
use generation_core::{Seed, ViewMode};

use crate::generation::landscape::generate;
use crate::generation::{Landscape, LandscapeConfig, LandscapePreset, resolve};
use crate::visual::{PointCloudEntity, VisualMesh, spawn_landscape};

/// Подсказка об управлении.
const HELP: &str =
    "[P] пресет  ·  [V] вид  ·  [R] новый вид  ·  [Shift+R] тот же вид  ·  [Tab] камера";

/// Состояние текущего ландшафта.
#[derive(Resource, Default)]
pub struct LandscapeState {
    /// Сгенерированный ландшафт (`None` — ещё не сгенерировано).
    pub landscape: Option<Landscape>,
    /// Корневая сущность ландшафта.
    pub root: Option<Entity>,
}

/// Маркер текстовой панели информации о ландшафте.
#[derive(Component)]
pub struct LandscapeInfoText;

/// Плагин генератора ландшафта с визуализацией.
#[derive(Default)]
pub struct LandscapePlugin {
    /// Начальный пресет (seed берётся из [`GlobalSeed`]).
    pub preset: LandscapePreset,
}

impl Plugin for LandscapePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<RegenerationRequest>()
            .init_resource::<GlobalSeed>()
            .init_resource::<ViewMode>()
            .init_resource::<LandscapeState>()
            .insert_resource(self.preset)
            .add_systems(Startup, setup_scene)
            .add_systems(
                Update,
                (
                    cycle_preset,
                    regenerate,
                    update_info,
                    apply_view_mode,
                    apply_light_shadows,
                )
                    .chain(),
            );
    }
}

/// `P` — следующий пресет ландшафта, перегенерировать с тем же seed.
fn cycle_preset(
    keys: Res<ButtonInput<KeyCode>>,
    mut preset: ResMut<LandscapePreset>,
    mut writer: MessageWriter<RegenerationRequest>,
) {
    if keys.just_pressed(KeyCode::KeyP) {
        *preset = preset.next();
        // Тот же seed — удобно сравнивать пресеты на одной «местности».
        writer.write(RegenerationRequest::same());
    }
}

/// `R` / `Shift + R` — перегенерация; затем спавн нового ландшафта.
#[allow(clippy::too_many_arguments)]
fn regenerate(
    mut commands: Commands,
    mut requests: MessageReader<RegenerationRequest>,
    mut global_seed: ResMut<GlobalSeed>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    preset: Res<LandscapePreset>,
    mut state: ResMut<LandscapeState>,
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
    regenerate_landscape(
        &mut commands,
        &mut meshes,
        &mut materials,
        global_seed.0,
        *preset,
        &mut state,
        orbit,
    );
}

/// Сцена: свет, панель информации. Затем — первый ландшафт.
#[allow(clippy::too_many_arguments)]
fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    global_seed: Res<GlobalSeed>,
    preset: Res<LandscapePreset>,
    mut state: ResMut<LandscapeState>,
    mut orbit: Option<ResMut<OrbitCamera>>,
) {
    // Солнце с тенями.
    commands.spawn((
        DirectionalLight {
            illuminance: 8_000.0,
            color: Color::srgb(1.0, 0.96, 0.88),
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -1.0, 0.8, 0.0)),
    ));

    // Рассеянная подсветка.
    commands.spawn((
        DirectionalLight {
            illuminance: 1_200.0,
            color: Color::srgb(0.64, 0.74, 0.95),
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, 0.9, -0.5, 0.0)),
    ));
    commands.insert_resource(GlobalAmbientLight {
        brightness: 450.0,
        ..default()
    });

    spawn_info_panel(&mut commands, &asset_server);

    regenerate_landscape(
        &mut commands,
        &mut meshes,
        &mut materials,
        global_seed.0,
        *preset,
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
            BackgroundColor(Color::srgba(0.04, 0.06, 0.07, 0.72)),
        ))
        .with_children(|parent| {
            parent.spawn((
                LandscapeInfoText,
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

/// Перегенерировать ландшафт: убрать старый, сгенерировать новый из `seed`,
/// подстроить камеру под габариты карты.
#[allow(clippy::too_many_arguments)]
fn regenerate_landscape(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    seed: Seed,
    preset: LandscapePreset,
    state: &mut LandscapeState,
    orbit: Option<&mut OrbitCamera>,
) {
    if let Some(root) = state.root.take() {
        commands.entity(root).despawn();
    }

    let config: LandscapeConfig = resolve(preset, seed);
    let landscape = generate(&config);
    let spawned = spawn_landscape(commands, meshes, materials, &landscape);

    if let Some(orbit) = orbit {
        // Высота в мировых осях Y = локальной Z сцены (корень наклонён на -90°).
        let mid = (landscape.min_height + landscape.max_height) * 0.5;
        orbit.focus = Vec3::new(0.0, mid.clamp(-20.0, 60.0) * 0.6, 0.0);
        orbit.radius = (landscape.heightfield.size_m * 0.85).clamp(8.0, 400.0);
        orbit.pitch = 0.58;
    }

    state.landscape = Some(landscape);
    state.root = Some(spawned.root);

    info!("Ландшафт сгенерирован: {preset:?}, seed = {seed}");
}

/// Обновить панель информации при смене ландшафта или вида.
fn update_info(
    mut text: Query<&mut Text, With<LandscapeInfoText>>,
    state: Res<LandscapeState>,
    view_mode: Res<ViewMode>,
) {
    if !state.is_changed() && !view_mode.is_changed() {
        return;
    }
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let Some(landscape) = &state.landscape else {
        return;
    };

    let mut lines = Vec::new();
    lines.push(format!(
        "Ландшафт · {} · seed {}",
        landscape.config.label, landscape.seed
    ));

    let (h_lo, h_hi) = (landscape.min_height, landscape.max_height);
    lines.push(format!(
        "Карта {:.0}×{:.0} м · рельеф {:.1}…{:.1} м",
        landscape.heightfield.size_m, landscape.heightfield.size_m, h_lo, h_hi
    ));

    // Вода.
    if landscape.has_water {
        let w_lo = landscape
            .water
            .levels
            .iter()
            .flatten()
            .fold(f32::INFINITY, |acc, &w| acc.min(w));
        let w_hi = landscape
            .water
            .levels
            .iter()
            .flatten()
            .fold(f32::NEG_INFINITY, |acc, &w| acc.max(w));
        lines.push(format!("Вода: {:.1}…{:.1} м", w_lo, w_hi));
    } else {
        lines.push("Вода: нет".to_string());
    }

    // Каменистость (частота и амплитуда шума).
    lines.push(format!(
        "Каменистость: шум {:.2} м при частоте {:.3} (1/м)",
        landscape.config.noise.amplitude, landscape.config.noise.frequency
    ));

    lines.push(format!(
        "Точки: грунт {} · вода {}",
        landscape.ground_cloud.points.len(),
        landscape.water_cloud.points.len()
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
