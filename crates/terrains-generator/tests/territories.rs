use bevy::math::{Vec2, Vec3};
use generation_core::presets::{PresetStore, load_preset_config};
use generation_core::{Occupancy, PointClass, Seed};
use plants_generator::PlantPreset;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
use terrains_generator::{
    preset::{GrassDensity, Settlement},
    sources::{self, PresetSource},
    *,
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../presets")
}
fn small(preset: TerrainPreset) -> TerrainPresetData {
    let mut data = TerrainPresetData::from_preset_with(preset, Some(&root()));
    data.grass_settings.density = GrassDensity::Legacy;
    data.grass = 25;
    data.trees = data.trees.min(9);
    data.bushes = data.bushes.min(3);
    data
}
fn scene(preset: TerrainPreset, seed: u64) -> TerrainScene {
    try_generate_from_data(&small(preset), Seed::new(seed), Some(&root())).unwrap()
}
#[test]
fn all_catalogue_configs_load_and_references_resolve() {
    let store = PresetStore::discover("terrains", &root());
    for &preset in TerrainPreset::ALL {
        assert!(
            store.presets.iter().any(|meta| meta.id == preset.name()),
            "{} missing",
            preset.name()
        );
        assert_eq!(TerrainPreset::from_name(preset.name()), Some(preset));
    }
    for meta in &store.presets {
        let data = load_preset_config::<TerrainPresetData>(meta).expect(&meta.id);
        data.validate().expect(&meta.id);
        for (category, refs, legacy) in [
            ("landscapes", &data.landscape_sources, &data.landscape_id),
            ("plants", &data.tree_sources, &data.tree_id),
            ("plants", &data.bush_sources, &data.bush_id),
            ("plants", &data.grass_sources, &data.grass_id),
            ("buildings", &data.building_sources, &data.building_id),
        ] {
            sources::resolve(category, refs, legacy, Some(&root())).expect(&meta.id);
        }
        let text = ron::ser::to_string(&data).unwrap();
        assert_eq!(ron::from_str::<TerrainPresetData>(&text).unwrap(), data);
    }
}
#[test]
fn old_config_and_new_custom_names_work() {
    let text = r#"(name:"custom-meadow", landscape_id:"plain", tree_id:"deciduous-tree", bush_id:"bush", grass_id:"sparse-grass", building_id:"", trees:0, bushes:0, grass:5, buildings:0)"#;
    let data: TerrainPresetData = ron::from_str(text).unwrap();
    let scene = try_generate_from_data(&data, Seed::new(6), Some(&root())).unwrap();
    assert_eq!(scene.preset, TerrainPreset::Custom);
    assert_eq!(scene.trees.len(), 0);
    assert!(!scene.grass.is_empty());
}
#[test]
fn nested_groups_expand_stably_and_deduplicate() {
    let refs = [
        PresetSource::preset("pine"),
        PresetSource::group("high"),
        PresetSource::group("coniferous"),
    ];
    let candidates = sources::resolve("plants", &refs, "", Some(&root())).unwrap();
    let ids: Vec<_> = candidates.iter().map(|c| c.source.id.as_str()).collect();
    assert_eq!(ids.len(), 7);
    assert_eq!(
        ids.iter().copied().collect::<BTreeSet<_>>().len(),
        ids.len()
    );
    assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(
        sources::resolve(
            "plants",
            &[PresetSource::group("high/coniferous")],
            "",
            Some(&root())
        )
        .unwrap()
        .len(),
        3
    );
}
#[test]
fn bad_references_and_weights_are_errors() {
    assert!(
        sources::resolve(
            "plants",
            &[PresetSource::preset("missing")],
            "",
            Some(&root())
        )
        .is_err()
    );
    assert!(
        sources::resolve(
            "plants",
            &[PresetSource::group("missing")],
            "",
            Some(&root())
        )
        .is_err()
    );
    assert!(sources::resolve("plants", &[PresetSource::group("coniferous")], "", None).is_err());
    let mut source = PresetSource::preset("pine");
    source.weight = f32::NAN;
    assert!(sources::resolve("plants", &[source], "", Some(&root())).is_err());
    let mut data = small(TerrainPreset::Field);
    data.tree_id = "missing".into();
    assert!(try_generate_from_data(&data, Seed::new(0), Some(&root())).is_err());
    data.tree_id = "bush".into();
    assert!(try_generate_from_data(&data, Seed::new(0), Some(&root())).is_err());
}
#[test]
fn empty_group_is_an_explicit_error() {
    let dir = std::env::temp_dir().join(format!("lidar-empty-group-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("plants/empty")).unwrap();
    let result = sources::resolve("plants", &[PresetSource::group("empty")], "", Some(&dir));
    assert!(result.unwrap_err().contains("пустая группа"));
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn weighted_choice_is_seeded_and_respects_distribution() {
    let candidates = [("a", 1.0), ("b", 9.0)];
    let sample = |seed: u64| {
        let mut rng = Seed::new(seed).rng();
        (0..2000)
            .map(|_| sources::choose(&candidates, |c| c.1, &mut rng))
            .collect::<Vec<_>>()
    };
    let a = sample(9);
    assert_eq!(a, sample(9));
    assert_ne!(a, sample(10));
    assert!(a.iter().filter(|&&index| index == 1).count() > 1600);
}
#[test]
fn fields_obey_exact_tree_limits_and_tree_free_slopes() {
    for seed in [1, 9, 42] {
        assert!((1..=3).contains(&scene(TerrainPreset::Field, seed).trees.len()));
    }
    for preset in [
        TerrainPreset::OpenField,
        TerrainPreset::HillSlope,
        TerrainPreset::TerracedSlope,
        TerrainPreset::RiverOpen,
        TerrainPreset::PondOpen,
        TerrainPreset::CoastOpen,
        TerrainPreset::WaterfallOpen,
    ] {
        assert!(scene(preset, 42).trees.is_empty(), "{}", preset.name());
    }
}
#[test]
fn conifers_palms_and_juniper_reuse_existing_models() {
    for (preset, expected) in [
        (
            TerrainPreset::ConiferForest,
            vec![PlantPreset::Spruce, PlantPreset::Fir, PlantPreset::Pine],
        ),
        (
            TerrainPreset::Beach,
            vec![PlantPreset::Palm, PlantPreset::PalmCoconuts],
        ),
    ] {
        let mut data = small(preset);
        data.trees = 24;
        let generated = try_generate_from_data(&data, Seed::new(42), Some(&root())).unwrap();
        for kind in expected {
            assert!(
                generated.trees.iter().any(|p| p.plant.preset == kind),
                "{preset:?}: {kind:?}"
            );
        }
    }
    let mut data = small(TerrainPreset::Woodland);
    data.bush_sources = vec![PresetSource::preset("juniper")];
    data.bushes = 2;
    let generated = try_generate_from_data(&data, Seed::new(42), Some(&root())).unwrap();
    assert!(!generated.bushes.is_empty());
    assert!(
        generated
            .bushes
            .iter()
            .all(|p| p.plant.preset == PlantPreset::Juniper && !p.plant.point_cloud.is_empty())
    );
}
#[test]
fn water_zones_are_obeyed() {
    for preset in [
        TerrainPreset::River,
        TerrainPreset::Pond,
        TerrainPreset::Coast,
        TerrainPreset::Waterfall,
    ] {
        let generated = scene(preset, 42);
        let map = PlacementMap::new(&generated.landscape);
        for tree in &generated.trees {
            let distance = map.distance_to_water(tree.position);
            match tree.plant.preset {
                PlantPreset::Willow => assert!(distance <= 11.0),
                PlantPreset::DeciduousTree => assert!(distance >= 13.0),
                _ => panic!("unexpected tree"),
            }
        }
        assert!(!generated.trees.is_empty(), "{}", preset.name());
    }
}
#[test]
fn grass_can_grow_under_canopy_but_not_in_trunks_buildings_or_water() {
    let generated = scene(TerrainPreset::OpenField, 42);
    let mut map = PlacementMap::new(&generated.landscape);
    let rules = PlacementRules {
        radius: 5.0,
        clearance: 0.0,
        max_slope_deg: 40.0,
    };
    assert!(map.try_place_tree(Vec2::ZERO, rules, 0.5).is_some());
    let grass = PlacementRules {
        radius: 0.1,
        ..rules
    };
    assert!(map.try_place_grass(Vec2::ZERO, grass).is_none());
    assert!(map.try_place_grass(Vec2::new(1.5, 0.0), grass).is_some());
    map.reserve(Occupancy::circle(Vec2::new(12.0, 0.0), 3.0));
    assert!(map.try_place_grass(Vec2::new(12.0, 0.0), grass).is_none());
    assert!(map.try_place_grass(Vec2::new(50.0, 0.0), grass).is_none());
    let wet = scene(TerrainPreset::PondOpen, 42);
    let hf = &wet.landscape.heightfield;
    let index = wet
        .landscape
        .water
        .levels
        .iter()
        .position(Option::is_some)
        .unwrap();
    assert!(
        PlacementMap::new(&wet.landscape)
            .try_place_grass(hf.node_xy(index % hf.n, index / hf.n), grass)
            .is_none()
    );
}
#[test]
fn streets_are_clear_and_facades_face_the_road() {
    for preset in [
        TerrainPreset::Village,
        TerrainPreset::ResidentialCity,
        TerrainPreset::ApartmentCity,
        TerrainPreset::SkyscraperCity,
    ] {
        let generated = scene(preset, 42);
        assert!(!generated.roads.is_empty());
        assert!(!generated.buildings.is_empty(), "{}", preset.name());
        for house in &generated.buildings {
            let xy = house.position.truncate();
            let road = generated
                .roads
                .iter()
                .min_by(|a, b| a.distance(xy).total_cmp(&b.distance(xy)))
                .unwrap();
            let forward = Vec2::new(house.rotation.sin(), -house.rotation.cos());
            assert!(forward.dot((road.nearest(xy) - xy).normalize()) > 0.98);
            for road in &generated.roads {
                assert!(
                    road.distance(xy) >= house.footprint.shape.bounding_radius() + road.width * 0.5
                );
            }
        }
    }
}
#[test]
fn bounds_heights_slopes_and_collisions_are_respected() {
    for preset in [
        TerrainPreset::Village,
        TerrainPreset::ForestSlope,
        TerrainPreset::Beach,
    ] {
        let generated = scene(preset, 17);
        assert_eq!(generated.landscape.heightfield.size_m, 100.0);
        for (i, a) in generated.footprints.iter().enumerate() {
            // Road rectangles can cross each other, all object footprints must be separate.
            for b in &generated.footprints[generated.roads.len()..i.max(generated.roads.len())] {
                assert!(!a.intersects(b));
            }
        }
        for grass in &generated.grass.instances {
            let p = grass.position;
            assert!(p.x.abs() < 50.0 && p.y.abs() < 50.0);
            assert_eq!(p.z, generated.landscape.heightfield.height_at(p.x, p.y));
        }
    }
}
#[test]
fn identical_seeds_reproduce_models_transforms_terrain_and_clouds() {
    for seed in [4, 17] {
        let mut data = small(TerrainPreset::Woodland);
        data.trees = 3;
        data.bushes = 2;
        let a = try_generate_from_data(&data, Seed::new(seed), Some(&root())).unwrap();
        let b = try_generate_from_data(&data, Seed::new(seed), Some(&root())).unwrap();
        assert_eq!(
            a.landscape.heightfield.values,
            b.landscape.heightfield.values
        );
        assert_eq!(a.grass.instances, b.grass.instances);
        assert_eq!(
            a.trees
                .iter()
                .map(|p| (p.position, p.rotation, p.plant.preset, p.plant.extent))
                .collect::<Vec<_>>(),
            b.trees
                .iter()
                .map(|p| (p.position, p.rotation, p.plant.preset, p.plant.extent))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            generation_core::point_clouds::export::to_xyz(&a.point_cloud()),
            generation_core::point_clouds::export::to_xyz(&b.point_cloud())
        );
    }
}
#[test]
fn grass_variants_are_bounded_and_cloud_uses_instance_transforms() {
    use plants_generator::generation::instances::{GrassField, GrassInstance};
    let config = plants_generator::resolve(PlantPreset::SparseGrass, Seed::new(1));
    let mut field = GrassField::variants(&[config], 3, Seed::new(2));
    let instance = GrassInstance {
        variant: 1,
        position: Vec3::new(2.0, 3.0, 4.0),
        rotation: 0.7,
        scale: 1.2,
    };
    field.instances.push(instance);
    let cloud = field.point_cloud();
    for (actual, original) in cloud
        .points
        .iter()
        .zip(&field.variants[1].point_cloud.points)
    {
        assert_eq!(
            actual.position,
            instance.matrix().transform_point3(original.position)
        );
        assert_eq!(actual.class, original.class);
        assert_eq!(actual.class, PointClass::Vegetation);
    }
    field.instances.extend(std::iter::repeat_n(instance, 10000));
    assert_eq!(field.variants.len(), 3);
}
#[test]
fn alternative_landscapes_and_custom_layout_are_data_driven() {
    let mut data = small(TerrainPreset::OpenField);
    data.name = "new-custom-terrain".into();
    data.landscape_sources = vec![PresetSource::preset("plain"), PresetSource::preset("slope")];
    data.placement.settlement = Settlement::Grid;
    let mut labels = BTreeSet::new();
    for seed in 0..8 {
        let scene = try_generate_from_data(&data, Seed::new(seed), Some(&root())).unwrap();
        assert_eq!(scene.preset, TerrainPreset::Custom);
        assert_eq!(scene.roads.len(), 2);
        labels.insert(scene.landscape_name);
    }
    assert_eq!(labels.len(), 2);
}

#[test]
fn shared_grass_renderer_allocates_resources_per_variant_not_per_tuft() {
    use bevy::{ecs::world::CommandQueue, prelude::*};
    use plants_generator::generation::instances::{GrassField, GrassInstance};
    let config = plants_generator::resolve(PlantPreset::SparseGrass, Seed::new(0));
    let mut field = GrassField::variants(&[config], 3, Seed::new(1));
    field.instances = (0..1000)
        .map(|i| GrassInstance {
            variant: i % 3,
            position: Vec3::ZERO,
            rotation: 0.0,
            scale: 1.0,
        })
        .collect();
    let mut world = World::new();
    let mut queue = CommandQueue::default();
    let mut meshes = Assets::<Mesh>::default();
    let mut materials = Assets::<StandardMaterial>::default();
    plants_generator::visual::spawn_grass_instances(
        &mut Commands::new(&mut queue, &world),
        &mut meshes,
        &mut materials,
        &field,
    );
    queue.apply(&mut world);
    assert_eq!(meshes.len(), 4);
    assert_eq!(materials.len(), 3);
    assert_eq!(
        world
            .query::<&MeshMaterial3d<StandardMaterial>>()
            .iter(&world)
            .count(),
        1000
    );
}

#[test]
fn custom_landscape_files_are_preserved_when_archetype_variation_is_disabled() {
    let dir = std::env::temp_dir().join(format!("lidar-custom-landscape-{}", std::process::id()));
    let preset_dir = dir.join("landscapes/custom");
    std::fs::create_dir_all(&preset_dir).unwrap();
    std::fs::write(
        preset_dir.join("manifest.ron"),
        r#"(id:"custom",name:"Мой склон",kind:"plain")"#,
    )
    .unwrap();
    let mut landscape = landscapes_generator::LandscapeConfig::default();
    landscape.base.z0 = 7.0;
    landscape.noise.amplitude = 0.0;
    std::fs::write(
        preset_dir.join("config.ron"),
        ron::ser::to_string(&landscape).unwrap(),
    )
    .unwrap();
    let mut data = small(TerrainPreset::OpenField);
    data.landscape_id = "custom".into();
    data.tree_id.clear();
    data.bush_id.clear();
    data.grass_id.clear();
    data.trees = 0;
    data.bushes = 0;
    data.grass_settings.density = GrassDensity::None;
    data.regenerate_landscape = false;
    let generated = try_generate_from_data(&data, Seed::new(2), Some(&dir)).unwrap();
    assert!(
        generated
            .landscape
            .heightfield
            .values
            .iter()
            .all(|&h| (h - 7.0).abs() < 0.001)
    );
    std::fs::write(preset_dir.join("config.ron"), "broken RON").unwrap();
    assert!(try_generate_from_data(&data, Seed::new(2), Some(&dir)).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn juniper_cloud_and_geometry_are_finite_and_repeatable() {
    for seed in [0, 9, 42] {
        let config = plants_generator::resolve(PlantPreset::Juniper, Seed::new(seed));
        let a = plants_generator::Plant::generate(&config);
        let b = plants_generator::Plant::generate(&config);
        assert!(a.point_cloud.points.iter().all(|p| p.position.is_finite()));
        assert_eq!(a.point_cloud.points.len(), b.point_cloud.points.len());
        assert!(
            a.point_cloud
                .points
                .iter()
                .zip(&b.point_cloud.points)
                .all(|(a, b)| a.position == b.position && a.class == b.class)
        );
    }
}

#[test]
fn every_builtin_terrain_generates_from_its_ron_without_invalid_coordinates() {
    let store = PresetStore::discover("terrains", &root());
    for &preset in TerrainPreset::ALL {
        let meta = store
            .presets
            .iter()
            .find(|p| p.id == preset.name())
            .unwrap();
        let mut data = load_preset_config::<TerrainPresetData>(meta).unwrap();
        data.trees = data.trees.min(3);
        data.bushes = data.bushes.min(1);
        data.grass_settings.density = GrassDensity::Legacy;
        data.grass = 8;
        let generated =
            try_generate_from_data(&data, Seed::new(42), Some(&root())).expect(preset.name());
        assert!(
            generated
                .grass
                .instances
                .iter()
                .all(|p| p.position.is_finite())
        );
        assert!(generated.buildings.iter().all(|p| p.position.is_finite()));
        for plant in generated.trees.iter().chain(&generated.bushes) {
            assert!(plant.position.is_finite());
            assert!(
                plant
                    .plant
                    .point_cloud
                    .points
                    .iter()
                    .all(|p| p.position.is_finite()),
                "{}",
                preset.name()
            );
        }
    }
}
