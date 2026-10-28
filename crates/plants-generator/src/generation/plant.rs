//! Дерево/куст: данные, генерация и сборка облака точек.
//!
//! Из скелета веток собираются два содержательных набора частей:
//! - «ветки» — усечённые конусы-сегменты (визуальный меш + облако на боковой
//!   поверхности);
//! - «листва» — крона из листьев/игл (см. `foliage`).
//!
//! Всё в локальной системе координат растения (ось Z — вверх), с нулевым
//! положением у земли.

use bevy::color::Color;
use bevy::math::{Vec2, Vec3};
use generation_core::geometry::Polygon;
use generation_core::occupancy::Occupancy;
use generation_core::point_clouds::primitives::{GridNoise, apply_noise};
use generation_core::{PointCloud, Seed};

use super::config::{PlantConfig, PlantPreset};
use super::foliage::build_foliage;
use super::part::Part;
use super::skeleton::PlantSkeleton;

/// Сгенерированное дерево/куст.
#[derive(Debug, Clone)]
pub struct Plant {
    /// Конфигурация, из которой получено растение.
    pub config: PlantConfig,
    /// Пресет.
    pub preset: PlantPreset,
    /// Seed генерации.
    pub seed: Seed,

    /// Корневая часть-дерево: «растение» → «ветки» + «листва».
    pub root: Part,
    /// Полное облако точек растения (в локальной системе).
    pub point_cloud: PointCloud,
    /// Занимаемая территория (для `terrains-generator` / расстановки).
    pub occupancy: Occupancy,

    /// Полная высота растения, м.
    pub height: f32,
    /// Радиус кроны (горизонтальный размах от оси), м.
    pub extent: f32,
    /// Число сегментов веток.
    pub branch_count: usize,
    /// Число листьев/игл кроны.
    pub leaf_count: usize,
}

impl Plant {
    /// Сгенерировать растение из конфигурации (детерминированно от seed).
    pub fn generate(config: &PlantConfig) -> Self {
        let seed = config.seed;
        let skeleton = PlantSkeleton::build(config);

        // --- ветки ---
        // Ветки группируются по собственному цвету (`BranchGen.color` либо
        // цвет веток конфигурации): у старых CSV-деревьев ствол коричневый,
        // а крона — зелёная, и каждая группа рендерится своим материалом.
        let mut by_color: Vec<(Color, Vec<Polygon>, Vec<Vec3>)> = Vec::new();
        for segment in &skeleton.segments {
            let p0 = skeleton.points[segment.from].position;
            let p1 = skeleton.points[segment.to].position;
            let r0 = skeleton.points[segment.to].radius;
            if r0 <= 1e-4 {
                continue;
            }
            let r1 = r0 * skeleton.points[segment.to].taper;
            let offset = p1 - p0;
            let height = offset.length();
            if height <= 1e-4 {
                continue;
            }
            let axis = offset / height;
            let end = skeleton.points[segment.to];
            let end_branch = config
                .branches
                .get(end.level)
                .and_then(|lv| lv.get(end.branch_id));
            let color = end_branch
                .and_then(|b| b.color)
                .unwrap_or(config.branch_color);
            let bucket = by_color
                .iter()
                .position(|(c, _, _)| *c == color)
                .unwrap_or_else(|| {
                    by_color.push((color, Vec::new(), Vec::new()));
                    by_color.len() - 1
                });
            let sides = end_branch.map(|b| b.mesh_sides.max(3)).unwrap_or(6);

            let (_, faces, cloud) = &mut by_color[bucket];
            let midpoint = p0 + offset * 0.5;
            for poly in cone_side_polygons(r0, r1, height, sides, axis) {
                faces.push(translate_polygon(&poly, midpoint));
            }
            cloud.extend(branch_cone_cloud(r0, r1, height, axis, p0, config.point_step));
        }
        // Шум на облако веток — сетка не должна «светиться» правильной решёткой.
        // Применяется к каждой группе отдельно (шум позиционный и детерминированный,
        // поэтому для одной группы результат совпадает с прежним общим проходом).
        for (_, _, cloud) in &mut by_color {
            apply_noise(
                cloud,
                GridNoise {
                    amplitude: config.jitter,
                },
                seed.with_label("ветки"),
            );
        }

        // Одна группа — сама часть «ветки» (как раньше); несколько — часть-группа
        // с дочерними частями по цвету (старые CSV-деревья: ствол и крона).
        let branches_part = if by_color.len() == 1 {
            let (color, faces, cloud) = by_color.into_iter().next().expect("одна группа");
            Part::new("ветки", color, faces, false, cloud)
        } else {
            let kids: Vec<Part> = by_color
                .into_iter()
                .map(|(color, faces, cloud)| Part::new("ветки", color, faces, false, cloud))
                .collect();
            Part::group(
                "ветки",
                config.branch_color,
                bevy::transform::components::Transform::IDENTITY,
            )
            .children(kids)
        };

        // --- листва ---
        let foliage = build_foliage(&skeleton, config);
        let foliage_part = Part::new(
            "листва",
            config.foliage.color,
            foliage.faces,
            true, // листья рисуются с двух сторон
            foliage.cloud,
        );

        let root = Part::group(
            "растение",
            config.branch_color,
            bevy::transform::components::Transform::IDENTITY,
        )
        .child(branches_part)
        .child(foliage_part);

        let point_cloud = root.collect_cloud();
        // Шум облака веток может «нырнуть» на пару миллиметров под землю
        // (z = 0): лишние точки под поверхностью облаку не нужны.
        let point_cloud = clamp_cloud_to_ground(point_cloud);

        let extent = skeleton.ground_radius.max(0.1);
        let occupancy = Occupancy::circle(Vec2::ZERO, extent * 0.9);

        Self {
            config: config.clone(),
            preset: config.preset,
            seed,
            root,
            point_cloud,
            occupancy,
            height: skeleton.height,
            extent,
            branch_count: skeleton.segments.len(),
            leaf_count: foliage.count,
        }
    }
}

/// Зажать Z всех точек облака на уровень земли (z >= 0).
fn clamp_cloud_to_ground(mut cloud: PointCloud) -> PointCloud {
    for p in &mut cloud.points {
        p.position.z = p.position.z.max(0.0);
    }
    cloud
}

/// Сегмент ветки, посаженный в `p0` (локальные координаты растения).
fn branch_cone_cloud(
    r0: f32,
    r1: f32,
    height: f32,
    axis: Vec3,
    p0: Vec3,
    step: f32,
) -> Vec<Vec3> {
    if step <= 1e-4 {
        return Vec::new();
    }
    let rings = (height / step).round().max(1.0) as usize;
    let (u, v) = basis(axis);
    let mut out = Vec::new();
    for k in 0..=rings {
        let t = k as f32 / rings as f32;
        let r = r0 * (1.0 - t) + r1 * t;
        if r <= 1e-4 {
            continue;
        }
        let center = p0 + axis * (height * t);
        // Минимум 2 точки на кольцо, как в прототипе (`points_per_level`):
        // тонкие кончики глубоких уровней дают по паре точек, а не сплошное
        // кольцо из 6 (иначе 15-уровневое дерево даёт ~500 тыс. точек веток).
        let segs = ((std::f32::consts::TAU * r) / step).round().max(2.0) as usize;
        for i in 0..segs {
            let a = std::f32::consts::TAU * i as f32 / segs as f32;
            out.push(center + u * (r * a.cos()) + v * (r * a.sin()));
        }
    }
    out
}

/// Боковая поверхность И основания усечённого конуса вдоль оси `axis`,
/// нижнее основание на `-height/2`, верхнее на `+height/2`. Нормали наружу.
fn cone_side_polygons(
    r0: f32,
    r1: f32,
    height: f32,
    sides: usize,
    axis: Vec3,
) -> Vec<Polygon> {
    let (u, v) = basis(axis);
    let z0 = -height * 0.5;
    let z1 = height * 0.5;
    let step = std::f32::consts::TAU / sides as f32;
    let mut polys = Vec::with_capacity(sides + 2 * sides);

    // Боковая поверхность.
    for i in 0..sides {
        let a0 = step * i as f32;
        let a1 = step * (i + 1) as f32;
        let c0 = u * a0.cos() + v * a0.sin();
        let c1 = u * a1.cos() + v * a1.sin();
        let b0 = c0 * r0 + axis * z0;
        let b1 = c1 * r0 + axis * z0;
        let t0 = c0 * r1 + axis * z1;
        let t1 = c1 * r1 + axis * z1;
        let mid = (c0 + c1) * 0.5;
        polys.push(Polygon::quad(b0, b1, t1, t0).oriented(mid));
    }

    // Основания (крышки): веером от центра, нормаль наружу.
    for (z, r, pref) in [(z0, r0, -axis), (z1, r1, axis)] {
        if r <= 1e-4 {
            continue;
        }
        let center = axis * z;
        for i in 0..sides {
            let a0 = step * i as f32;
            let a1 = step * (i + 1) as f32;
            let p0 = center + (u * (a0.cos() * r) + v * (a0.sin() * r));
            let p1 = center + (u * (a1.cos() * r) + v * (a1.sin() * r));
            polys.push(Polygon::triangle(center, p0, p1).oriented(pref));
        }
    }

    polys
}

/// Перенести все точки полигона на `offset`.
fn translate_polygon(poly: &Polygon, offset: Vec3) -> Polygon {
    let pts: Vec<Vec3> = poly.points().iter().map(|p| *p + offset).collect();
    Polygon::from_points(pts)
}

/// Ортонормированный базис плоскости, перпендикулярной `axis`.
fn basis(axis: Vec3) -> (Vec3, Vec3) {
    let axis = axis.normalize_or_zero();
    let arbitrary = if axis.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
    let u = axis.cross(arbitrary).normalize();
    let v = axis.cross(u).normalize();
    (u, v)
}
