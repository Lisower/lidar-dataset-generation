//! Скелет ветвлений растения: точки и рёбра.
//!
//! Логика повторяет прототип (`tree_skeleton.rs`):
//! - длина и радиус дочерней ветки умножаются на родительские, а вертикаль
//!   (угол от оси Z) и азимут НАКАПЛИВАЮТСЯ в глобальных сферических
//!   координатах. Именно накопление «разворачивает» ветки и даёт ажурную
//!   крону, а не кусты из веток, продолжающих родителя «в лоб»;
//! - окно кандидатов дочерних веток — все `children_max` веток уровня подряд
//!   от `first_child_index` (в прототипе окно было на единицу короче, из-за
//!   чего «правая» ветка тройки «продолжить/−поворот/+поворот» была
//!   недостижима, и азимут систематически дрейфовал — крона закручивалась
//!   спиралью в одну сторону при виде сверху);
//! - количество веток ограничено ([`PlantConfig::max_branches`]) — широкий
//!   предохранитель, чтобы дерево не разрасталось безгранично;
//! - детерминизм от [`PlantConfig::seed`]: один поток `StdRng` на всё растение.

use bevy::math::Vec3;

use rand::RngExt;
use rand::rngs::StdRng;

use super::config::{BranchGen, PlantConfig, PlantPreset};

/// Точка скелета — конец сегмента ветки.
#[derive(Debug, Clone, Copy)]
pub struct SkeletonPoint {
    /// Положение в локальной системе координат растения (ось Z — вверх).
    pub position: Vec3,
    /// Уровень ветки в иерархии.
    pub level: usize,
    /// Номер ветки в уровне.
    pub branch_id: usize,
    /// Нижний радиус сегмента, которым оканчивается точка, м.
    pub radius: f32,
    /// Сужение верхнего конца сегмента.
    pub taper: f32,
    /// Число дочерних веток (0 — терминальная).
    pub children_count: usize,
}

/// Сегмент ветки между двумя точками скелета.
#[derive(Debug, Clone, Copy)]
pub struct SkeletonSegment {
    /// Индекс точки-основания.
    pub from: usize,
    /// Индекс точки-конца.
    pub to: usize,
}

/// Скелет ветвлений растения.
#[derive(Debug, Clone, Default)]
pub struct PlantSkeleton {
    pub points: Vec<SkeletonPoint>,
    pub segments: Vec<SkeletonSegment>,
    /// Максимальное горизонтальное удаление от оси растения, м.
    pub ground_radius: f32,
    /// Полная высота растения, м.
    pub height: f32,
}

impl PlantSkeleton {
    /// Построить скелет из конфигурации (детерминированно от seed).
    pub fn build(config: &PlantConfig) -> Self {
        if config.preset == PlantPreset::Spruce {
            return Self::build_spruce(config);
        }
        if config.preset == PlantPreset::Juniper {
            return Self::build_juniper(config);
        }
        if config.preset == PlantPreset::CactusSmall {
            return Self::build_cactus_small(config);
        }
        if config.preset == PlantPreset::Yucca {
            return Self::build_yucca(config);
        }
        if config.preset == PlantPreset::Agave {
            return Self::build_agave(config);
        }
        if config.preset == PlantPreset::Oak {
            return Self::build_oak(config);
        }
        let mut rng = config.seed.with_label("skeleton").rng();

        let mut skeleton = Self::default();

        // Корневая точка (основание растения на земле, z = 0).
        skeleton.points.push(SkeletonPoint {
            position: Vec3::ZERO,
            level: 0,
            branch_id: 0,
            radius: 0.0,
            taper: 1.0,
            children_count: 0,
        });

        if let Some(level0) = config.branches.first() {
            for branch in level0 {
                if skeleton.points.len() >= config.max_branches {
                    break;
                }
                grow(
                    &mut rng,
                    config,
                    0,
                    branch,
                    1.0, // «длина родителя» для уровня 0 не нужна
                    0.0, // «вертикаль родителя»
                    0.0, // «азимут родителя»
                    1.0, // «радиус родителя»
                    1.0, // «тапер родителя»
                    &mut skeleton,
                );
            }
        }

        Self::finish(&mut skeleton);
        skeleton
    }

        /// Специальное построение ели:
        /// вертикальный ствол, ярусы основных ветвей и боковые веточки.
        fn build_spruce(config: &PlantConfig) -> Self {
            let mut rng = config.seed.with_label("spruce_skeleton").rng();
            let mut skeleton = Self::default();

            // Основание дерева.
            skeleton.points.push(SkeletonPoint {
                position: Vec3::ZERO,
                level: 0,
                branch_id: 0,
                radius: 0.0,
                taper: 1.0,
                children_count: 1,
            });

            // Высота и толщина ствола немного меняются от seed.
            let height = rng.random_range(10.0..=16.0);
            let trunk_radius = rng.random_range(0.28..=0.42);

            // Основные ярусы кроны.
            // Верхушка строится отдельно как продолжение центральной оси.
            let tiers = 15usize;

            let mut previous_trunk_point = 0usize;

            // Строим вертикальный ствол по частям.
            for tier in 1..=tiers {
                let t = tier as f32 / tiers as f32;
                // Ярусы слегка смещаются по высоте,
                // чтобы крона не выглядела как ровная лестница.
                let tier_jitter =
                    rng.random_range(-0.18..=0.18);

                let z =
                    (height * t * 0.72 + tier_jitter * 0.5)
                        .clamp(0.5, height * 0.72);

                let trunk_point = skeleton.points.len();

                skeleton.points.push(SkeletonPoint {
                    position: Vec3::new(0.0, 0.0, z),
                    level: tier,
                    branch_id: 0,
                    radius: trunk_radius * (1.0 - t * 0.75),
                    taper: 0.9,
                    children_count: 0,
                });

                skeleton.segments.push(SkeletonSegment {
                    from: previous_trunk_point,
                    to: trunk_point,
                });

                // Нижние ветви длиннее, верхние короче.
                let crown_factor = 1.0 - t;
                let base_branch_length = match tier {
                    15 => 0.55,
                    14 => 0.85,
                    13 => 1.20,
                    _ => 0.7 + crown_factor * 4.5,
                };

                let branch_count = match tier {
                    15 => 5usize,
                    14 => 6usize,
                    13 => 7usize,
                    _ => rng.random_range(7usize..=10usize),
                };

                // Каждый ярус немного поворачиваем относительно предыдущего,
                // чтобы ветви не располагались строгими этажами.
                let tier_offset =
                    tier as f32 * 17.0 + rng.random_range(-12.0..=12.0);

                for branch_index in 0..branch_count {
                    // Немного изменяем направление каждой ветви,
                    // чтобы крона не выглядела слишком геометрической.
                    let azimuth =
                        tier_offset
                            + branch_index as f32 * (360.0 / branch_count as f32)
                            + rng.random_range(-10.0..=10.0);

                    // Ветви одного яруса немного различаются по длине.
                    let branch_length =
                        base_branch_length * rng.random_range(0.82..=1.12);

                    // Нижние ветви почти горизонтальные,
                    // верхние сильнее направлены вверх.
                    let vertical_deg = match tier {
                        15 => rng.random_range(48.0_f32..=54.0_f32),
                        14 => rng.random_range(52.0_f32..=58.0_f32),
                        13 => rng.random_range(56.0_f32..=62.0_f32),
                        _ => 82.0 - t * 25.0 + rng.random_range(-6.0..=6.0),
                    };

                    let v = vertical_deg.to_radians();
                    let h = azimuth.to_radians();

                    let direction = Vec3::new(
                        v.sin() * h.cos(),
                        v.sin() * h.sin(),
                        v.cos(),
                    );

                    let start_position = skeleton.points[trunk_point].position;
                    let end_position = start_position + direction * branch_length;

                    // Конец основной ветви.
                    let branch_point = skeleton.points.len();

                    skeleton.points.push(SkeletonPoint {
                        position: end_position,
                        level: tier,
                        branch_id: branch_index + 1,
                        radius: 0.08 * (1.0 - t * 0.5),
                        taper: 0.75,
                        children_count: 0,
                    });

                    skeleton.segments.push(SkeletonSegment {
                        from: trunk_point,
                        to: branch_point,
                    });

                    // Нижняя часть ели густая, к вершине количество
                    // боковых веточек постепенно уменьшается.
                    let (side_min, side_max) = if t < 0.35 {
                        (12usize, 16usize)
                    } else if t < 0.70 {
                        (9usize, 13usize)
                    } else {
                        (5usize, 8usize)
                    };

                    let side_branches = rng.random_range(side_min..=side_max);

                    for side_index in 1..=side_branches {
                        let along =
                            side_index as f32 / (side_branches + 1) as f32;

                        let attach_position =
                            start_position + direction * branch_length * along;

                        // Ближе к концу основной ветви боковые веточки короче.
                        let side_length =
                            branch_length
                                * (0.22 - along * 0.08)
                                * rng.random_range(0.75..=1.15);

                        // Чередуем веточки по двум сторонам основной ветви.
                        let side_spread = rng.random_range(50.0..=75.0);
                        let side_jitter = rng.random_range(-8.0..=8.0);

                        let side_angle = if side_index % 2 == 0 {
                            azimuth + side_spread + side_jitter
                        } else {
                            azimuth - side_spread + side_jitter
                        };

                        let side_h = side_angle.to_radians();
                        let side_vertical =
                            (78.0 - t * 15.0).to_radians();

                        let side_direction = Vec3::new(
                            side_vertical.sin() * side_h.cos(),
                            side_vertical.sin() * side_h.sin(),
                            side_vertical.cos(),
                        );

                        let side_end =
                            attach_position + side_direction * side_length;

                        // Точка крепления боковой веточки.
                        let attach_idx = skeleton.points.len();

                        skeleton.points.push(SkeletonPoint {
                            position: attach_position,
                            level: tier,
                            branch_id: branch_index + 1,
                            radius: 0.035 * (1.0 - t * 0.4),
                            taper: 0.75,
                            children_count: 1,
                        });

                        skeleton.segments.push(SkeletonSegment {
                            from: trunk_point,
                            to: attach_idx,
                        });

                        // Конец боковой веточки.
                        let side_idx = skeleton.points.len();

                        skeleton.points.push(SkeletonPoint {
                            position: side_end,
                            level: tier + 1,
                            branch_id: side_index,
                            radius: 0.025 * (1.0 - t * 0.4),
                            taper: 0.7,
                            children_count: 0,
                        });

                        skeleton.segments.push(SkeletonSegment {
                            from: attach_idx,
                            to: side_idx,
                        });
                    }
                }

                // Учитываем продолжение ствола и основные ветви.
                skeleton.points[trunk_point].children_count =
                    if tier < tiers {
                        branch_count + 1
                    } else {
                        branch_count
                    };

                previous_trunk_point = trunk_point;
            }

            // Верхняя часть ели: центральная ось продолжается несколькими
            // короткими сегментами, а боковые ветви постепенно уменьшаются.
            // Так макушка плавно сужается, а не образует отдельные ярусы.
            let top_levels = 4usize;

            for top_level in 1..=top_levels {
                let progress = top_level as f32 / top_levels as f32;

                let from = previous_trunk_point;
                let start_position = skeleton.points[from].position;

                // К вершине сегменты центральной оси становятся немного короче.
                let trunk_step = 0.22 - progress * 0.04;

                let trunk_idx = skeleton.points.len();

                skeleton.points.push(SkeletonPoint {
                    position: start_position + Vec3::new(0.0, 0.0, trunk_step),
                    level: tiers + top_level,
                    branch_id: 0,
                    radius: trunk_radius * (0.16 - progress * 0.10),
                    taper: 0.65,
                    children_count: 0,
                });

                skeleton.segments.push(SkeletonSegment {
                    from,
                    to: trunk_idx,
                });

                // На последнем уровне остаётся только терминальный
                // центральный побег — сама макушка.
                let branch_count = match top_level {
                    1 => 6usize,
                    2 => 5usize,
                    3 => 4usize,
                    _ => 0usize,
                };

                let branch_length = match top_level {
                    1 => 0.55,
                    2 => 0.38,
                    3 => 0.22,
                    _ => 0.0,
                };

                let tier_offset =
                    top_level as f32 * 31.0 + rng.random_range(-10.0..=10.0);

                for branch_index in 0..branch_count {
                    let azimuth =
                        tier_offset
                            + branch_index as f32 * (360.0 / branch_count as f32)
                            + rng.random_range(-8.0..=8.0);

                    // Верхние ветви направлены вверх и наружу.
                    let vertical_deg: f32 =
                        rng.random_range(48.0..=60.0);

                    let v = vertical_deg.to_radians();
                    let h = azimuth.to_radians();

                    let direction = Vec3::new(
                        v.sin() * h.cos(),
                        v.sin() * h.sin(),
                        v.cos(),
                    );

                    let length =
                        branch_length * rng.random_range(0.88..=1.08);

                    let branch_idx = skeleton.points.len();

                    skeleton.points.push(SkeletonPoint {
                        position:
                            skeleton.points[trunk_idx].position
                                + direction * length,
                        level: tiers + top_level,
                        branch_id: branch_index + 1,
                        radius: 0.025,
                        taper: 0.65,
                        children_count: 0,
                    });

                    skeleton.segments.push(SkeletonSegment {
                        from: trunk_idx,
                        to: branch_idx,
                    });
                }

                // Центральная ось продолжается до следующего уровня.
                // На последнем уровне она становится терминальной.
                skeleton.points[trunk_idx].children_count =
                    if top_level < top_levels {
                        branch_count + 1
                    } else {
                        branch_count
                    };

                previous_trunk_point = trunk_idx;
            }     
            
            Self::finish(&mut skeleton);
            skeleton
        }

        /// Специальное построение маленького кустового кактуса:
        /// несколько толстых ребристых побегов растут отдельной группой от земли.
        fn build_cactus_small(config: &PlantConfig) -> Self {
            let mut rng = config.seed.with_label("cactus_small_skeleton").rng();
            let mut skeleton = Self::default();

            // Служебная центральная точка растения.
            skeleton.points.push(SkeletonPoint {
                position: Vec3::ZERO,
                level: 0,
                branch_id: 0,
                radius: 0.0,
                taper: 1.0,
                children_count: 0,
            });

            // Количество отдельных мясистых побегов меняется от seed.
            let shoot_count = rng.random_range(6usize..=9usize);

            for shoot_index in 0..shoot_count {
                if skeleton.points.len() >= config.max_branches {
                    break;
                }

                // Центральные побеги располагаются ближе к середине,
                // остальные образуют вокруг них компактную группу.
                let base_radius = if shoot_index < 2 {
                    rng.random_range(0.0_f32..=0.07_f32)
                } else {
                    rng.random_range(0.22_f32..=0.40_f32)
                };

                let base_angle =
                    shoot_index as f32 * (std::f32::consts::TAU / shoot_count as f32)
                        + rng.random_range(0.08_f32..=0.18_f32);

                let start_position = Vec3::new(
                    base_radius * base_angle.cos(),
                    base_radius * base_angle.sin(),
                    0.0,
                );

                // Центральные побеги выше, наружные немного короче.
                let height = if shoot_index < 2 {
                    rng.random_range(0.95_f32..=1.30_f32)
                } else {
                    rng.random_range(0.55_f32..=1.05_f32)
                };

                // Небольшой наклон наружу делает куст естественнее,
                // но побеги остаются преимущественно вертикальными.
                let lean = if shoot_index < 2 {
                    rng.random_range(0.0_f32..=0.04_f32)
                } else {
                    rng.random_range(0.08_f32..=0.18_f32)
                };

                let lean_angle =
                    base_angle + rng.random_range(-0.25_f32..=0.25_f32);

                let horizontal = height * lean;

                let end_position = Vec3::new(
                    start_position.x + horizontal * lean_angle.cos(),
                    start_position.y + horizontal * lean_angle.sin(),
                    height,
                );

                // Немного различаем толщину отдельных побегов.
                let radius = if shoot_index < 2 {
                    rng.random_range(0.14_f32..=0.18_f32)
                } else {
                    rng.random_range(0.10_f32..=0.16_f32)
                };

                // Отдельная точка основания нужна, чтобы побеги действительно
                // начинались в разных местах, а не сходились в один центр.
                let base_idx = skeleton.points.len();

                skeleton.points.push(SkeletonPoint {
                    position: start_position,
                    level: 0,
                    branch_id: shoot_index,
                    radius,
                    taper: 0.92,
                    children_count: 1,
                });

                let end_idx = skeleton.points.len();

                skeleton.points.push(SkeletonPoint {
                    position: end_position,
                    level: 0,
                    branch_id: shoot_index,
                    radius,
                    taper: rng.random_range(0.86_f32..=0.93_f32),
                    children_count: 0,
                });

                skeleton.segments.push(SkeletonSegment {
                    from: base_idx,
                    to: end_idx,
                });
            }

            skeleton.points[0].children_count = shoot_count;

            Self::finish(&mut skeleton);
            skeleton
        }

        fn build_yucca(config: &PlantConfig) -> Self {
        let mut rng: StdRng = config.seed.with_label("yucca_skeleton").rng();
        let mut skeleton: PlantSkeleton = Self::default();

        // Основание ствола.
        skeleton.points.push(SkeletonPoint {
            position: Vec3::ZERO,
            level: 0,
            branch_id: 0,
            radius: rng.random_range(0.23_f32..=0.29_f32),
            taper: 1.0,
            children_count: 1,
        });

        // Верхушка короткого древесного ствола.
        let trunk_height = rng.random_range(0.65_f32..=0.90_f32);
        let top_idx = skeleton.points.len();

        skeleton.points.push(SkeletonPoint {
            position: Vec3::new(0.0, 0.0, trunk_height),
            level: 0,
            branch_id: 0,
            radius: skeleton.points[0].radius * 0.48,
            taper: 0.48,
            children_count: 0,
        });

        skeleton.segments.push(SkeletonSegment {
            from: 0,
            to: top_idx,
        });

        Self::finish(&mut skeleton);
        skeleton
    }

    fn build_agave(config: &PlantConfig) -> Self {
    let mut rng = config.seed.with_label("agave_skeleton").rng();

    let mut skeleton = Self {
        points: Vec::new(),
        segments: Vec::new(),
        height: 0.0,
        ground_radius: 0.0,
    };

    // Агава растёт непосредственно от земли.
    // Небольшой центральный сегмент нужен только как точка,
    // от которой будет строиться прикорневая розетка листьев.
    let base_radius = rng.random_range(0.10_f32..=0.14_f32);
    let center_height = rng.random_range(0.035_f32..=0.055_f32);

    skeleton.points.push(SkeletonPoint {
        position: Vec3::ZERO,
        radius: base_radius,
        taper: 1.0,
        level: 0,
        branch_id: 0,
        children_count: 0,
    });

    skeleton.points.push(SkeletonPoint {
        position: Vec3::Z * center_height,
        radius: base_radius * 0.85,
        taper: 0.85,
        level: 0,
        branch_id: 0,
        children_count: 0,
    });

    skeleton.segments.push(SkeletonSegment {
        from: 0,
        to: 1,
    });

    // Размеры учитывают прикорневую розетку агавы, а не только
    // технический центральный сегмент скелета.
    skeleton.height = 0.75;
    skeleton.ground_radius = 0.95;

    skeleton
}

/// Специальное построение дуба:
/// мощный ствол и плотная округлая крона с ветвями,
/// распределёнными по всей её высоте.
fn build_oak(config: &PlantConfig) -> Self {
    let mut rng = config.seed.with_label("oak_skeleton").rng();
    let mut skeleton = Self::default();

    let total_height = rng.random_range(10.5_f32..=12.0_f32);
    let trunk_radius = rng.random_range(0.58_f32..=0.76_f32);

    // Основание ствола.
    skeleton.points.push(SkeletonPoint {
        position: Vec3::ZERO,
        level: 0,
        branch_id: 0,
        radius: trunk_radius,
        taper: 1.0,
        children_count: 1,
    });

    // Ствол доходит внутрь кроны.
    // Ветки будут крепиться к нему на разных высотах.
    let trunk_levels = 7usize;
    let crown_start = total_height * 0.30;
    let trunk_top = total_height * 0.78;

    let mut trunk_points = Vec::new();
    let mut previous = 0usize;

    for i in 0..trunk_levels {
        let t = i as f32 / (trunk_levels - 1) as f32;
        let z = crown_start + (trunk_top - crown_start) * t;

        let sway = if i == 0 {
            Vec3::ZERO
        } else {
            Vec3::new(
                rng.random_range(-0.12_f32..=0.12_f32),
                rng.random_range(-0.12_f32..=0.12_f32),
                0.0,
            )
        };

        let idx = skeleton.points.len();

        skeleton.points.push(SkeletonPoint {
            position: Vec3::new(sway.x, sway.y, z),
            level: 0,
            branch_id: 0,
            radius: trunk_radius * (0.72 - t * 0.42),
            taper: 0.88,
            children_count: 0,
        });

        skeleton.segments.push(SkeletonSegment {
            from: previous,
            to: idx,
        });

        trunk_points.push(idx);
        previous = idx;
    }

    // На каждом уровне ствола создаём несколько скелетных ветвей.
    // За счёт этого листва заполняет не только верх, но и середину/низ кроны.
    for (level_index, &start_idx) in trunk_points.iter().enumerate() {
        let start = skeleton.points[start_idx].position;

        let branch_count = match level_index {
            0 => 5usize,
            1 => 6usize,
            2 => 6usize,
            3 => 6usize,
            4 => 5usize,
            5 => 5usize,
            _ => 4usize,
        };

        let level_offset =
            level_index as f32 * 31.0
                + rng.random_range(-15.0_f32..=15.0_f32);

        let mut children_added = 0usize;

        for branch_index in 0..branch_count {
            if skeleton.points.len() >= config.max_branches {
                break;
            }

            let azimuth =
                level_offset
                    + branch_index as f32
                        * (360.0 / branch_count as f32)
                    + rng.random_range(-12.0_f32..=12.0_f32);

            let h = azimuth.to_radians();

            let horizontal = match level_index {
                // Нижняя часть кроны — самые мощные и широко раскинутые ветви.
                0 => rng.random_range(3.8_f32..=4.8_f32),
                1 => rng.random_range(4.2_f32..=5.2_f32),

                // Максимальная ширина взрослого дуба приходится
                // примерно на нижнюю/среднюю часть кроны.
                2 => rng.random_range(4.5_f32..=5.5_f32),
                3 => rng.random_range(4.2_f32..=5.2_f32),

                // Выше крона постепенно сужается.
                4 => rng.random_range(3.6_f32..=4.6_f32),
                5 => rng.random_range(3.0_f32..=4.0_f32),
                _ => rng.random_range(2.2_f32..=3.2_f32),
            };

            let rise = if level_index <= 1 {
                rng.random_range(0.7_f32..=1.5_f32)
            } else if level_index <= 4 {
                rng.random_range(1.0_f32..=2.0_f32)
            } else {
                rng.random_range(1.3_f32..=2.3_f32)
            };

            // Основная ветвь из двух сегментов.
            // Так она остаётся мощной, но не образует длинную голую "палку".
            let middle_horizontal = horizontal * 0.52;
            let middle_rise = rise * 0.45;

            let middle_position =
                start
                    + Vec3::new(
                        middle_horizontal * h.cos(),
                        middle_horizontal * h.sin(),
                        middle_rise,
                    );

            let middle_idx = skeleton.points.len();

            skeleton.points.push(SkeletonPoint {
                position: middle_position,
                level: 1,
                branch_id: branch_index + 1,
                radius: trunk_radius
                    * rng.random_range(0.20_f32..=0.30_f32),
                taper: 0.80,
                children_count: 1,
            });

            skeleton.segments.push(SkeletonSegment {
                from: start_idx,
                to: middle_idx,
            });

            let end_angle =
                h + rng.random_range(-8.0_f32..=8.0_f32).to_radians();

            let end_position =
                middle_position
                    + Vec3::new(
                        (horizontal - middle_horizontal) * end_angle.cos(),
                        (horizontal - middle_horizontal) * end_angle.sin(),
                        rise - middle_rise,
                    );

            let end_idx = skeleton.points.len();

            skeleton.points.push(SkeletonPoint {
                position: end_position,
                level: 1,
                branch_id: branch_index + 1,
                radius: trunk_radius
                    * rng.random_range(0.12_f32..=0.19_f32),
                taper: 0.76,
                children_count: 0,
            });

            skeleton.segments.push(SkeletonSegment {
                from: middle_idx,
                to: end_idx,
            });

            // Вторичные ветви начинаются и от середины,
            // и от конца основной ветви.
            // Это заполняет пустоты вокруг крупных веток.
            for &(parent_idx, parent_position, secondary_amount) in &[
                (middle_idx, middle_position, 4usize),
                (end_idx, end_position, 6usize),
            ] {
                let mut secondary_added = 0usize;

                for secondary_index in 0..secondary_amount {
                    if skeleton.points.len() >= config.max_branches {
                        break;
                    }

                    let side =
                        secondary_index as f32
                            - (secondary_amount as f32 - 1.0) * 0.5;

                    let secondary_angle =
                        azimuth
                            + side * rng.random_range(18.0_f32..=28.0_f32)
                            + rng.random_range(-12.0_f32..=12.0_f32);

                    let sh = secondary_angle.to_radians();

                    let secondary_horizontal =
                        rng.random_range(0.75_f32..=1.45_f32);

                    // Часть веточек идёт чуть вниз/в стороны,
                    // часть вверх — крона получается объёмной, а не плоской.
                    let secondary_vertical =
                        rng.random_range(-0.25_f32..=0.85_f32);

                    let secondary_end =
                        parent_position
                            + Vec3::new(
                                secondary_horizontal * sh.cos(),
                                secondary_horizontal * sh.sin(),
                                secondary_vertical,
                            );

                    let secondary_idx = skeleton.points.len();

                    skeleton.points.push(SkeletonPoint {
                        position: secondary_end,
                        level: 2,
                        branch_id: secondary_index + 1,
                        radius: rng.random_range(0.045_f32..=0.085_f32),
                        taper: 0.72,
                        children_count: 0,
                    });

                    skeleton.segments.push(SkeletonSegment {
                        from: parent_idx,
                        to: secondary_idx,
                    });

                    // Короткие терминальные побеги создают плотную массу листвы.
                    let twig_count = rng.random_range(4usize..=6usize);
                    let mut twigs_added = 0usize;

                    for twig_index in 0..twig_count {
                        if skeleton.points.len() >= config.max_branches {
                            break;
                        }

                        let twig_angle =
                            secondary_angle
                                + (twig_index as f32
                                    - (twig_count as f32 - 1.0) * 0.5)
                                    * rng.random_range(18.0_f32..=30.0_f32);

                        let th = twig_angle.to_radians();

                        let twig_horizontal =
                            rng.random_range(0.30_f32..=0.65_f32);

                        let twig_vertical =
                            rng.random_range(-0.12_f32..=0.45_f32);

                        let twig_end =
                            secondary_end
                                + Vec3::new(
                                    twig_horizontal * th.cos(),
                                    twig_horizontal * th.sin(),
                                    twig_vertical,
                                );

                        let twig_idx = skeleton.points.len();

                        skeleton.points.push(SkeletonPoint {
                            position: twig_end,
                            level: 3,
                            branch_id: twig_index + 1,
                            radius: rng.random_range(0.018_f32..=0.035_f32),
                            taper: 0.66,
                            children_count: 0,
                        });

                        skeleton.segments.push(SkeletonSegment {
                            from: secondary_idx,
                            to: twig_idx,
                        });

                        twigs_added += 1;
                    }

                    skeleton.points[secondary_idx].children_count =
                        twigs_added;

                    secondary_added += 1;
                }

                skeleton.points[parent_idx].children_count =
                    secondary_added;
            }

            children_added += 1;
        }

        skeleton.points[start_idx].children_count =
            children_added + if level_index + 1 < trunk_levels { 1 } else { 0 };
    }

    // Верхняя центральная масса кроны.
    // Она закрывает визуальную "дыру" над стволом.
    let top_start = *trunk_points.last().unwrap();
    let top_position = skeleton.points[top_start].position;

    let top_count = 8usize;

    for top_index in 0..top_count {
        if skeleton.points.len() >= config.max_branches {
            break;
        }

        let azimuth =
            top_index as f32 * (360.0 / top_count as f32)
                + rng.random_range(-12.0_f32..=12.0_f32);

        let h = azimuth.to_radians();

        let horizontal =
            rng.random_range(0.8_f32..=1.8_f32);

        let vertical =
            rng.random_range(1.8_f32..=3.0_f32);

        let end =
            top_position
                + Vec3::new(
                    horizontal * h.cos(),
                    horizontal * h.sin(),
                    vertical,
                );

        let idx = skeleton.points.len();

        skeleton.points.push(SkeletonPoint {
            position: end,
            level: 2,
            branch_id: top_index + 1,
            radius: rng.random_range(0.06_f32..=0.10_f32),
            taper: 0.70,
            children_count: 0,
        });

        skeleton.segments.push(SkeletonSegment {
            from: top_start,
            to: idx,
        });

        let twig_count = 6usize;
        let mut added = 0usize;

        for twig_index in 0..twig_count {
            if skeleton.points.len() >= config.max_branches {
                break;
            }

            let spread =
                (twig_index as f32 - 2.5)
                    * rng.random_range(20.0_f32..=28.0_f32);

            let th = (azimuth + spread).to_radians();

            let twig_end =
                end
                    + Vec3::new(
                        rng.random_range(0.30_f32..=0.65_f32) * th.cos(),
                        rng.random_range(0.30_f32..=0.65_f32) * th.sin(),
                        rng.random_range(-0.05_f32..=0.45_f32),
                    );

            let twig_idx = skeleton.points.len();

            skeleton.points.push(SkeletonPoint {
                position: twig_end,
                level: 3,
                branch_id: twig_index + 1,
                radius: rng.random_range(0.018_f32..=0.032_f32),
                taper: 0.65,
                children_count: 0,
            });

            skeleton.segments.push(SkeletonSegment {
                from: idx,
                to: twig_idx,
            });

            added += 1;
        }

        skeleton.points[idx].children_count = added;
    }

    Self::finish(&mut skeleton);
    skeleton
}

        /// Специальное построение можжевельника:
        /// низкий раскидистый куст с несколькими основными побегами
        /// и большим количеством коротких боковых веточек.
        fn build_juniper(config: &PlantConfig) -> Self {
            let mut rng = config.seed.with_label("juniper_skeleton").rng();
            let mut skeleton = Self::default();

            // Центральная точка у земли.
            skeleton.points.push(SkeletonPoint {
                position: Vec3::ZERO,
                level: 0,
                branch_id: 0,
                radius: 0.0,
                taper: 1.0,
                children_count: 0,
            });

            // Основные побеги расходятся от центра.
            let main_count = rng.random_range(16usize..=20usize);

            for main_index in 0..main_count {
                let azimuth =
                    main_index as f32 * (360.0 / main_count as f32)
                        + rng.random_range(-8.0..=8.0);

                // Основные побеги длинные и почти горизонтальные.
                let branch_length: f32 = rng.random_range(1.10..=1.35);

                // Конец основного побега находится примерно на высоте 30–60 см.
                let branch_height: f32 = rng.random_range(0.38..=0.75);

                let horizontal_length: f32 =
                    (branch_length * branch_length - branch_height * branch_height).sqrt();

                let h: f32 = azimuth.to_radians();

                // Основные побеги начинаются не строго из одной точки:
                // небольшое смещение основания формирует плотную естественную середину куста.
                let base_radius: f32 = rng.random_range(0.0_f32..=0.18_f32);
                let base_angle: f32 = rng.random_range(0.0_f32..=std::f32::consts::TAU);

                let start_position = Vec3::new(
                    base_radius * base_angle.cos(),
                    base_radius * base_angle.sin(),
                    rng.random_range(0.0_f32..=0.06_f32),
                );

                let end_position: Vec3 = start_position
                    + Vec3::new(
                        horizontal_length * h.cos(),
                        horizontal_length * h.sin(),
                        branch_height,
                    );
                let base_idx = skeleton.points.len();

                skeleton.points.push(SkeletonPoint {
                    position: start_position,
                    level: 0,
                    branch_id: main_index + 1,
                    radius: rng.random_range(0.018..=0.032),
                    taper: 0.88,
                    children_count: 1,
                });

                let main_idx = skeleton.points.len();

                skeleton.points.push(SkeletonPoint {
                    position: end_position,
                    level: 1,
                    branch_id: main_index + 1,
                    radius: rng.random_range(0.015..=0.028),
                    taper: 0.82,
                    children_count: 0,
                });

                skeleton.segments.push(SkeletonSegment {
                    from: base_idx,
                    to: main_idx,
                });

                // По всей длине основного побега располагаются боковые веточки.
                let side_count = rng.random_range(14usize..=18usize);
                let mut children = 0usize;

                for side_index in 1..=side_count {
                    if skeleton.points.len() >= config.max_branches {
                        break;
                    }

                    let along =
                        side_index as f32 / (side_count + 1) as f32;

                    // Место крепления боковой веточки.
                    let attach_position =
                        start_position + (end_position - start_position) * along;

                    let attach_idx = skeleton.points.len();

                    skeleton.points.push(SkeletonPoint {
                        position: attach_position,
                        level: 1,
                        branch_id: main_index + 1,
                        radius: 0.018,
                        taper: 0.78,
                        children_count: 1,
                    });

                    skeleton.segments.push(SkeletonSegment {
                        from: 0,
                        to: attach_idx,
                    });

                    // Из каждой точки растут две боковые веточки:
                    // по обе стороны основного побега.
                    for side_sign in [-1.0_f32, 1.0_f32] {
                        if skeleton.points.len() >= config.max_branches {
                            break;
                        }

                        let side_angle =
                            azimuth
                                + side_sign * rng.random_range(35.0..=70.0)
                                + rng.random_range(-10.0..=10.0);

                        // В середине куста веточки немного длиннее,
                        // ближе к краям становятся короче.
                        let centre_factor =
                            1.0 - ((along - 0.55).abs() * 1.3).min(0.65);

                        let side_length =
                            rng.random_range(0.30..=0.55) * centre_factor;

                        let side_height =
                            rng.random_range(0.04..=0.16);

                        let side_horizontal =
                            (side_length * side_length - side_height * side_height)
                                .max(0.0)
                                .sqrt();

                        let sh = side_angle.to_radians();

                        let side_end = attach_position
                            + Vec3::new(
                                side_horizontal * sh.cos(),
                                side_horizontal * sh.sin(),
                                side_height,
                            );

                        let side_idx = skeleton.points.len();

                        skeleton.points.push(SkeletonPoint {
                            position: side_end,
                            level: 2,
                            branch_id: side_index,
                            radius: rng.random_range(0.008..=0.016),
                            taper: 0.72,
                            children_count: 0,
                        });

                        skeleton.segments.push(SkeletonSegment {
                            from: attach_idx,
                            to: side_idx,
                        });

                        children += 1;
                    }
                }

                skeleton.points[main_idx].children_count = children;
            }
            
            // Внутренний ярус можжевельника.
            // Короткие наклонные побеги перекрывают центр куста,
            // не образуя вертикального пучка.
            let inner_count = rng.random_range(18usize..=24usize);

            for inner_index in 0..inner_count {
                if skeleton.points.len() >= config.max_branches {
                    break;
                }

                // Сдвигаем внутренние побеги относительно внешних,
                // чтобы они попадали в промежутки между основными лапами.
                let azimuth =
                    inner_index as f32 * (360.0 / inner_count as f32)
                        + 360.0 / inner_count as f32 * 0.5
                        + rng.random_range(-18.0_f32..=18.0_f32);

                // Внутренние лапы заметно короче внешних.
                let branch_length =
                    rng.random_range(0.28_f32..=0.50_f32);

                // Они слегка приподняты, но остаются наклонными,
                // а не превращаются в вертикальные стволики.
                let branch_height =
                    rng.random_range(0.30_f32..=0.50_f32);

                let horizontal_length =
                    (branch_length * branch_length
                        - branch_height * branch_height)
                        .max(0.0)
                        .sqrt();
                    
                let h = azimuth.to_radians();

                let end_position = Vec3::new(
                    horizontal_length * h.cos(),
                    horizontal_length * h.sin(),
                    branch_height,
                );

                let inner_idx = skeleton.points.len();

                skeleton.points.push(SkeletonPoint {
                    position: end_position,
                    level: 1,
                    branch_id: main_count + inner_index + 1,
                    radius: rng.random_range(0.018_f32..=0.030_f32),
                    taper: 0.78,
                    children_count: 0,
                });

                skeleton.segments.push(SkeletonSegment {
                    from: 0,
                    to: inner_idx,
                });
            }

            skeleton.points[0].children_count = main_count + inner_count;

            Self::finish(&mut skeleton);
            skeleton
        }

    /// Построить скелет из готовой конфигурации, но с внешним rng (для тестов).
    ///
    /// Вынесено отдельно, чтобы `build` оставался единственной точкой входа
    /// с детерминированным rng.
    fn finish(skeleton: &mut Self) {
        let mut max_dist: f32 = 0.0;
        let mut max_z: f32 = 0.0;
        for p in &skeleton.points {
            max_dist = max_dist.max(p.position.x.hypot(p.position.y));
            max_z = max_z.max(p.position.z);
        }
        skeleton.ground_radius = max_dist;
        skeleton.height = max_z;
    }
}

/// Рекурсивно вырастить ветку: добавить точку-конец и, если есть уровень ниже,
/// породить дочерние ветки.
///
/// Семантика — как в прототипе (`tree_skeleton::build_branch`): уровень 0
/// задаёт абсолютные размеры и углы, глубже — длина/радиус умножаются на
/// родительские, а вертикаль и азимут накапливаются в глобальных сферических
/// координатах (`vertical` — угол от оси Z, `azimuth` — по кругу).
#[allow(clippy::too_many_arguments)]
fn grow(
    rng: &mut StdRng,
    config: &PlantConfig,
    parent: usize,
    branch: &BranchGen,
    parent_length: f32,
    parent_vertical: f32,
    parent_horizontal: f32,
    parent_radius: f32,
    parent_taper: f32,
    skeleton: &mut PlantSkeleton,
) {
    let parent_position = skeleton.points[parent].position;

    let (mut length, vertical, horizontal, mut radius, taper) = if branch.level == 0 {
        (
            rng.random_range(branch.length_min..=branch.length_max),
            rng.random_range(branch.vertical_min..=branch.vertical_max),
            rng.random_range(branch.horizontal_min..=branch.horizontal_max),
            rng.random_range(branch.radius_min..=branch.radius_max),
            rng.random_range(branch.taper_min..=branch.taper_max),
        )
    } else {
        (
            rng.random_range(branch.length_min..=branch.length_max) * parent_length,
            rng.random_range(branch.vertical_min..=branch.vertical_max) + parent_vertical,
            rng.random_range(branch.horizontal_min..=branch.horizontal_max) + parent_horizontal,
            rng.random_range(branch.radius_min..=branch.radius_max) * parent_radius * parent_taper,
            // Тапер: по умолчанию накапливается (новый движок), но старые
            // CSV-деревья берут свой тапер независимо от родителя (как в
            // старой программе) — иначе на 30–40 уровнях накопленный тапер
            // квадратично схлопывает радиус кроны.
            if branch.independent_taper {
                rng.random_range(branch.taper_min..=branch.taper_max)
            } else {
                rng.random_range(branch.taper_min..=branch.taper_max) * parent_taper
            },
        )
    };

    // Клэмп размера сегмента (для старых CSV-деревьев со своими таблицами):
    // ограничивает длину и радиус в метрах ПОСЛЕ умножения на родителя —
    // «иглы» нижних уровней не раздуваются до брёвен, на верхушке не
    // превращаются в крошки; уменьшение размера от уровня к уровню
    // сглаживается. `branch.length_clamp/radius_clamp` заполняется в
    // конвертере для зелёных игл ели.
    if let Some((lo, hi)) = branch.length_clamp {
        length = length.clamp(lo, hi);
    }
    if let Some((lo, hi)) = branch.radius_clamp {
        radius = radius.clamp(lo, hi);
    }

    // Направление — глобальные сферические координаты, как в прототипе.
    // Накопленный азимут «заводит» ветку по кругу, но не складывает её на
    // 180° к стволу: горизонтальные секторы уровней узкие (см. пресеты).
    let v_rad = vertical.to_radians();
    let h_rad = horizontal.to_radians();
    let dir = Vec3::new(v_rad.sin() * h_rad.cos(), v_rad.sin() * h_rad.sin(), v_rad.cos());
    let dir = dir.normalize_or_zero();

    let position = parent_position + dir * length;
    // Растение стоит на земле: ветки, у которых накопленная вертикаль ушла
    // за 90°, могут «нырнуть» под плоскость z = 0 — зажимаем их на землю.
    let position = Vec3::new(position.x, position.y, position.z.max(0.0));

    let idx = skeleton.points.len();
    skeleton.points.push(SkeletonPoint {
        position,
        level: branch.level,
        branch_id: branch.branch_id,
        radius,
        taper,
        children_count: 0,
    });
    skeleton.segments.push(SkeletonSegment {
        from: parent,
        to: idx,
    });

    // Дочерние ветки.
    let mut children = rng.random_range(branch.children_min..=branch.children_max);
    // Не больше, чем позволяет полка производительности.
    let budget = config.max_branches.saturating_sub(skeleton.points.len());
    children = children.min(budget);
    if children == 0 {
        return;
    }
    let Some(next_level) = config.branches.get(branch.level + 1) else {
        return;
    };

    // Окно кандидатов — все `children_max` веток уровня подряд от
    // `first_child_index` (в прототипе окно было на единицу короче, из-за чего
    // «правая» ветка тройки «продолжить/−поворот/+поворот» не использовалась,
    // азимут дрейфовал в одну сторону, и крона закручивалась спиралью). Все
    // шаблоны уровня (включая «+поворот») теперь достигаются симметрично.
    let window_len = branch.children_max.max(1);
    let window_end = branch
        .first_child_index
        .saturating_add(window_len)
        .min(next_level.len());
    let from = branch.first_child_index.min(window_end);
    if from >= window_end {
        return;
    }

    // Способ выбора дочерних веток:
    // - `consecutive_children: true` (старые CSV-деревья) — дети строго подряд
    //   от `first_child_index`, первые `children` веток уровня, как в старой
    //   программе (`GenerateChildren`: цикл `startChildIndex + i`). Первая
    //   ветка окна — «продолжение» родителя (ствол/ветка), поэтому случайный
    //   выбор из окна терял бы её и дерево обрывалось;
    // - иначе (пресеты нового движка) — окно из `children_max` веток
    //   перемешивается и берутся случайные `children`, так у тройки шаблонов
    //   «продолжить/−поворот/+поворот» достижимы все три и азимут не дрейфует.
    let indices: Vec<usize> = if branch.consecutive_children {
        let count = children.min(next_level.len().saturating_sub(from));
        (from..from + count).collect()
    } else {
        let mut candidates: Vec<usize> = (from..window_end).collect();
        // Детерминированное перемешивание окна (частичный Фишер–Йетс).
        for i in (1..candidates.len()).rev() {
            let j = rng.random_range(0..=i);
            candidates.swap(i, j);
        }
        candidates.truncate(children);
        candidates
    };

    let mut added = 0usize;
    for ci in indices {
        if skeleton.points.len() >= config.max_branches {
            break;
        }
        let child = &next_level[ci];
        grow(
            rng,
            config,
            idx,
            child,
            length,
            vertical,
            horizontal,
            radius,
            taper,
            skeleton,
        );
        added += 1;
    }
    // Фактическое число добавленных детей (для подсчёта терминальных).
    skeleton.points[idx].children_count = added;
}

/// Сегмент является терминальным (заканчивает ветку — на него вешается крона).
pub fn is_terminal(skeleton: &PlantSkeleton, segment: &SkeletonSegment) -> bool {
    skeleton.points[segment.to].children_count == 0
}
