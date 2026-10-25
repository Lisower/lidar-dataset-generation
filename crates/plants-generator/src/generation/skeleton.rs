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

            // Количество ярусов ели.
            let tiers = 18usize;

            let mut previous_trunk_point = 0usize;

            // Строим вертикальный ствол по частям.
            for tier in 1..=tiers {
                let t = tier as f32 / tiers as f32;
                let z = height * t;

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
                let base_branch_length = 0.7 + crown_factor * 4.5;

                // Число основных ветвей немного различается между ярусами.
                let branch_count = rng.random_range(6usize..=9usize);

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
                    // Небольшой разброс делает форму естественнее.
                    let vertical_deg =
                        82.0 - t * 25.0 + rng.random_range(-6.0..=6.0);

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

                    // Боковые веточки второго порядка.
                    // Располагаем их вдоль каждой основной еловой лапы.
                    let side_branches = rng.random_range(6usize..=10usize);

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

    let (length, vertical, horizontal, radius, taper) = if branch.level == 0 {
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
            rng.random_range(branch.taper_min..=branch.taper_max) * parent_taper,
        )
    };

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
    let mut candidates: Vec<usize> = (from..window_end).collect();

    // Детерминированное перемешивание окна (частичный Фишер–Йетс).
    for i in (1..candidates.len()).rev() {
        let j = rng.random_range(0..=i);
        candidates.swap(i, j);
    }

    let mut added = 0usize;
    for &ci in candidates.iter().take(children) {
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
