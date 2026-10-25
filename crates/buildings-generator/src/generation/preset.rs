//! Compound footprints. Only exposed facades contribute points at volume joins.
use super::elements::{canopy_part, canopy_selected, canopy_has_clearance};
use super::elements::{Facade, FaceCutout, WallCtx, door_part, wall_part, window_part};
use super::{Building, BuildingConfig, BuildingPreset, Part, RoofStyle};
use bevy::math::{Vec2, Vec3};
use bevy::transform::components::Transform;
use generation_core::geometry::Polygon;
use generation_core::occupancy::Occupancy;

pub fn generate(config: &BuildingConfig) -> Building {
    let mut cfg = config.clone();
    cfg.preset = BuildingPreset::Rectangular;
    let mut b = Building::generate(&cfg);
    let (l, w, h) = (b.size.x, b.size.y, b.size.z);
    let (x0, x1, x2) = (-l * 0.5, l * 0.12, l * 0.5);
    let (y0, y1, y2) = (-w * 0.5, 0.0, w * 0.5);
    let ah = if config.preset == BuildingPreset::LShaped {
        h
    } else {
        b.floor_height * 0.65
    };
    let mut root = Part::group("compound-house", b.colors.wall, Transform::IDENTITY);
    let segments = [
        (x0, y0, x1, y0, Facade::South, h, true),
        (x1, y0, x2, y0, Facade::South, ah, false),
        (x2, y0, x2, y1, Facade::East, ah, false),
        (x1, y1, x2, y1, Facade::North, ah, false),
        (x1, y1, x1, y2, Facade::East, h, false),
        (x0, y2, x1, y2, Facade::North, h, false),
        (x0, y0, x0, y2, Facade::West, h, false),
    ];
    let t = config.roof_thickness;
    let mut roofs = vec![top(x0, x1, y0, y2, h + t), top(x1, x2, y0, y1, ah + t)];
    for (ax, ay, bx, by, facade, height, door) in segments {
        let a = Vec2::new(ax, ay);
        let end = Vec2::new(bx, by);
        root = root.child(facade_part(a, end, facade, 0.0, height, door, &b, config));
        roofs.push(
            Polygon::quad(
                a.extend(height),
                end.extend(height),
                end.extend(height + t),
                a.extend(height + t),
            )
            .oriented(facade.outward()),
        );
    }
    if ah < h {
        root = root.child(facade_part(
            Vec2::new(x1, y0),
            Vec2::new(x1, y1),
            Facade::East,
            ah,
            h - ah,
            false,
            &b,
            config,
        ));
        roofs.push(
            Polygon::quad(
                Vec3::new(x1, y0, h),
                Vec3::new(x1, y1, h),
                Vec3::new(x1, y1, h + t),
                Vec3::new(x1, y0, h + t),
            )
            .oriented(Vec3::X),
        );
    }
    root = root.child(Part::from_faces(
        "compound-roof",
        roofs,
        b.colors.roof,
        Transform::IDENTITY,
        true,
        config,
    ));
    b.config = config.clone();
    b.root = root;
    b.point_cloud = b.root.collect_cloud();
    b.roof = RoofStyle::Flat;
    b.roof_pitch_deg = 0.0;
    b.overhang = 0.0;
    b.parapet = false;
    b.dormer = false;
    b.occupancy = Occupancy::square(Vec2::ZERO, Vec2::new(l * 0.5 + 0.2, w * 0.5 + 0.2), 0.0);
    b
}
fn top(x0: f32, x1: f32, y0: f32, y1: f32, z: f32) -> Polygon {
    Polygon::quad(
        Vec3::new(x0, y0, z),
        Vec3::new(x1, y0, z),
        Vec3::new(x1, y1, z),
        Vec3::new(x0, y1, z),
    )
    .oriented(Vec3::Z)
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
        if door && canopy_selected(config,"door-canopy",config.door_canopy_probability)
            && canopy_has_clearance(config.door_height+0.14,config.door_height+0.28,height) {
            wall = wall.child(canopy_part(&ctx,config.door_width+0.5,0.75,config.door_height+0.28-row_h*0.5,b.colors.roof,config));
        }
        group = group.child(wall);
    }
    group
}

/// Resolve tall-building dimensions without changing the caller's seed or preset.
pub fn tall_config(config: &BuildingConfig) -> BuildingConfig {
    let mut cfg = config.clone();
    match config.preset {
        BuildingPreset::ApartmentBlock => {
            cfg.min_size=16.0; cfg.max_size=28.0; cfg.max_ratio=1.7;
            cfg.min_floors=6; cfg.max_floors=16;
            cfg.floor_height_min=2.8; cfg.floor_height_max=3.2;
            cfg.window_width=1.3; cfg.window_height=1.45; cfg.window_gap=0.9;
            cfg.balcony_probability=0.45;
        }
        BuildingPreset::Skyscraper => {
            cfg.min_size=18.0; cfg.max_size=26.0; cfg.max_ratio=1.25;
            cfg.min_floors=32; cfg.max_floors=60;
            cfg.floor_height_min=3.2; cfg.floor_height_max=3.6;
            cfg.window_width=1.6; cfg.window_height=2.3; cfg.window_gap=0.55;
            cfg.balcony_probability=0.0;
        }
        _ => return cfg,
    }
    cfg.roof_overhang_min=0.1; cfg.roof_overhang_max=0.2;
    cfg.parapet_probability=1.0; cfg.parapet_height=0.8;
    cfg.dormer_probability=0.0; cfg.chimney_probability=0.0;
    cfg.point_step=cfg.point_step.max(0.4);
    cfg.wall_point_step=cfg.wall_point_step.max(0.6);
    cfg
}
