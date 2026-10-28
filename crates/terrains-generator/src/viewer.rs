//! Composition only: reuses the team's existing Bevy camera, controls and renderers.
//! No custom terrain, plant, building or point-cloud renderer is implemented here.

use std::collections::BTreeMap;
use std::f32::consts::FRAC_PI_2;

use bevy::prelude::*;
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

#[derive(Resource)]
struct SelectedPreset(TerrainPreset);

#[derive(Default, Resource)]
struct TerrainViewerState {
    root: Option<Entity>,
}

pub struct TerrainViewerPlugin {
    pub initial_preset: TerrainPreset,
}

impl Plugin for TerrainViewerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(SelectedPreset(self.initial_preset))
            .init_resource::<TerrainViewerState>()
            .add_systems(Startup, setup)
            .add_systems(Update, (regenerate, apply_view_mode).chain());
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
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
    ));
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

    build_scene(
        &mut commands,
        &mut meshes,
        &mut materials,
        preset.0,
        seed.0,
        store.as_deref(),
        &mut state,
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
                return (scene, data.name.clone());
            }
        }
    }
    (generate(preset, seed), preset.name().to_string())
}

fn build_scene(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    preset: TerrainPreset,
    seed: Seed,
    store: Option<&PresetStore>,
    state: &mut TerrainViewerState,
) {
    let (scene, preset_name) = scene_from(preset, seed, store);
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

    state.root = Some(root);
    info!(
        "Preset: {} | seed: {} | 100 x 100m | trees={} bushes={} grass={} buildings={} | Ctrl+S presets, R new, Shift+R same, V mode, Tab camera",
        preset_name,
        seed,
        scene.trees.len(),
        scene.bushes.len(),
        scene.grass.len(),
        scene.buildings.len()
    );
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
