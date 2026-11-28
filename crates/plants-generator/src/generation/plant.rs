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
use super::foliage::{
    build_agave_edges,
    build_cactus_flower,
    build_cactus_needles,
    build_foliage,
};
use super::part::Part;
use super::skeleton::{PlantSkeleton, is_terminal};

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

            let is_cactus_tip =
                config.preset == PlantPreset::CactusLarge
                && is_terminal(&skeleton, segment);

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
            if config.preset == PlantPreset::CactusLarge {
                // Smooth only unbranched joints. At forks each arm retains its own axis.
                let incoming = skeleton.segments.iter()
                    .find(|other| other.to == segment.from)
                    .map(|other| (p0 - skeleton.points[other.from].position).normalize_or_zero());
                let outgoing: Vec<Vec3> = skeleton.segments.iter()
                    .filter(|other| other.from == segment.to)
                    .map(|other| (skeleton.points[other.to].position - p1).normalize_or_zero())
                    .collect();
                let from_count = skeleton.segments.iter()
                    .filter(|other| other.from == segment.from).count();
                let start_axis = if from_count == 1 {
                    incoming.map(|prev| (prev + axis).normalize_or_zero())
                        .filter(|dir| dir.length_squared() > 1e-6)
                        .unwrap_or(axis)
                } else { axis };
                let end_axis = if outgoing.len() == 1 {
                    (axis + outgoing[0]).normalize_or_zero()
                } else { axis };
                let end_axis = if end_axis.length_squared() > 1e-6 { end_axis } else { axis };
                let (polygons, points) = cactus_curved_geometry(
                    p0, p1, r0, r1, sides, start_axis, end_axis,
                    is_cactus_tip, config.point_step,
                );
                faces.extend(polygons);
                cloud.extend(points);
                continue;
            }
            let midpoint = p0 + offset * 0.5;
            let polygons = if config.preset == PlantPreset::CactusSmall {
                cactus_small_polygons(
                    r0,
                    r1,
                    height,
                    sides,
                    axis,
                )
            } else if config.preset == PlantPreset::Yucca {
                yucca_trunk_polygons(
                    r0,
                    r1,
                    height,
                    sides,
                    axis,
                )
            } else if config.preset == PlantPreset::Oak && end.level == 0 {
                oak_trunk_polygons(
                    r0,
                    r1,
                    height,
                    sides,
                    axis,
                )
            } else {
                cone_side_polygons(r0, r1, height, sides, axis)
            };
            for poly in polygons {
                faces.push(translate_polygon(&poly, midpoint));
            }
                        if config.preset == PlantPreset::CactusSmall {
                    cloud.extend(cactus_small_cloud(
                        r0,
                        r1,
                        height,
                        axis,
                        p0,
                        config.point_step,
                    ));
                } else {
                    cloud.extend(branch_cone_cloud(
                        r0,
                        r1,
                        height,
                        axis,
                        p0,
                        config.point_step,
                    ));
                }
        }
        // Шум на облако веток — сетка не должна «светиться» правильной решёткой.
        // Применяется к каждой группе отдельно (шум позиционный и детерминированный,
        // поэтому для одной группы результат совпадает с прежним общим проходом).
        if config.preset != PlantPreset::CactusSmall {
            for (_, _, cloud) in &mut by_color {
                apply_noise(
                    cloud,
                    GridNoise {
                        amplitude: config.jitter,
                    },
                    seed.with_label("ветки"),
                );
            }
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

        let foliage_part = if config.preset == PlantPreset::CactusSmall {
            let needles = build_cactus_needles(&skeleton, config);
            let flower = build_cactus_flower(&skeleton, config);

            let needles_part = Part::new(
                "иголки",
                Color::srgb(0.40, 0.314, 0.224),
                needles.faces,
                true,
                needles.cloud,
            );

            let flower_part = Part::new(
                "цветок",
                config.foliage.color,
                flower.faces,
                true,
                flower.cloud,
            );

            Part::group(
                "листва",
                config.foliage.color,
                bevy::transform::components::Transform::IDENTITY,
            )
            .child(needles_part)
            .child(flower_part)
                } else if config.preset == PlantPreset::Agave {
            let edges = build_agave_edges(&skeleton, config);

            let leaves_part = Part::new(
                "листья",
                config.foliage.color,
                foliage.faces,
                true,
                foliage.cloud,
            );

            let edges_part = Part::new(
                "кромка",
                Color::srgb(0.34, 0.48, 0.37),
                edges.faces,
                true,
                edges.cloud,
            );

            Part::group(
                "листва",
                config.foliage.color,
                bevy::transform::components::Transform::IDENTITY,
            )
            .child(leaves_part)
            .child(edges_part)
        } else {
            Part::new(
                "листва",
                config.foliage.color,
                foliage.faces,
                true,
                foliage.cloud,
            )
        };

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

/// Облако точек маленького кактуса:
/// точки распределяются по поверхности каждого отдельного побега,
/// сохраняя его вертикальную форму и продольные рёбра.
fn cactus_small_cloud(
    r0: f32,
    r1: f32,
    height: f32,
    axis: Vec3,
    p0: Vec3,
    step: f32,
) -> Vec<Vec3> {
    if step <= 1e-4 || height <= 1e-4 {
        return Vec::new();
    }

    let (u, v) = basis(axis);

    let vertical_step = step.max(0.09);
    let rings = (height / vertical_step).ceil().max(6.0) as usize;

    // Нескольких продольных линий достаточно, чтобы показать
    // цилиндрическую форму побега, не сливая соседние побеги
    // в одно сплошное облако.
    let rib_count = 6usize;

    let mut out = Vec::with_capacity((rings + 1) * rib_count);

    for k in 0..=rings {
        let t = k as f32 / rings as f32;

        let center = p0 + axis * (height * t);

        let base_radius = r0 * (1.0 - t) + r1 * t;

        // Верхняя часть плавно сужается, повторяя округлую макушку.
        let cap_start = 0.82_f32;
        let cap_factor = if t > cap_start {
            let cap_t = (t - cap_start) / (1.0 - cap_start);
            (cap_t * std::f32::consts::FRAC_PI_2)
                .cos()
                .max(0.08)
        } else {
            1.0
        };

        let radius = base_radius * cap_factor;

        for rib in 0..rib_count {
            let angle =
                std::f32::consts::TAU * rib as f32 / rib_count as f32;

            // Небольшое чередование радиуса подчёркивает рёбра,
            // но не превращает облако в сплошной овальный объём.
            let relief = if rib % 2 == 0 { 1.045 } else { 0.955 };

            let radial =
                u * angle.cos() + v * angle.sin();

            out.push(center + radial * radius * relief);
        }
    }

    out
}

/// Ребристая поверхность маленького кактуса
/// с плавно закруглённой верхушкой.
fn cactus_small_polygons(
    r0: f32,
    r1: f32,
    height: f32,
    sides: usize,
    axis: Vec3,
) -> Vec<Polygon> {
    let (u, v) = basis(axis);

    let rib_count = sides.max(6);
    let vertex_count = rib_count * 2;
    let angle_step = std::f32::consts::TAU / vertex_count as f32;

    // Основная цилиндрическая часть заканчивается чуть ниже
    // полной высоты — оставшееся место занимает округлая макушка.
    let cap_height = r1.min(height * 0.22);
    let body_height = (height - cap_height).max(height * 0.5);

    let z_bottom = -height * 0.5;
    let z_body_top = z_bottom + body_height;

    let mut polys = Vec::new();

    // Ребристая боковая поверхность основного побега.
    for i in 0..vertex_count {
        let next = (i + 1) % vertex_count;

        let a0 = angle_step * i as f32;
        let a1 = angle_step * next as f32;

        let c0 = u * a0.cos() + v * a0.sin();
        let c1 = u * a1.cos() + v * a1.sin();

        let relief0 = if i % 2 == 0 { 1.045 } else { 0.955 };
        let relief1 = if next % 2 == 0 { 1.045 } else { 0.955 };

        let b0 = c0 * (r0 * relief0) + axis * z_bottom;
        let b1 = c1 * (r0 * relief1) + axis * z_bottom;

        let t0 = c0 * (r1 * relief0) + axis * z_body_top;
        let t1 = c1 * (r1 * relief1) + axis * z_body_top;

        let mid = (c0 * relief0 + c1 * relief1) * 0.5;

        polys.push(
            Polygon::quad(b0, b1, t1, t0)
                .oriented(mid),
        );
    }

    // Округлая верхушка из нескольких постепенно сужающихся колец.
    let cap_rings = 4usize;

    for ring in 0..cap_rings {
        let t0 = ring as f32 / cap_rings as f32;
        let t1 = (ring + 1) as f32 / cap_rings as f32;

        let angle0 = t0 * std::f32::consts::FRAC_PI_2;
        let angle1 = t1 * std::f32::consts::FRAC_PI_2;

        let ring_r0 = r1 * angle0.cos();
        let ring_r1 = r1 * angle1.cos();

        let ring_z0 = z_body_top + cap_height * angle0.sin();
        let ring_z1 = z_body_top + cap_height * angle1.sin();

        for i in 0..vertex_count {
            let next = (i + 1) % vertex_count;

            let a0 = angle_step * i as f32;
            let a1 = angle_step * next as f32;

            let c0 = u * a0.cos() + v * a0.sin();
            let c1 = u * a1.cos() + v * a1.sin();

            let relief0 = if i % 2 == 0 { 1.045 } else { 0.955 };
            let relief1 = if next % 2 == 0 { 1.045 } else { 0.955 };

            let b0 =
                c0 * (ring_r0 * relief0) + axis * ring_z0;
            let b1 =
                c1 * (ring_r0 * relief1) + axis * ring_z0;

            let t0p =
                c0 * (ring_r1 * relief0) + axis * ring_z1;
            let t1p =
                c1 * (ring_r1 * relief1) + axis * ring_z1;

            let mid = (c0 * relief0 + c1 * relief1) * 0.5;

            polys.push(
                Polygon::quad(b0, b1, t1p, t0p)
                    .oriented(mid),
            );
        }
    }

    // Нижняя крышка.
    let center = axis * z_bottom;

    for i in 0..vertex_count {
        let next = (i + 1) % vertex_count;

        let a0 = angle_step * i as f32;
        let a1 = angle_step * next as f32;

        let relief0 = if i % 2 == 0 { 1.045 } else { 0.955 };
        let relief1 = if next % 2 == 0 { 1.045 } else { 0.955 };

        let p0 = center
            + (u * a0.cos() + v * a0.sin()) * (r0 * relief0);
        let p1 = center
            + (u * a1.cos() + v * a1.sin()) * (r0 * relief1);

        polys.push(
            Polygon::triangle(center, p0, p1)
                .oriented(-axis),
        );
    }

    polys
}

/// Curved saguaro tube: the mesh and point cloud use the same centerline and rings.
fn cactus_curved_geometry(
    p0: Vec3, p1: Vec3, r0: f32, r1: f32, sides: usize,
    start_axis: Vec3, end_axis: Vec3, is_tip: bool, step: f32,
) -> (Vec<Polygon>, Vec<Vec3>) {
    let delta = p1 - p0;
    let length = delta.length();
    let axis = delta / length;
    let cap_height = if is_tip { r1.min(length * 0.22) } else { 0.0 };
    let body_fraction = 1.0 - cap_height / length;
    let rings = ((length / 0.12).ceil() as usize).max(12);
    let vertex_count = sides.max(6) * 2;
    let mut faces = Vec::new();
    let mut cloud = Vec::new();
    let (reference_u, _) = basis(axis);

    let frame = |t: f32| -> (Vec3, Vec3, Vec3, Vec3) {
        let t2 = t * t;
        let t3 = t2 * t;
        let center = p0 * (2.0*t3 - 3.0*t2 + 1.0)
            + start_axis * (length * (t3 - 2.0*t2 + t))
            + p1 * (-2.0*t3 + 3.0*t2)
            + end_axis * (length * (t3 - t2));
        let derivative = p0 * (6.0*t2 - 6.0*t)
            + start_axis * (length * (3.0*t2 - 4.0*t + 1.0))
            + p1 * (-6.0*t2 + 6.0*t)
            + end_axis * (length * (3.0*t2 - 2.0*t));
        let tangent = derivative.normalize_or_zero();
        let tangent = if tangent.length_squared() > 1e-6 { tangent } else { axis };
        let u = (reference_u - tangent * reference_u.dot(tangent)).normalize_or_zero();
        let u = if u.length_squared() > 1e-6 { u } else { basis(tangent).0 };
        let v = tangent.cross(u).normalize();
        (center, tangent, u, v)
    };
    let ring_vertex = |t: f32, i: usize, radius: f32| -> Vec3 {
        let (center, _, u, v) = frame(t);
        let angle = std::f32::consts::TAU * i as f32 / vertex_count as f32;
        let relief = if i % 2 == 0 { 1.045 } else { 0.955 };
        center + (u * angle.cos() + v * angle.sin()) * (radius * relief)
    };
    for ring in 0..rings {
        let t0 = body_fraction * ring as f32 / rings as f32;
        let t1 = body_fraction * (ring + 1) as f32 / rings as f32;
        let r_start = r0 + (r1 - r0) * (t0 / body_fraction);
        let r_end = r0 + (r1 - r0) * (t1 / body_fraction);
        for i in 0..vertex_count {
            let j = (i + 1) % vertex_count;
            let a = ring_vertex(t0, i, r_start);
            let b = ring_vertex(t0, j, r_start);
            let c = ring_vertex(t1, j, r_end);
            let d = ring_vertex(t1, i, r_end);
            let center = frame((t0+t1)*0.5).0;
            let outward = ((a+b+c+d)*0.25-center).normalize_or_zero();
            faces.push(Polygon::quad(a,b,c,d).oriented(outward));
        }
    }
    if is_tip {
        let cap_rings = 6;
        let (base, tangent, u, v) = frame(body_fraction);
        for ring in 0..cap_rings {
            let a0 = std::f32::consts::FRAC_PI_2 * ring as f32 / cap_rings as f32;
            let a1 = std::f32::consts::FRAC_PI_2 * (ring+1) as f32 / cap_rings as f32;
            for i in 0..vertex_count {
                let j = (i+1)%vertex_count;
                let point = |a: f32, k: usize| {
                    let theta = std::f32::consts::TAU * k as f32 / vertex_count as f32;
                    let relief = if k%2==0 {1.045} else {0.955};
                    base + tangent * (cap_height * a.sin())
                        + (u*theta.cos()+v*theta.sin()) * (r1*a.cos()*relief)
                };
                let a=point(a0,i); let b=point(a0,j);
                let c=point(a1,j); let d=point(a1,i);
                let outward = ((a+b+c+d)*0.25 - (base+tangent*cap_height*0.5)).normalize_or_zero();
                faces.push(Polygon::quad(a,b,c,d).oriented(outward));
            }
        }
    }
    // Cloud points follow the same curved tube rather than straight skeleton edges.
    if step > 1e-4 {
        let samples = ((length / step).ceil() as usize).max(1);
        for k in 0..=samples {
            let t = body_fraction * k as f32 / samples as f32;
            let radius = r0 + (r1-r0) * (t/body_fraction);
            let count = ((std::f32::consts::TAU * radius / step).round() as usize).max(2);
            let (center, _, u, v) = frame(t);
            for i in 0..count {
                let angle = std::f32::consts::TAU * i as f32 / count as f32;
                cloud.push(center + (u*angle.cos()+v*angle.sin())*radius);
            }
        }
        if is_tip {
            let (base,tangent,u,v) = frame(body_fraction);
            let cap_samples = ((cap_height/step).ceil() as usize).max(5);
            for k in 1..=cap_samples {
                let angle = std::f32::consts::FRAC_PI_2 * k as f32 / cap_samples as f32;
                let radius = r1 * angle.cos();
                let center = base + tangent * (cap_height * angle.sin());
                let count = ((std::f32::consts::TAU * radius / step).round() as usize).max(2);
                for i in 0..count {
                    let theta = std::f32::consts::TAU * i as f32 / count as f32;
                    cloud.push(center + (u*theta.cos()+v*theta.sin())*radius);
                }
            }
        }
    }
    (faces,cloud)
}

/// Неровный древесный ствол юкки.
/// Несколько колец по высоте создают естественный рельеф
/// и следы от старых оснований листьев.
fn yucca_trunk_polygons(
    r0: f32,
    r1: f32,
    height: f32,
    sides: usize,
    axis: Vec3,
) -> Vec<Polygon> {
    let (u, v) = basis(axis);

    let sides = sides.max(10);
    let rings = 9usize;
    let angle_step = std::f32::consts::TAU / sides as f32;

    let mut polys = Vec::new();

    for ring in 0..rings {
        let t0 = ring as f32 / rings as f32;
        let t1 = (ring + 1) as f32 / rings as f32;

        let z0 = -height * 0.5 + height * t0;
        let z1 = -height * 0.5 + height * t1;

        // Основной силуэт: широкий снизу и узкий сверху.
        let base_r0 = r0 * (1.0 - t0) + r1 * t0;
        let base_r1 = r0 * (1.0 - t1) + r1 * t1;

        for i in 0..sides {
            let next = (i + 1) % sides;

            let a0 = angle_step * i as f32;
            let a1 = angle_step * next as f32;

            let c0 = u * a0.cos() + v * a0.sin();
            let c1 = u * a1.cos() + v * a1.sin();

            // Небольшая нерегулярность поверхности.
            let relief00 =
                1.0 + 0.045 * (a0 * 3.0 + ring as f32 * 1.7).sin();
            let relief01 =
                1.0 + 0.045 * (a1 * 3.0 + ring as f32 * 1.7).sin();

            let relief10 =
                1.0 + 0.045 * (a0 * 3.0 + (ring + 1) as f32 * 1.7).sin();
            let relief11 =
                1.0 + 0.045 * (a1 * 3.0 + (ring + 1) as f32 * 1.7).sin();

            // Верхняя часть ствола немного грубее:
            // имитация старых оснований листьев.
            let upper_relief = if t1 > 0.55 {
                1.015
            } else {
                1.0
            };

            let b0 =
                c0 * (base_r0 * relief00) + axis * z0;
            let b1 =
                c1 * (base_r0 * relief01) + axis * z0;

            let t0p =
                c0 * (base_r1 * relief10 * upper_relief) + axis * z1;
            let t1p =
                c1 * (base_r1 * relief11 * upper_relief) + axis * z1;

            let outward = (c0 + c1).normalize_or_zero();

            polys.push(
                Polygon::quad(b0, b1, t1p, t0p)
                    .oriented(outward),
            );
        }
    }

    // Нижняя крышка.
    let bottom_center = -axis * (height * 0.5);

    for i in 0..sides {
        let next = (i + 1) % sides;

        let a0 = angle_step * i as f32;
        let a1 = angle_step * next as f32;

        let p0 = bottom_center
            + (u * a0.cos() + v * a0.sin()) * r0;

        let p1 = bottom_center
            + (u * a1.cos() + v * a1.sin()) * r0;

        polys.push(
            Polygon::triangle(bottom_center, p0, p1)
                .oriented(-axis),
        );
    }

    polys
}

/// Массивный ствол дуба с расширенным комлем и лёгкой неровностью поверхности.
fn oak_trunk_polygons(
    r0: f32,
    r1: f32,
    height: f32,
    sides: usize,
    axis: Vec3,
) -> Vec<Polygon> {
    let (u, v) = basis(axis);

    let sides = sides.max(12);
    let rings = 8usize;
    let angle_step = std::f32::consts::TAU / sides as f32;

    let mut polys = Vec::new();

    for ring in 0..rings {
        let t0 = ring as f32 / rings as f32;
        let t1 = (ring + 1) as f32 / rings as f32;

        let z0 = -height * 0.5 + height * t0;
        let z1 = -height * 0.5 + height * t1;

        // У дуба выраженный широкий комель.
        // Выше ствол постепенно переходит к обычному радиусу.
        let flare0 = 1.0 + 0.42 * (1.0 - t0).powf(4.0);
        let flare1 = 1.0 + 0.42 * (1.0 - t1).powf(4.0);

        let base_r0 = (r0 * (1.0 - t0) + r1 * t0) * flare0;
        let base_r1 = (r0 * (1.0 - t1) + r1 * t1) * flare1;

        for i in 0..sides {
            let next = (i + 1) % sides;

            let a0 = angle_step * i as f32;
            let a1 = angle_step * next as f32;

            let c0 = u * a0.cos() + v * a0.sin();
            let c1 = u * a1.cos() + v * a1.sin();

            // Небольшая нерегулярность делает ствол менее цилиндрическим.
            let relief00 =
                1.0 + 0.055 * (a0 * 3.0 + ring as f32 * 1.35).sin();
            let relief01 =
                1.0 + 0.055 * (a1 * 3.0 + ring as f32 * 1.35).sin();

            let relief10 =
                1.0 + 0.055 * (a0 * 3.0 + (ring + 1) as f32 * 1.35).sin();
            let relief11 =
                1.0 + 0.055 * (a1 * 3.0 + (ring + 1) as f32 * 1.35).sin();

            let b0 = c0 * (base_r0 * relief00) + axis * z0;
            let b1 = c1 * (base_r0 * relief01) + axis * z0;

            let t0p = c0 * (base_r1 * relief10) + axis * z1;
            let t1p = c1 * (base_r1 * relief11) + axis * z1;

            let outward = (c0 + c1).normalize_or_zero();

            polys.push(
                Polygon::quad(b0, b1, t1p, t0p)
                    .oriented(outward),
            );
        }
    }

    // Закрываем основание ствола.
    let bottom_center = -axis * (height * 0.5);
    let bottom_radius = r0 * 1.42;

    for i in 0..sides {
        let next = (i + 1) % sides;

        let a0 = angle_step * i as f32;
        let a1 = angle_step * next as f32;

        let p0 =
            bottom_center + (u * a0.cos() + v * a0.sin()) * bottom_radius;
        let p1 =
            bottom_center + (u * a1.cos() + v * a1.sin()) * bottom_radius;

        polys.push(
            Polygon::triangle(bottom_center, p0, p1)
                .oriented(-axis),
        );
    }

    polys
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

