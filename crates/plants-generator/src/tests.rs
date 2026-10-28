//! Unit-тесты генерации растений.
//!
//! Проверяем детерминизм от seed, достоверность генерации для всех пресетов,
//! ограничение числа веток и непересечение объектов в множественном режиме.


use bevy::math::Vec3;
use generation_core::Seed;

use crate::generation::{
    GenerationMode, PlantConfig, PlantPreset, PlantScene, PlantType, resolve,
};

/// Сцена для пресета и seed.
fn scene(preset: PlantPreset, seed: u64, mode: GenerationMode) -> PlantScene {
    let config: PlantConfig = resolve(preset, Seed::new(seed));
    PlantScene::generate(&config, mode)
}

/// Глубокие уровни ветвятся нормально: боковые ветви ранних уровней есть, а не
/// вырождаются в голый ствол, и иерархия доходит до самого низа. Регресс:
/// при добавлении уровней (например, у дерева до 15) боковые ветви ранних
/// уровней пропадали — дерево переставало влезать в лимит веток, и первое
/// поддерево съедало весь бюджет.
#[test]
fn deep_levels_branch_normally() {
    use crate::generation::PlantSkeleton;

    for preset in [PlantPreset::DeciduousTree, PlantPreset::Bush] {
        for seed in 0..6u64 {
            let config: PlantConfig = resolve(preset, Seed::new(seed));
            let skeleton = PlantSkeleton::build(&config);
            let total = skeleton.segments.len();
            assert!(total > 0, "{preset:?} seed={seed}: пустой скелет");

            // На каждом раннем уровне есть реальные сегменты.
            for level in 1..=3 {
                let count = skeleton
                    .segments
                    .iter()
                    .filter(|seg| skeleton.points[seg.to].level == level)
                    .count();
                assert!(
                    count > 0,
                    "{preset:?} seed={seed}: уровень {level} без сегментов (всего {total})"
                );
            }

            // Иерархия доходит до глубоких уровней (у дерева — до 12-го).
            let max_level = skeleton.points.iter().map(|p| p.level).max().unwrap_or(0);
            let min_deep = if preset == PlantPreset::DeciduousTree { 10 } else { 6 };
            assert!(
                max_level >= min_deep,
                "{preset:?} seed={seed}: ветки не доходят до глубоких уровней (max={max_level})"
            );
        }
    }
}

/// Крона раскидывается по горизонтали, а не кучкуется у оси ствола (регресс бага
/// «ветви кучкуются в центре при виде сверху»).
#[test]
fn canopy_spreads_horizontally() {
    for seed in 0..5u64 {
        let s = scene(PlantPreset::DeciduousTree, seed, GenerationMode::Single);
        let plant = &s.plants[0].plant;

        let mut r_max = 0.0f32;
        let mut total = 0usize;
        let mut bunched = 0usize;
        for p in plant.point_cloud.positions() {
            let r = p.x.hypot(p.y);
            r_max = r_max.max(r);
            total += 1;
        }
        for p in plant.point_cloud.positions() {
            let r = p.x.hypot(p.y);
            if r_max > 1e-3 && r < r_max * 0.2 {
                bunched += 1;
            }
        }

        assert!(r_max > 0.5, "seed={seed}: крона почти без размаха (r_max={r_max})");
        let frac = bunched as f32 / total.max(1) as f32;
        assert!(
            frac < 0.4,
            "seed={seed}: {:.0}% точек кроны кучкуются у оси (r_max={r_max})",
            frac * 100.0
        );
    }
}

/// Все позиции облака точек сцены (для сравнений).
fn all_cloud_positions(scene: &PlantScene) -> Vec<Vec3> {
    let mut v = Vec::new();
    for p in &scene.plants {
        v.extend(p.plant.point_cloud.positions());
    }
    for t in &scene.tufts {
        v.extend(t.tuft.point_cloud.positions());
    }
    v
}

#[test]
fn deterministic_by_seed() {
    for preset in PlantPreset::ALL {
        for mode in [GenerationMode::Single, GenerationMode::Multiple] {
            let a = scene(preset, 42, mode);
            let b = scene(preset, 42, mode);
            assert_eq!(a.plants.len(), b.plants.len(), "{preset:?} {mode:?}");
            assert_eq!(a.tufts.len(), b.tufts.len(), "{preset:?} {mode:?}");
            assert_eq!(a.cloud_points, b.cloud_points, "{preset:?} {mode:?}");
            assert_eq!(
                all_cloud_positions(&a),
                all_cloud_positions(&b),
                "{preset:?} {mode:?}: разные облака при одном seed"
            );
        }
    }
}

#[test]
fn different_seeds_differ() {
    let a = scene(PlantPreset::DeciduousTree, 1, GenerationMode::Single);
    let b = scene(PlantPreset::DeciduousTree, 2, GenerationMode::Single);
    assert_ne!(all_cloud_positions(&a), all_cloud_positions(&b));
}

#[test]
fn every_preset_yields_something() {
    for preset in PlantPreset::ALL {
        for mode in [GenerationMode::Single, GenerationMode::Multiple] {
            let s = scene(preset, 7, mode);
            assert!(
                s.object_count() > 0,
                "{preset:?} {mode:?}: пустая сцена"
            );
            assert!(s.cloud_points > 0, "{preset:?} {mode:?}: нет точек");
            assert!(s.max_height > 0.0, "{preset:?} {mode:?}: нулевая высота");
        }
    }
}

#[test]
fn woody_plant_is_valid() {
    for preset in [PlantPreset::DeciduousTree, PlantPreset::Bush] {
        for seed in 0..6u64 {
            let s = scene(preset, seed, GenerationMode::Single);
            assert_eq!(s.plants.len(), 1, "{preset:?} seed={seed}");
            let plant = &s.plants[0].plant;
            assert!(plant.branch_count > 0, "{preset:?} seed={seed}: нет веток");
            assert!(plant.leaf_count > 0, "{preset:?} seed={seed}: нет кроны");
            assert!(plant.height > 0.2, "{preset:?} seed={seed}: слишком низкое");
            assert!(plant.extent > 0.0);
            // Растения не должны уходить под землю (облако минимально выше нуля,
            // с учётом небольшого шума).
            let min_z = plant
                .point_cloud
                .positions()
                .map(|p| p.z)
                .fold(f32::INFINITY, f32::min);
            assert!(
                min_z >= -0.15,
                "{preset:?} seed={seed}: нижняя точка {min_z} под землёй"
            );
        }
    }
}

#[test]
fn branches_respect_limit() {
    let config: PlantConfig = resolve(PlantPreset::DeciduousTree, Seed::new(5));
    let skeleton = crate::generation::PlantSkeleton::build(&config);
    assert!(
        skeleton.segments.len() <= config.max_branches,
        "веток {} больше лимита {}",
        skeleton.segments.len(),
        config.max_branches
    );
}

/// Хвойные перенесены из старых CSV: высокий ствол + крона из зелёных
/// конусов-«игл» таблицы (коричневые ветки + зелёные сегменты; отдельной
/// листвы нет — `leaf_count` = 0, крону дают зелёные сегменты).
#[test]
fn conifers_have_high_trunk_and_green_crown() {
    for preset in [PlantPreset::Fir, PlantPreset::Pine] {
        for seed in 0..4u64 {
            let config: PlantConfig = resolve(preset, Seed::new(seed));
            let skeleton = crate::generation::PlantSkeleton::build(&config);
            // Есть и коричневый (ствол/ветки), и зелёный (крона) сегменты.
            let mut brown = 0usize;
            let mut green = 0usize;
            for seg in &skeleton.segments {
                let end = skeleton.points[seg.to];
                let color = config
                    .branches
                    .get(end.level)
                    .and_then(|lv| lv.get(end.branch_id))
                    .and_then(|b| b.color)
                    .unwrap_or(config.branch_color);
                let c = color.to_srgba();
                if c.red > c.green && c.red > c.blue {
                    brown += 1;
                } else {
                    green += 1;
                }
            }
            assert!(brown > 0, "{preset:?} seed={seed}: нет коричневых веток");
            assert!(green > 0, "{preset:?} seed={seed}: нет зелёной кроны");

            let s = scene(preset, seed, GenerationMode::Single);
            let plant = &s.plants[0].plant;
            let min_height = if preset == PlantPreset::Fir { 4.0 } else { 12.0 };
            assert!(
                plant.height > min_height,
                "{preset:?} seed={seed}: рост {:.1} — хвойное должно быть высоким",
                plant.height
            );
            assert!(plant.extent > 1.0, "{preset:?} seed={seed}: крона почти без размаха");
            assert!(
                plant.extent < plant.height,
                "{preset:?} seed={seed}: крона шире, чем дерево высоким"
            );
            // Крона читается и в облаке точек.
            assert!(
                s.cloud_points > 1000,
                "{preset:?} seed={seed}: слишком мало точек ({})",
                s.cloud_points
            );
        }
    }
}

/// Ива — широкая плакучая крона с густой листвой на свисающих прядях.
#[test]
fn willow_is_wide_and_leafy() {
    for seed in 0..4u64 {
        let s = scene(PlantPreset::Willow, seed, GenerationMode::Single);
        let plant = &s.plants[0].plant;
        assert!(
            plant.extent > 6.0,
            "seed={seed}: ива не раскидистая ({:.1})",
            plant.extent
        );
        assert!(plant.height < 30.0, "seed={seed}: слишком высокая ива");
        assert!(
            plant.leaf_count > 200,
            "seed={seed}: мало листвы ({})",
            plant.leaf_count
        );
    }
}

/// Пальмы — голый ствол с кроной-«фонтаном» на верхушке: веток немного
/// (ствол + линейные цепочки вай, все уровни таблицы до конца); крону образуют
/// сами зелёные вайи — отдельной листвы (leaf_count) нет.
#[test]
fn palms_have_bare_trunk_and_top_crown() {
    for preset in [PlantPreset::Palm, PlantPreset::PalmCoconuts] {
        for seed in 0..4u64 {
            let config: PlantConfig = resolve(preset, Seed::new(seed));
            let skeleton = crate::generation::PlantSkeleton::build(&config);
            assert!(
                skeleton.segments.len() < 1200,
                "{preset:?} seed={seed}: слишком много веток для пальмы ({})",
                skeleton.segments.len()
            );
            // Крону дают зелёные вайи (коричневый — только ствол и кокосы).
            let mut green = 0usize;
            for seg in &skeleton.segments {
                let end = skeleton.points[seg.to];
                let color = config
                    .branches
                    .get(end.level)
                    .and_then(|lv| lv.get(end.branch_id))
                    .and_then(|b| b.color)
                    .unwrap_or(config.branch_color);
                let c = color.to_srgba();
                if c.green > c.red && c.green > c.blue {
                    green += 1;
                }
            }
            assert!(
                green > 0,
                "{preset:?} seed={seed}: нет зелёной кроны-фонтана"
            );
            let s = scene(preset, seed, GenerationMode::Single);
            let plant = &s.plants[0].plant;
            assert!(plant.height > 8.0, "{preset:?} seed={seed}: низкая");
            assert!(plant.extent > 2.0, "{preset:?} seed={seed}: нет кроны");
            // Отдельная листва отключена — крона только из зелёных сегментов.
            assert_eq!(
                plant.leaf_count, 0,
                "{preset:?} seed={seed}: листва на кончиках не нужна"
            );
        }
    }
}

#[test]
fn grass_tuft_has_blades_and_cloud() {
    for preset in [PlantPreset::TallGrass, PlantPreset::SparseGrass] {
        for seed in 0..4u64 {
            let config: PlantConfig = resolve(preset, Seed::new(seed));
            let tuft = crate::generation::generate_tuft(&config, Seed::new(seed).with_label("t"));
            assert!(tuft.blade_count > 0);
            assert!(!tuft.part.faces.is_empty());
            assert!(!tuft.point_cloud.is_empty());
            assert!(tuft.height > 0.0);
            // Точки травы не ниже земли.
            let min_z = tuft
                .point_cloud
                .positions()
                .map(|p| p.z)
                .fold(f32::INFINITY, f32::min);
            assert!(min_z >= -1e-3, "трава под землей: {min_z}");
        }
    }
}

#[test]
fn single_is_center_only_multiple_spreads() {
    let single = scene(PlantPreset::DeciduousTree, 11, GenerationMode::Single);
    assert_eq!(single.plants.len(), 1);
    assert_eq!(single.plants[0].position, bevy::math::Vec2::ZERO);

    let multiple = scene(PlantPreset::DeciduousTree, 11, GenerationMode::Multiple);
    assert!(
        multiple.plants.len() > 1,
        "множественный режим должен давать соседей"
    );

    // Трава: одиночный — один пучок, множественный — поле.
    let g_single = scene(PlantPreset::TallGrass, 11, GenerationMode::Single);
    assert_eq!(g_single.tufts.len(), 1);
    let g_multi = scene(PlantPreset::TallGrass, 11, GenerationMode::Multiple);
    assert!(g_multi.tufts.len() >= 2, "у поля должно быть много пучков");
}

#[test]
fn multiple_mode_no_overlap() {
    for preset in [PlantPreset::DeciduousTree, PlantPreset::Bush] {
        for seed in 0..8u64 {
            let s = scene(preset, seed, GenerationMode::Multiple);
            let positions: Vec<bevy::math::Vec2> =
                s.plants.iter().map(|p| p.position).collect();
            for i in 0..s.plants.len() {
                for j in (i + 1)..s.plants.len() {
                    let need = (s.plants[i].plant.extent + s.plants[j].plant.extent) * 0.9;
                    let dist = positions[i].distance(positions[j]);
                    assert!(
                        dist >= need - 1e-3,
                        "{preset:?} seed={seed}: объекты {i} и {j} пересекаются ({dist}<{need})"
                    );
                }
            }
        }
    }
}

#[test]
fn grass_field_stays_in_radius() {
    let config: PlantConfig = resolve(PlantPreset::TallGrass, Seed::new(3));
    let field = crate::generation::generate_field(&config, config.seed);
    for t in &field {
        let r = t.position.length();
        assert!(
            r <= config.grass.field_radius + 1e-3,
            "пучок на {r} дальше радиуса поля"
        );
    }
}

#[test]
fn grass_type_is_grass_preset() {
    assert_eq!(PlantPreset::TallGrass.plant_type(), PlantType::Grass);
    assert_eq!(PlantPreset::DeciduousTree.plant_type(), PlantType::Tree);
    assert_eq!(PlantPreset::Bush.plant_type(), PlantType::Bush);
}

/// Против «спирали» кроны (видно сверху): у ветвящихся уровней тройка
/// шаблонов горизонта симметрична (продолжить/−поворот/+поворот: центры в
/// сумме ~0), а окно кандидатов покрывает весь следующий уровень. В прототипе
/// окно было на единицу короче, «+поворот» не использовался, и азимут
/// систематически дрейфовал в одну сторону — крона закручивалась спиралью.
#[test]
fn branch_templates_are_symmetric_no_spiral() {
    for preset in [PlantPreset::DeciduousTree, PlantPreset::Bush] {
        let config: PlantConfig = resolve(preset, Seed::new(3));
        let levels = &config.branches;
        for l in 0..levels.len() {
            let level = &levels[l];
            // Полное окно: все шаблоны следующего уровня достижимы (нет
            // «мёртвой» ветки, из-за которой азимут дрейфует в одну сторону).
            if l + 1 < levels.len() {
                let window = level.iter().map(|b| b.children_max).max().unwrap_or(0);
                assert_eq!(
                    window,
                    levels[l + 1].len(),
                    "{preset:?} уровень {l}: окно кандидатов ({window}) не покрывает уровень {} — \
                     «+поворот» снова мёртв, крона закрутится",
                    l + 1
                );
            }
            // Тройки относительных шаблонов симметричны: сумма центров ~0.
            if level.len() == 3 {
                let sum_center: f32 = level
                    .iter()
                    .map(|b| b.horizontal_min + b.horizontal_max)
                    .sum();
                assert!(
                    sum_center.abs() < 1.0,
                    "{preset:?} уровень {l}: несимметричные горизонты (сумма центров \
                     {sum_center}) — азимут дрейфует в одну сторону"
                );
            }
        }
    }
}
