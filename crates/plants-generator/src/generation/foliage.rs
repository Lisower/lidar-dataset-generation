//! Крона растения: листья, навешиваемые на скелет веток.
//!
//! Строит крону по правилам [`FoliageConfig`]: `Leafy` + `Terminal` — плоские
//! листья-ромбики на концах терминальных веток (дерево, куст).
//!
//! Результат — набор полигонов (для визуального меша) и позиций облака точек
//! в локальной системе координат растения.

use bevy::math::Vec3;

use rand::RngExt;
use rand::rngs::StdRng;

use generation_core::geometry::Polygon;

use super::config::{FoliageConfig, FoliagePlacement, PlantConfig, PlantPreset};
use super::skeleton::is_terminal;
use super::skeleton::PlantSkeleton;
use super::skeleton::SkeletonSegment;

/// Результат построения кроны.
#[derive(Debug, Clone, Default)]
pub struct Foliage {
    /// Полигоны листьев/игл (локальные координаты растения).
    pub faces: Vec<Polygon>,
    /// Позиции облака точек кроны.
    pub cloud: Vec<Vec3>,
    /// Число листьев/игл.
    pub count: usize,
}

/// Построить крону растения из скелета (детерминированно от seed).
pub fn build_foliage(skeleton: &PlantSkeleton, config: &PlantConfig) -> Foliage {
    if config.preset == PlantPreset::Spruce {
        build_spruce_needles(skeleton, config)
    } else if config.preset == PlantPreset::Juniper {
        build_juniper_needles(skeleton, config)
    } else {
        build_leafy(skeleton, config)
    }
}

/// Хвоя ели.
///
/// Иглы располагаются вдоль ветвей, а не только на их концах.
/// Это создаёт плотную хвойную крону по всей длине еловых лап.
fn build_spruce_needles(
    skeleton: &PlantSkeleton,
    config: &PlantConfig,
) -> Foliage {
    let mut rng = config.seed.with_label("spruce_needles").rng();
    //let cfg = &config.foliage;

    let mut foliage = Foliage::default();

    for segment in &skeleton.segments {
        let from = skeleton.points[segment.from];
        let to = skeleton.points[segment.to];

        // Ствол не покрываем хвоей.
        // Основные и боковые ветви ели начинаются с level >= 1.
        if to.level == 0 || to.branch_id == 0 {
            continue;
        }

        let branch = to.position - from.position;
        let length = branch.length();

        if length <= 0.001 {
            continue;
        }

        let dir = branch.normalize();

        // Чем длиннее сегмент, тем больше точек с хвоей.
        let anchors =
            ((length / 0.12).ceil() as usize).clamp(4, 30);

        for anchor_index in 0..anchors {
            let t =
                (anchor_index as f32 + 0.5) / anchors as f32;

            let center = from.position + branch * t;

            // Несколько игл вокруг каждой точки ветви.
            for needle_index in 0..10 {
                let angle =
                    needle_index as f32
                        * std::f32::consts::TAU
                        / 10.0
                        + rng.random_range(-0.25..=0.25);

                let up = Vec3::Z;
                let side0 = dir.cross(up).normalize_or_zero();

                let side0 = if side0 == Vec3::ZERO {
                    Vec3::X
                } else {
                    side0
                };

                let pitch =
                    dir.cross(side0).normalize_or_zero();

                let radial =
                    side0 * angle.cos()
                        + pitch * angle.sin();

                // Игла немного направлена наружу и вперёд.
                let needle_dir =
                    (radial * 0.85 + dir * 0.25)
                        .normalize_or_zero();

                let needle_length =
                    rng.random_range(0.14..=0.20);

                let needle_width = 0.028;

                // Немного отодвигаем иглу от центральной оси ветки.
                // Так хвоя образует объём вокруг еловой лапы,
                // а коричневая ветка меньше просматривается.
                let foliage_radius =
                    rng.random_range(0.015..=0.045);

                let needle_center =
                    center + radial * foliage_radius;

                let tip =
                    needle_center + needle_dir * needle_length;

                let c0 = tip;
                let c1 =
                    needle_center + side0 * needle_width;
                let c2 =
                    needle_center
                        - needle_dir * needle_length * 0.15;
                let c3 =
                    needle_center - side0 * needle_width;

                let [c0, c1, c2, c3] =
                    on_ground([c0, c1, c2, c3]);

                foliage.faces.push(Polygon::quad(
                    c0, c1, c2, c3,
                ));

                foliage.cloud.push(c0);
                foliage.cloud.push(needle_center);

                foliage.count += 1;
            }
        }
    }

    foliage
}

/// Хвоя можжевельника.
///
/// Короткие густые иглы располагаются по всей длине ветвей,
/// формируя плотную объёмную крону и скрывая большую часть скелета.
fn build_juniper_needles(
    skeleton: &PlantSkeleton,
    config: &PlantConfig,
) -> Foliage {
    let mut rng = config.seed.with_label("juniper_needles").rng();
    let mut foliage = Foliage::default();

    for segment in &skeleton.segments {
        let from = skeleton.points[segment.from];
        let to = skeleton.points[segment.to];

        let branch = to.position - from.position;
        let length = branch.length();

        if length <= 0.001 {
            continue;
        }

        let dir = branch.normalize();

        // Расставляем точки с хвоей достаточно часто по всей ветке.
        // На длинных основных побегах их больше, на коротких — меньше.
        let anchors = ((length / 0.035).ceil() as usize).clamp(4, 40);

        for anchor_index in 0..anchors {
            let t =
                (anchor_index as f32 + 0.5) / anchors as f32;

            let center = from.position + branch * t;

            // Строим базис вокруг направления ветки.
            let up = Vec3::Z;
            let side0 = dir.cross(up).normalize_or_zero();

            let side0 = if side0 == Vec3::ZERO {
                Vec3::X
            } else {
                side0
            };

            let pitch =
                dir.cross(side0).normalize_or_zero();

            // Несколько коротких игл вокруг каждой точки ветви.
            for needle_index in 0..8 {
                let angle =
                    needle_index as f32
                        * std::f32::consts::TAU
                        / 8.0
                        + rng.random_range(-0.30_f32..=0.30_f32);

                let radial =
                    side0 * angle.cos()
                        + pitch * angle.sin();

                // Игла смотрит преимущественно наружу,
                // но немного направлена вдоль самой ветви.
                let needle_dir =
                    (radial * 0.90 + dir * 0.20)
                        .normalize_or_zero();

                // Короткая мелкая хвоя можжевельника.
                let needle_length =
                    rng.random_range(0.07_f32..=0.11_f32);

                let needle_width =
                    rng.random_range(0.012_f32..=0.020_f32);

                // Немного отодвигаем хвою от оси ветки,
                // чтобы зелень образовывала объёмный слой.
                let foliage_radius =
                    rng.random_range(0.015_f32..=0.045_f32);

                let needle_center =
                    center + radial * foliage_radius;

                let tip =
                    needle_center + needle_dir * needle_length;

                let c0 = tip;
                let c1 =
                    needle_center + side0 * needle_width;
                let c2 =
                    needle_center
                        - needle_dir * needle_length * 0.18;
                let c3 =
                    needle_center - side0 * needle_width;

                let [c0, c1, c2, c3] =
                    on_ground([c0, c1, c2, c3]);

                foliage.faces.push(Polygon::quad(
                    c0, c1, c2, c3,
                ));

                // Две точки на каждую иглу дают достаточно плотное
                // облако без чрезмерного увеличения его размера.
                foliage.cloud.push(c0);
                foliage.cloud.push(needle_center);

                foliage.count += 1;
            }
        }
    }

    foliage
}

/// Листья-ромбики на терминальных ветках уровня >= `min_level`.
fn build_leafy(skeleton: &PlantSkeleton, config: &PlantConfig) -> Foliage {
    let mut rng = config.seed.with_label("foliage").rng();
    let cfg = &config.foliage;

    // Единственный способ размещения кроны — на терминальных ветках.
    let FoliagePlacement::Terminal { min_level } = cfg.placement;

    let mut foliage = Foliage::default();
    for segment in &skeleton.segments {
        if skeleton.points[segment.to].level < min_level {
            continue;
        }
        if !is_terminal(skeleton, segment) {
            continue;
        }
        for _ in 0..cfg.per_anchor.max(1) {
            let leaf = leaf_quad(skeleton, segment, cfg, &mut rng);
            let cloud = leaf_cloud_points(&leaf, cfg.cloud_points);
            foliage.faces.push(leaf.polygon);
            foliage.cloud.extend(cloud);
            foliage.count += 1;
        }
    }
    foliage
}

/// Один лист: полигон + опорные точки для облака.
struct LeafQuad {
    /// До четырёх опорных точек (кончик, основание и т.п.).
    corners: [Vec3; 4],
    /// Плоский полигон.
    polygon: Polygon,
}

/// Один лист-ромбик на конце терминальной ветки.
fn leaf_quad(
    skeleton: &PlantSkeleton,
    segment: &SkeletonSegment,
    cfg: &FoliageConfig,
    rng: &mut StdRng,
) -> LeafQuad {
    let tip = skeleton.points[segment.to].position;
    let base = skeleton.points[segment.from].position;
    let dir = (tip - base).normalize_or_zero();
    let dir = if dir == Vec3::ZERO { Vec3::Z } else { dir };

    // Базис, перпендикулярный ветке: `side` и `pitch`.
    let up = Vec3::Z;
    let side0 = dir.cross(up).normalize_or_zero();
    let (side0, pitch0) = if side0 == Vec3::ZERO {
        (Vec3::X, Vec3::Y)
    } else {
        (side0, dir.cross(side0).normalize_or_zero())
    };

    // Поворот листа вокруг оси ветки: крона получается объёмной.
    let rot = rng.random_range(0.0..std::f32::consts::TAU);
    let side = side0 * rot.cos() + pitch0 * rot.sin();
    let pitch = pitch0 * rot.cos() - side0 * rot.sin();

    // Лист смотрит наружу-вверх от ветки.
    let out = (dir + pitch * 0.55 + side * 0.15).normalize_or_zero();

    let len = cfg.length;
    let wid = cfg.width;

    let c0 = tip + out * len; // кончик
    let c1 = tip + side * wid * 0.5; // правая
    let c2 = tip - out * len * 0.3; // основание (чуть назад)
    let c3 = tip - side * wid * 0.5; // левая

    // Лист не должен уходить под землю (z = 0): зажимаем углы на уровень
    // земли — так облако и визуальный меш остаются на/над плоскостью.
    let [c0, c1, c2, c3] = on_ground([c0, c1, c2, c3]);

    let polygon = Polygon::quad(c0, c1, c2, c3);
    LeafQuad {
        corners: [c0, c1, c2, c3],
        polygon,
    }
}

/// Зажать Z всех точек на уровень земли (z >= 0).
fn on_ground(mut pts: [Vec3; 4]) -> [Vec3; 4] {
    for p in &mut pts {
        p.z = p.z.max(0.0);
    }
    pts
}

/// Точки облака для одного листа: опорные вершины (не более `count`).
fn leaf_cloud_points(leaf: &LeafQuad, count: usize) -> Vec<Vec3> {
    let count = count.clamp(1, 4);
    leaf.corners[..count].to_vec()
}
