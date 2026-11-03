//! Composition only: reuses the team's existing Bevy camera, controls and renderers.
//! No custom terrain, plant, building or point-cloud renderer is implemented here.

use std::collections::BTreeMap;
use std::f32::consts::FRAC_PI_2;

use bevy::prelude::*;
use bevy_utils::cameras::{OrbitCamera, map_overview_radius};
use bevy_utils::visuals::lighting::sun_shadows;
use buildings_generator::visual::{
    PointCloudEntity as BuildingCloud, VisualMesh as BuildingMesh, spawn_building,
};
use generation_core::general::RegenerationRequest;
use generation_core::presets::{PresetStore, load_preset_config};
use generation_core::seeds::GlobalSeed;
use generation_core::{Seed, ViewMode};
use landscapes_generator::visual::{
    PointCloudEntity as LandscapeCloud, VisualMesh as LandscapeMesh, spawn_landscape,
};
use plants_generator::visual::{
    PointCloudEntity as PlantCloud, VisualMesh as PlantMesh, spawn_plant_scene,
};
use plants_generator::{GenerationMode, PlacedPlant, PlacedTuft, PlantPreset, PlantScene};
use terrains_generator::{TerrainPreset, TerrainPresetData, TerrainScene, generate, generate_from_data};

/// Подсказка об управлении.
const HELP: &str =
    "[Ctrl+S] пресеты  ·  [V] вид  ·  [R] новый вид  ·  [Shift+R] тот же вид  ·  [Tab] камера";

#[derive(Resource)]
struct SelectedPreset(TerrainPreset);

#[derive(Default, Resource)]
struct TerrainViewerState {
    root: Option<Entity>,
    /// Сгенерированная сцена (нужна панели информации).
    scene: Option<TerrainScene>,
    /// Русское название текущего пресета из манифеста.
    preset_name: String,
}

/// Маркер текстовой панели информации о территории.
#[derive(Component)]
struct TerrainInfoText;

pub struct TerrainViewerPlugin {
    pub initial_preset: TerrainPreset,
}

impl Plugin for TerrainViewerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SelectedPreset(self.initial_preset))
            .init_resource::<TerrainViewerState>()
            .add_systems(Startup, setup)
            .add_systems(Update, (regenerate, update_info, apply_view_mode).chain());
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    seed: Res<GlobalSeed>,
    preset: Res<SelectedPreset>,
    store: Option<Res<PresetStore>>,
    mut state: ResMut<TerrainViewerState>,
) {
    // Same daylight configuration as the project's landscape viewer.
    commands.spawn((
        DirectionalLight {
            illuminance: 8_000.0,
            color: Color::srgb(1.0, 0.96, 0.88),
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -1.0, 0.8, 0.0)),
        sun_shadows(),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 1_200.0,
            color: Color::srgb(0.64, 0.74, 0.95),
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, 0.9, -0.5, 0.0)),
        sun_shadows(),
    ));
    commands.insert_resource(GlobalAmbientLight {
        brightness: 450.0,
        ..default()
    });

    spawn_info_panel(&mut commands, &asset_server);

    build_scene(
        &mut commands,
        &mut meshes,
        &mut materials,
        preset.0,
        seed.0,
        store.as_deref(),
        &mut state,
        // При первом запуске сохраняем обзорный ракурс камеры; подстраиваем
        // расстояние только при перегенерации (R / выбор пресета).
        None,
    );
}

#[allow(clippy::too_many_arguments)]
fn regenerate(
    mut requests: MessageReader<RegenerationRequest>,
    mut commands: Commands,
    mut seed: ResMut<GlobalSeed>,
    preset: Res<SelectedPreset>,
    store: Option<Res<PresetStore>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut state: ResMut<TerrainViewerState>,
    mut orbit: Option<ResMut<OrbitCamera>>,
) {
    let mut refresh = false;
    // R / Shift+R are emitted by the team's GenerationControlsPlugin.
    for request in requests.read() {
        if !request.same_seed {
            seed.regenerate();
        }
        refresh = true;
    }
    if !refresh {
        return;
    }

    if let Some(old_root) = state.root.take() {
        commands.entity(old_root).despawn();
    }
    build_scene(
        &mut commands,
        &mut meshes,
        &mut materials,
        preset.0,
        seed.0,
        store.as_deref(),
        &mut state,
        orbit.as_deref_mut(),
    );
}

/// Сгенерировать сцену и имя пресета для информационного вывода.
fn scene_from(
    preset: TerrainPreset,
    seed: Seed,
    store: Option<&PresetStore>,
) -> (TerrainScene, String) {
    if let Some(store) = store {
        // Демонстрация: истинно случайный пресет из выбранных.
        if let Some(meta) = store.random() {
            if let Some(data) = load_preset_config::<TerrainPresetData>(meta) {
                // Корень каталога пресетов = родитель папки категории.
                let presets_root = store.root.parent().unwrap_or(&store.root);
                let scene = generate_from_data(&data, seed, Some(presets_root));
                // В отображаемом имени — русское название из манифеста, а не
                // технический id пресета (`data.name`), который используется
                // только для обратного поиска архетипа.
                return (scene, meta.name.clone());
            }
        }
    }
    (generate(preset, seed), preset.label().to_string())
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
                TerrainInfoText,
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

/// Обновить панель информации при смене сцены или вида.
fn update_info(
    mut text: Query<&mut Text, With<TerrainInfoText>>,
    state: Res<TerrainViewerState>,
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

    let landscape = &scene.landscape;
    let mut lines = Vec::new();
    lines.push(format!(
        "Территория · {} · seed {}",
        state.preset_name, scene.seed
    ));
    lines.push(format!(
        "Карта {:.0}×{:.0} м · рельеф {:.1}…{:.1} м",
        landscape.heightfield.size_m,
        landscape.heightfield.size_m,
        landscape.min_height,
        landscape.max_height
    ));

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

    lines.push(format!(
        "Объекты: деревья {} · кусты {} · трава {} · здания {}",
        scene.trees.len(),
        scene.bushes.len(),
        scene.grass.len(),
        scene.buildings.len()
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

fn build_scene(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    preset: TerrainPreset,
    seed: Seed,
    store: Option<&PresetStore>,
    state: &mut TerrainViewerState,
    orbit: Option<&mut OrbitCamera>,
) {
    let (scene, preset_name) = scene_from(preset, seed, store);

    if let Some(orbit) = orbit {
        // Та же посадка камеры, что и в генераторе ландшафтов: расстояние до
        // стометровой карты и наклон взгляда должны совпадать, чтобы после
        // перегенерации территория выглядела привычно.
        let mid = (scene.landscape.min_height + scene.landscape.max_height) * 0.5;
        orbit.focus = Vec3::new(0.0, mid.clamp(-20.0, 60.0) * 0.6, 0.0);
        orbit.radius = map_overview_radius(scene.landscape.heightfield.size_m);
        orbit.pitch = 0.58;
    }

    let root = commands
        .spawn((
            Transform::default(),
            Visibility::default(),
            Name::new("Общая территория"),
        ))
        .id();

    // Use the existing renderer for soil, water, waterfalls and point clouds.
    let ground = spawn_landscape(commands, meshes, materials, &scene.landscape);
    commands.entity(root).add_child(ground.root);

    for tree in &scene.trees {
        add_plant(
            commands,
            meshes,
            materials,
            root,
            &scene,
            tree,
            PlantPreset::DeciduousTree,
        );
    }
    for bush in &scene.bushes {
        add_plant(
            commands,
            meshes,
            materials,
            root,
            &scene,
            bush,
            PlantPreset::Bush,
        );
    }

    // Existing grass renderer batches tufts into one mesh. Bucket by 0.5 m
    // elevation so that groups follow the ground approximately. Exact draping
    // of every tuft can be added to the shared renderer in a later iteration.
    let mut grass_bands: BTreeMap<i32, Vec<PlacedTuft>> = BTreeMap::new();
    for tuft in &scene.grass {
        let ground_y = scene
            .landscape
            .heightfield
            .height_at(tuft.position.x, tuft.position.y);
        let band = (ground_y / 0.5).round() as i32;
        grass_bands.entry(band).or_default().push(tuft.clone());
    }
    for (band, tufts) in grass_bands {
        let grass_preset = if matches!(
            scene.preset,
            TerrainPreset::Field | TerrainPreset::Village | TerrainPreset::Town
        ) {
            PlantPreset::SparseGrass
        } else {
            PlantPreset::TallGrass
        };
        let grass_scene = PlantScene {
            mode: GenerationMode::Multiple,
            preset: grass_preset,
            seed,
            plants: Vec::new(),
            tufts,
            extent: 50.0,
            max_height: 1.0,
            cloud_points: 0,
        };
        let spawned = spawn_plant_scene(commands, meshes, materials, &grass_scene);
        commands.entity(spawned.root).insert(
            Transform::from_xyz(0.0, band as f32 * 0.5, 0.0)
                .with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
        );
        commands.entity(root).add_child(spawned.root);
    }

    // Buildings renderer expects the building in local Z-up coordinates.
    for placed in &scene.buildings {
        let spawned = spawn_building(commands, meshes, materials, &placed.building);
        // Local scene XY -> Bevy X,-Z; local Z -> Bevy Y.
        let world_position = Vec3::new(placed.position.x, placed.position.z, -placed.position.y);
        let world_rotation =
            Quat::from_rotation_x(-FRAC_PI_2) * Quat::from_rotation_z(placed.rotation);
        commands
            .entity(spawned.root)
            .insert(Transform::from_translation(world_position).with_rotation(world_rotation));
        commands.entity(root).add_child(spawned.root);
    }

    info!(
        "Территория: {} | seed: {} | 100 x 100m | trees={} bushes={} grass={} buildings={} | Ctrl+S presets, R new, Shift+R same, V mode, Tab camera",
        preset_name,
        seed,
        scene.trees.len(),
        scene.bushes.len(),
        scene.grass.len(),
        scene.buildings.len()
    );

    state.root = Some(root);
    state.preset_name = preset_name;
    state.scene = Some(scene);
}

fn add_plant(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    root: Entity,
    scene: &TerrainScene,
    plant: &PlacedPlant,
    kind: PlantPreset,
) {
    let single = PlantScene {
        mode: GenerationMode::Single,
        preset: kind,
        seed: scene.seed,
        plants: vec![plant.clone()],
        tufts: Vec::new(),
        extent: plant.plant.extent,
        max_height: 0.0,
        cloud_points: 0,
    };
    let ground_y = scene
        .landscape
        .heightfield
        .height_at(plant.position.x, plant.position.y);
    let spawned = spawn_plant_scene(commands, meshes, materials, &single);
    commands.entity(spawned.root).insert(
        Transform::from_xyz(0.0, ground_y, 0.0).with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
    );
    commands.entity(root).add_child(spawned.root);
}

// V toggles the shared ViewMode resource. All three existing renderers use
// their own marker types, so all three are handled here (no custom renderer).
fn apply_view_mode(
    mode: Res<ViewMode>,
    mut visuals: Query<
        (
            &mut Visibility,
            Option<&LandscapeMesh>,
            Option<&LandscapeCloud>,
            Option<&PlantMesh>,
            Option<&PlantCloud>,
            Option<&BuildingMesh>,
            Option<&BuildingCloud>,
        ),
        Or<(
            With<LandscapeMesh>,
            With<LandscapeCloud>,
            With<PlantMesh>,
            With<PlantCloud>,
            With<BuildingMesh>,
            With<BuildingCloud>,
        )>,
    >,
) {
    for (
        mut visibility,
        land_mesh,
        land_cloud,
        plant_mesh,
        plant_cloud,
        building_mesh,
        building_cloud,
    ) in &mut visuals
    {
        let is_mesh = land_mesh.is_some() || plant_mesh.is_some() || building_mesh.is_some();
        let is_cloud = land_cloud.is_some() || plant_cloud.is_some() || building_cloud.is_some();
        *visibility = match *mode {
            ViewMode::Visual if is_mesh => Visibility::Visible,
            ViewMode::PointCloud if is_cloud => Visibility::Visible,
            _ => Visibility::Hidden,
        };
    }
}
