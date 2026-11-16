//! Isolated geometry benchmark plus dense terrain and resource metrics (no FPS claims).
//! cargo run -p terrains-generator --example profile_grass -- 10000
use bevy::{ecs::world::CommandQueue, prelude::*};
use generation_core::Seed;
use plants_generator::{
    PlantPreset,
    generation::{
        grass::generate_tuft,
        instances::{GrassField, GrassInstance},
    },
    visual::spawn_grass_instances,
};
use std::time::Instant;
use terrains_generator::{TerrainPreset, TerrainPresetData, try_generate_from_data};
fn main() {
    let count: usize = std::env::args()
        .nth(1)
        .unwrap_or("10000".into())
        .parse()
        .expect("count");
    let config = plants_generator::resolve(PlantPreset::SparseGrass, Seed::new(42));
    let started = Instant::now();
    let mut old_faces = 0;
    for i in 0..count {
        old_faces += std::hint::black_box(generate_tuft(&config, Seed::new(i as u64)))
            .part
            .faces
            .len();
    }
    let old = started.elapsed();
    let started = Instant::now();
    let mut field = GrassField::variants(&[config], 4, Seed::new(42));
    field.instances = (0..count)
        .map(|i| GrassInstance {
            variant: i % 4,
            position: Vec3::new((i % 100) as f32, (i / 100) as f32, 0.0),
            rotation: i as f32 * 0.7,
            scale: 0.8 + (i % 5) as f32 * 0.1,
        })
        .collect();
    let new = started.elapsed();
    let mut world = World::new();
    let mut queue = CommandQueue::default();
    let mut meshes = Assets::<Mesh>::default();
    let mut materials = Assets::<StandardMaterial>::default();
    let started = Instant::now();
    spawn_grass_instances(
        &mut Commands::new(&mut queue, &world),
        &mut meshes,
        &mut materials,
        &field,
    );
    queue.apply(&mut world);
    println!(
        "Tufts: {count}; old geometry: {:.2} ms / {old_faces} faces; shared variants + instances: {:.2} ms / {} variants",
        old.as_secs_f64() * 1000.0,
        new.as_secs_f64() * 1000.0,
        field.variants.len()
    );
    println!(
        "Shared renderer ECS preparation: {:.2} ms; meshes: {}; materials: {}; entities: {} (no GPU/FPS measured)",
        started.elapsed().as_secs_f64() * 1000.0,
        meshes.len(),
        materials.len(),
        world.query::<Entity>().iter(&world).count()
    );
    let data = TerrainPresetData::from_preset(TerrainPreset::Forest);
    let scene = try_generate_from_data(&data, Seed::new(42), None).unwrap();
    println!(
        "Dense forest: {} grass instances, {} variants, grass {:.2} ms, full composition {:.2} ms",
        scene.grass.len(),
        scene.grass.variants.len(),
        scene.grass_generation_time.as_secs_f64() * 1000.0,
        scene.generation_time.as_secs_f64() * 1000.0
    );
}
