//! Compound footprints. Only exposed facades contribute points at volume joins.
use super::elements::{Facade, FaceCutout, WallCtx, door_part, wall_part, window_part};
use super::elements::{canopy_has_clearance, canopy_part, canopy_selected};
use super::{Building, BuildingConfig, BuildingPreset, Part};
use bevy::math::Vec2;
use bevy::transform::components::Transform;
use generation_core::occupancy::Occupancy;
use rand::RngExt;

#[path = "compound_roof.rs"]
mod compound_roof;

pub fn generate(config: &BuildingConfig) -> Building {
    let mut cfg = config.clone();
    cfg.preset = BuildingPreset::Rectangular;
    let mut b = Building::generate(&cfg);
    let (l, w, h) = (b.size.x, b.size.y, b.size.z);
    let (x0, x1, x2) = (-l * 0.5, l * 0.12, l * 0.5);
    let (y0, y1, y2) = (-w * 0.5, 0.0, w * 0.5);
    let mut rng = config.seed.with_label("compound-corner").rng();
    let sx = if rng.random::<bool>() { 1.0 } else { -1.0 };
    let sy = if rng.random::<bool>() { 1.0 } else { -1.0 };
    let ah = if config.preset == BuildingPreset::LShaped {
        h
    } else {
        b.floor_height * 0.65
    };
    let mut root = Part::group("compound-house", b.colors.wall, Transform::IDENTITY);
    let segments = [
        (x0, y0, x1, y0, Facade::South, h),
        (x1, y0, x2, y0, Facade::South, ah),
        (x2, y0, x2, y1, Facade::East, ah),
        (x1, y1, x2, y1, Facade::North, ah),
        (x1, y1, x1, y2, Facade::East, h),
        (x0, y2, x1, y2, Facade::North, h),
        (x0, y0, x0, y2, Facade::West, h),
    ];
    for (ax, ay, bx, by, facade, height) in segments {
        let a = Vec2::new(ax * sx, ay * sy);
        let end = Vec2::new(bx * sx, by * sy);
        let facade = match facade {
            Facade::East if sx < 0.0 => Facade::West,
            Facade::West if sx < 0.0 => Facade::East,
            Facade::North if sy < 0.0 => Facade::South,
            Facade::South if sy < 0.0 => Facade::North,
            other => other,
        };
        // The entrance stays on the main volume's south facade, so changing
        // the wing corner also changes its position relative to the entrance.
        let door = facade == Facade::South && ax == x0 && bx == x1;
        root = root.child(facade_part(a, end, facade, 0.0, height, door, &b, config));
    }
    let main = compound_roof::Section::new(x0 * sx, x1 * sx, y0, y2, h);
    let wing = compound_roof::Section::new(x1 * sx, x2 * sx, y0 * sy, y1, ah);
    root = root.child(compound_roof::build(main, wing, x1 * sx, &b, config));
    b.config = config.clone();
    b.root = root;
    b.point_cloud = b.root.collect_cloud();
    b.overhang = 0.0;
    b.parapet = false;
    b.dormer = false;
    b.occupancy = Occupancy::square(Vec2::ZERO, Vec2::new(l * 0.5 + 0.2, w * 0.5 + 0.2), 0.0);
    b
}
#[allow(clippy::too_many_arguments)]
fn facade_part(
    a: Vec2,
    end: Vec2,
    facade: Facade,
    z0: f32,
    height: f32,
    entrance: bool,
    b: &Building,
    config: &BuildingConfig,
) -> Part {
    let length = a.distance(end);
    let rows = (height / b.floor_height).round().max(1.0) as u32;
    let row_h = height / rows as f32;
    let mut group = Part::group(
        "compound-facade",
        b.colors.wall,
        Transform::from_xyz(0.0, 0.0, z0),
    );
    for row in 0..rows {
        let mut cfg = config.clone();
        cfg.window_height = config.window_height.min(row_h * 0.45);
        let ctx = WallCtx {
            facade,
            center: ((a + end) * 0.5).extend(0.0) - facade.outward() * config.wall_thickness * 0.5,
            thickness: config.wall_thickness,
        };
        let door = entrance && row == 0 && row_h > config.door_height + 0.2;
        let width = config.window_width.min(length * 0.55);
        let cut = if door {
            FaceCutout {
                along_min: -config.door_width * 0.5 - 0.08,
                along_max: config.door_width * 0.5 + 0.08,
                z_min: -row_h * 0.5,
                z_max: -row_h * 0.5 + config.door_height + 0.1,
            }
        } else {
            FaceCutout {
                along_min: -width * 0.5 - 0.08,
                along_max: width * 0.5 + 0.08,
                z_min: -cfg.window_height * 0.5 - 0.08,
                z_max: cfg.window_height * 0.5 + 0.08,
            }
        };
        let mut wall = wall_part(&ctx, length, row_h, b.colors.wall, &cfg, &[cut]);
        wall.transform.translation.z += row as f32 * row_h;
        wall = if door {
            wall.child(door_part(
                &ctx,
                (config.door_height + 0.14) * 0.5 - row_h * 0.5,
                b.colors.door,
                config.door_height,
                &cfg,
            ))
        } else {
            wall.child(window_part(&ctx, 0.0, 0.0, width, b.colors.window, &cfg))
        };
        if door
            && canopy_selected(config, "door-canopy", config.door_canopy_probability)
            && canopy_has_clearance(config.door_height + 0.14, config.door_height + 0.28, height)
        {
            wall = wall.child(canopy_part(
                &ctx,
                config.door_width + 0.5,
                0.75,
                config.door_height + 0.28 - row_h * 0.5,
                b.colors.roof,
                config,
            ));
        }
        group = group.child(wall);
    }
    group
}

/// Resolve tall-building dimensions without changing the caller's seed or preset.
pub fn tall_config(config: &BuildingConfig) -> BuildingConfig {
    let mut cfg = config.clone();
    match config.preset {
        BuildingPreset::ApartmentBlock | BuildingPreset::MultiEntranceApartment => {
            let multi = config.preset == BuildingPreset::MultiEntranceApartment;
            cfg.min_size = if multi { 12.0 } else { 16.0 };
            cfg.max_size = if multi { 18.0 } else { 28.0 };
            cfg.max_ratio = if multi { 1.3 } else { 1.7 };
            cfg.min_floors = 6;
            cfg.max_floors = 16;
            cfg.floor_height_min = 2.8;
            cfg.floor_height_max = 3.2;
            cfg.window_width = 1.3;
            cfg.window_height = 1.45;
            cfg.window_gap = 0.9;
            cfg.balcony_probability = 0.45;
        }
        BuildingPreset::Skyscraper => {
            cfg.min_size = 18.0;
            cfg.max_size = 26.0;
            cfg.max_ratio = 1.25;
            cfg.min_floors = 32;
            cfg.max_floors = 60;
            cfg.floor_height_min = 3.2;
            cfg.floor_height_max = 3.6;
            cfg.window_width = 1.6;
            cfg.window_height = 2.3;
            cfg.window_gap = 0.55;
            cfg.balcony_probability = 0.0;
        }
        _ => return cfg,
    }
    cfg.roof_overhang_min = 0.1;
    cfg.roof_overhang_max = 0.2;
    cfg.parapet_probability = 1.0;
    cfg.parapet_height = 0.8;
    cfg.dormer_probability = 0.0;
    cfg.chimney_probability = 0.0;
    cfg.point_step = cfg.point_step.max(0.4);
    cfg.wall_point_step = cfg.wall_point_step.max(0.6);
    cfg
}

/// Single-storey service buildings use wide gates rather than residential doors.
pub fn outbuilding_config(config: &BuildingConfig) -> BuildingConfig {
    let mut cfg = config.clone();
    let (min, max, ratio, low, high, gate_width, gate_height) = match config.preset {
        BuildingPreset::Hangar => (10.0, 30.0, 2.2, 4.8, 7.0, 6.0, 4.0),
        BuildingPreset::Garage => (4.0, 9.0, 1.8, 2.8, 3.5, 2.8, 2.3),
        BuildingPreset::Barn => (6.0, 16.0, 2.0, 3.5, 5.2, 3.0, 2.8),
        _ => return cfg,
    };
    cfg.min_size = min;
    cfg.max_size = max;
    cfg.max_ratio = ratio;
    cfg.min_floors = 1;
    cfg.max_floors = 1;
    cfg.floor_height_min = low;
    cfg.floor_height_max = high;
    cfg.door_width = gate_width;
    cfg.door_height = gate_height;
    cfg.window_width = 0.8;
    cfg.window_height = 0.65;
    cfg.window_gap = 1.5;
    cfg.balcony_probability = 0.0;
    cfg.door_canopy_probability = 0.0;
    cfg.parapet_probability = 0.0;
    cfg.dormer_probability = 0.0;
    cfg.chimney_probability = 0.0;
    cfg
}
