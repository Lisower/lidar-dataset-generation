//! Пресеты растений: архетипы и детерминированный разброс от seed.
//!
//! Идея — как в `landscapes-generator`: генерация умеет строить разные
//! растения, но в рамках одного архетипа ([`PlantPreset`]) параметры берутся
//! из эталона и ограниченно «дрожат» от seed. `resolve(preset, seed)` — чистая
//! функция: один и тот же пресет и seed всегда дают одну и ту же конфигурацию.

use bevy::color::Color;
use rand::RngExt;
use rand::rngs::StdRng;
use std::path::PathBuf;

use generation_core::{
    PRESETS_DIR, Seed, load_preset_config, resolve_cross_category,
};

use super::config::{
    BranchGen, BranchLevels, FoliageConfig, FoliagePlacement, FoliageStyle, GrassConfig,
    PlantConfig, PlantPreset, PlantType,
};

/// Зелёные оттенки листвы.
const FOLIAGE_GREEN_LIGHT: Color = Color::srgb(0.32, 0.62, 0.22);
const FOLIAGE_GREEN_MID: Color = Color::srgb(0.24, 0.5, 0.18);
const FOLIAGE_GREEN_DARK: Color = Color::srgb(0.18, 0.38, 0.16);

/// Цвета стволов/веток.
const BARK_TREE: Color = Color::srgb(0.36, 0.22, 0.10);
const BARK_BUSH: Color = Color::srgb(0.28, 0.2, 0.08);

/// Детерминированный поток случайности для пресетных решений.
fn preset_rng(preset: PlantPreset, seed: Seed) -> StdRng {
    seed.with_label(preset.label()).rng()
}

/// Корень каталога пресетов (workspace/presets).
fn presets_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(PRESETS_DIR)
}

/// Конфигурация пресета из готового RON-файла каталога `presets/plants/...`
/// (папки `high/coniferous/fir` и т.д.) — так же, как её читает и приложение
/// через [`PresetStore`](generation_core::PresetStore). Пресет ищется по id
/// манифеста ([`PlantPreset::ron_id`]), поэтому папку можно свободно
/// переименовывать и перекладывать по группам.
///
/// Используется для деревьев, перенесённых из старых CSV-таблиц Unity-программы
/// (ель высокая, сосна, ива, пальмы): таблица ветвления перенесена в RON 1:1,
/// отдельного кода конвертации в крейте больше нет.
fn from_ron(preset: PlantPreset) -> PlantConfig {
    let root = presets_root();
    let meta = resolve_cross_category("plants", preset.ron_id(), &root).unwrap_or_else(|| {
        panic!(
            "пресет «{}» не найден в {} — сгенерируйте его примером `dump_presets`",
            preset.label(),
            root.join("plants").display()
        )
    });
    load_preset_config(&meta).unwrap_or_else(|| {
        panic!(
            "не читается config.ron пресета «{}»: {}",
            preset.label(),
            meta.config_path.display()
        )
    })
}

/// Полностью разрешённая конфигурация пресета `preset` от `seed`.
pub fn resolve(preset: PlantPreset, seed: Seed) -> PlantConfig {
    // Деревья, перенесённые из старых CSV-таблиц старой Unity-программы
    // (ель высокая, сосна, ива, пальмы), заданы готовыми пресетами RON в
    // `presets/plants/...` — конфигурация читается из `config.ron` (1:1 из
    // таблицы ветвления), как это делает и приложение через `PresetStore`.
    // Дополнительное дрожание к ним не применяется: разброс даёт сам seed.
    if matches!(
        preset,
        PlantPreset::Fir
            | PlantPreset::Pine
            | PlantPreset::Willow
            | PlantPreset::Palm
            | PlantPreset::PalmCoconuts
    ) {
        let mut cfg = from_ron(preset);
        cfg.seed = seed;
        return cfg;
    }

    let mut cfg = match preset {
        PlantPreset::DeciduousTree => deciduous_tree(),
        PlantPreset::Spruce => spruce(),
        // Обработаны выше (загрузка из RON-пресетов) — сюда не попадают.
        PlantPreset::Fir
        | PlantPreset::Pine
        | PlantPreset::Willow
        | PlantPreset::Palm
        | PlantPreset::PalmCoconuts => unreachable!("пресет RON-каталога"),
        PlantPreset::Bush => bush(),
        PlantPreset::TallGrass => tall_grass(),
        PlantPreset::SparseGrass => sparse_grass(),
    };
    cfg.seed = seed;

    let mut rng = preset_rng(preset, seed);
    jitter(&mut cfg, preset, &mut rng);

    cfg
}

/// Радиальная ветка уровня: горизонт занимает сектор вокруг `sector_deg`
/// с размахом `span_deg`; вертикаль — около `vertical_deg` с размахом `tilt_deg`.
#[allow(clippy::too_many_arguments)]
fn sector_branch(
    level: usize,
    id: usize,
    sector_deg: f32,
    span_deg: f32,
    vertical_deg: f32,
    tilt_deg: f32,
    length: (f32, f32),
    radius: (f32, f32),
    taper: (f32, f32),
    children: (usize, usize),
    first_child: usize,
    sides: usize,
) -> BranchGen {
    BranchGen {
        level,
        branch_id: id,
        length_min: length.0,
        length_max: length.1,
        radius_min: radius.0,
        radius_max: radius.1,
        taper_min: taper.0,
        taper_max: taper.1,
        vertical_min: vertical_deg - tilt_deg,
        vertical_max: vertical_deg + tilt_deg,
        horizontal_min: sector_deg - span_deg,
        horizontal_max: sector_deg + span_deg,
        children_min: children.0,
        children_max: children.1,
        first_child_index: first_child,
        mesh_sides: sides,
        consecutive_children: false,
        independent_taper: false,
        length_clamp: None,
        radius_clamp: None,
        color: None,
    }
}

/// N веток уровня, равномерно распределённых по кругу.
#[allow(clippy::too_many_arguments)]
fn radial_level(
    level: usize,
    count: usize,
    span_deg: f32,
    vertical_deg: f32,
    tilt_deg: f32,
    length: (f32, f32),
    radius: (f32, f32),
    taper: (f32, f32),
    children: (usize, usize),
    first_child: usize,
    sides: usize,
) -> Vec<BranchGen> {
    (0..count)
        .map(|j| {
            let sector = 360.0 * j as f32 / count as f32;
            sector_branch(
                level,
                j,
                sector,
                span_deg,
                vertical_deg,
                tilt_deg,
                length,
                radius,
                taper,
                children,
                first_child,
                sides,
            )
        })
        .collect()
}

/// Тройка веток уровня — шаблон средних уровней прототипа: одна ветка
/// продолжает азимут родителя (горизонт 0 ± `span`), две другие сворачивают
/// на ±`turn`. Горизонт каждой ветки — относительный, вокруг азимута родителя
/// (в накопленной модели), а не абсолютный сектор по кругу: абсолютные
/// сектора ±120° как раз и заворачивали ветки к стволу.
#[allow(clippy::too_many_arguments)]
fn branching_level(
    level: usize,
    length: (f32, f32),
    radius: (f32, f32),
    taper: (f32, f32),
    vertical: (f32, f32),
    span: f32,
    turn: f32,
    children: (usize, usize),
    sides: usize,
) -> Vec<BranchGen> {
    let horiz = [
        (-span, span),                 // продолжает азимут родителя
        (-(turn + span), -(turn - span)), // поворот на −`turn`
        (turn - span, turn + span),    // поворот на +`turn`
    ];
    (0..3)
        .map(|j| BranchGen {
            level,
            branch_id: j,
            length_min: length.0,
            length_max: length.1,
            radius_min: radius.0,
            radius_max: radius.1,
            taper_min: taper.0,
            taper_max: taper.1,
            vertical_min: vertical.0,
            vertical_max: vertical.1,
            horizontal_min: horiz[j].0,
            horizontal_max: horiz[j].1,
            children_min: children.0,
            children_max: children.1,
            first_child_index: 0,
            mesh_sides: sides,
            consecutive_children: false,
            independent_taper: false,
            length_clamp: None,
            radius_clamp: None,
            color: None,
        })
        .collect()
}

/// Лиственное дерево: 12 уровней, ствол → 4 основные ветви → тройки шаблонов.
///
/// Таблицы уровней — из прототипа (`tree_config`): длины/радиусы/тапера/углы/
/// число детей/число граней. Вместе с накоплением углов в скелете это даёт
/// ажурную крону. Окно кандидатов (см. `grow`) делает достижимыми все три
/// ветки тройки «продолжить/−поворот/+поворот» — азимут не дрейфует, крона
/// не закручивается спиралью. Дерево на 12 уровнях (0..11), а не на 15:
/// при ветвлении 2..3 на узел 15 уровней дали бы ~600 тыс. веток (у 15-го
/// уровня прототипа ветвление было строго 2 из-за «короткого» окна).
fn deciduous_tree() -> PlantConfig {
    let mut branches: BranchLevels = Vec::new();

    // Уровень 0 — ствол: абсолютные размеры, почти вертикальный (горизонт
    // произвольный 0..360).
    branches.push(vec![sector_branch(
        0, 0, 180.0, 180.0, 0.0, 5.0, (1.5, 2.5), (0.6, 0.8), (0.8, 0.85), (2, 4), 0, 12,
    )]);

    // Уровень 1 — 4 основные ветви по секторам 120° (как в прототипе, секторы
    // не равномерны, а 0/120/240/360).
    let mut level1 = Vec::new();
    for j in 0..4 {
        level1.push(sector_branch(
            1, j, 120.0 * j as f32, 40.0, 30.0, 5.0, (0.8, 0.85), (0.9, 0.95), (0.85, 0.9),
            (2, 3), 0, 16,
        ));
    }
    branches.push(level1);

    // Уровни 2..4 — скелет кроны: ветки продолжают азимут родителя либо
    // сворачивают на ±40°.
    for level in 2..5 {
        branches.push(branching_level(
            level, (0.8, 0.85), (0.9, 0.95), (0.9, 0.95), (-10.0, 30.0), 15.0, 40.0, (2, 3), 8,
        ));
    }

    // Средние ветви кроны — сохраняем густоту.
    for level in 5..7 {
        branches.push(branching_level(
            level, (0.8, 0.85), (0.9, 0.95), (0.95, 0.97),
            (-5.0, 20.0), 12.5, 32.5, (2, 3), 4,
        ));
    }

    // Мелкие верхние ветви — уменьшаем лишнее ветвление.
    for level in 7..10 {
        branches.push(branching_level(
            level, (0.8, 0.85), (0.9, 0.95), (0.95, 0.97),
            (-10.0, 10.0), 12.5, 32.5, (2, 3), 4,
        ));
    }

    // Уровни 9..10 — самые тонкие кончики, почти без наклона вверх: накоп-
    // ленная вертикаль уже большая, поэтому крона «капает» вниз по краю.
    for level in 10..11 {
        branches.push(branching_level(
            level, (0.8, 0.85), (0.9, 0.95), (0.95, 0.97), (-5.0, 5.0), 10.0, 25.0, (2, 3), 4,
        ));
    }

    PlantConfig {
        seed: Seed::default(),
        preset: PlantPreset::DeciduousTree,
        plant_type: PlantType::Tree,
        branches,
        branch_color: BARK_TREE,
        max_branches: 300_000,
        foliage: FoliageConfig {
            style: FoliageStyle::Leafy,
            placement: FoliagePlacement::Terminal { min_level: 2 },
            color: FOLIAGE_GREEN_LIGHT,
            length: 0.2,
            width: 0.15,
            per_anchor: 3,
            anchors: 1,
            pitch_jitter: 60.0,
            cloud_points: 1,
        },
        grass: grass_default(),
        point_step: 0.2,
        jitter: 0.02,
        spread_radius: 30.0,
        neighbors_min: 3,
        neighbors_max: 7,
    }
}

/// Ель: высокий прямой ствол и коническая крона.
///
/// Нижние ветви длинные и почти горизонтальные.
/// По мере приближения к вершине ветви становятся короче
/// и сильнее направляются вверх.
fn spruce() -> PlantConfig {
    let mut branches: BranchLevels = Vec::new();

    // Уровень 0 — высокий прямой ствол.
    branches.push(vec![sector_branch(
        0,
        0,
        180.0,
        180.0,
        0.0,
        2.0,
        (4.0, 5.0),
        (0.35, 0.5),
        (0.82, 0.88),
        (6, 8),
        0,
        12,
    )]);

    // Уровень 1 — нижний ярус кроны.
    // Самые длинные ветви, расположенные почти горизонтально.
    branches.push(radial_level(
        1,
        8,
        8.0,
        70.0,
        8.0,
        (2.5, 3.2),
        (0.18, 0.24),
        (0.75, 0.85),
        (2, 3),
        0,
        8,
    ));

    // Уровень 2 — средняя часть кроны.
    branches.push(branching_level(
        2,
        (0.65, 0.8),
        (0.65, 0.8),
        (0.75, 0.85),
        (5.0, 20.0),
        12.0,
        35.0,
        (2, 3),
        6,
    ));

    // Уровень 3 — ветви становятся короче.
    branches.push(branching_level(
        3,
        (0.55, 0.7),
        (0.6, 0.75),
        (0.75, 0.85),
        (5.0, 18.0),
        10.0,
        30.0,
        (2, 3),
        5,
    ));

    // Уровень 4 — мелкие веточки.
    branches.push(branching_level(
        4,
        (0.45, 0.6),
        (0.55, 0.7),
        (0.7, 0.82),
        (0.0, 15.0),
        10.0,
        25.0,
        (2, 3),
        4,
    ));

    // Уровень 5 — кончики хвойных ветвей.
    branches.push(branching_level(
        5,
        (0.35, 0.5),
        (0.5, 0.65),
        (0.7, 0.8),
        (-5.0, 12.0),
        8.0,
        20.0,
        (1, 2),
        4,
    ));

    PlantConfig {
        seed: Seed::default(),
        preset: PlantPreset::Spruce,
        plant_type: PlantType::Tree,
        branches,
        branch_color: BARK_TREE,
        max_branches: 80_000,

        foliage: FoliageConfig {
            style: FoliageStyle::Leafy,
            placement: FoliagePlacement::Terminal { min_level: 2 },
            color: FOLIAGE_GREEN_DARK,
            length: 0.12,
            width: 0.025,
            per_anchor: 5,
            anchors: 2,
            pitch_jitter: 35.0,
            cloud_points: 1,
        },

        grass: grass_default(),

        point_step: 0.15,
        jitter: 0.015,

        spread_radius: 25.0,
        neighbors_min: 3,
        neighbors_max: 6,
    }
}

/// Куст: много стволиков от земли, листва почти от самой земли.
///
/// Из 8 уровней прототипа: 11 стволиков от земли, дальше по 3 ветки на
/// уровень. Внутренние уровни короткие (1..3), внешние (4..7) длиннее — куст
/// растёт вширь.
fn bush() -> PlantConfig {
    let mut branches: BranchLevels = Vec::new();

    // Уровень 0 — основные побеги от земли.
    branches.push(radial_level(
        0, 8, 10.0, 22.5, 17.5,
        (0.3, 0.4),
        (0.03, 0.04),
        (0.97, 0.99),
        (2, 3),
        0,
        5,
    ));

    // Уровни 1..3 — внутренние ветки куста.
    for level in 1..4 {
        branches.push(branching_level(
            level,
            (0.6, 0.7),
            (0.97, 0.99),
            (0.8, 0.9),
            (10.0, 15.0),
            15.0,
            30.0,
            (2, 3),
            4,
        ));
    }

    // Уровни 4..6 — внешние ветки.
    // На последних уровнях уменьшаем количество дочерних ветвей.
    for level in 4..7 {
        branches.push(branching_level(
            level,
            (0.9, 0.95),
            (0.97, 0.99),
            (0.7, 0.75),
            (5.0, 10.0),
            15.0,
            30.0,
            (2, 3),
            4,
        ));
    }

    PlantConfig {
        seed: Seed::default(),
        preset: PlantPreset::Bush,
        plant_type: PlantType::Bush,
        branches,
        branch_color: BARK_BUSH,
        max_branches: 30_000,
        foliage: FoliageConfig {
            style: FoliageStyle::Leafy,
            placement: FoliagePlacement::Terminal { min_level: 1 },
            color: FOLIAGE_GREEN_MID,
            length: 0.2,
            width: 0.15,
            per_anchor: 1,
            anchors: 1,
            pitch_jitter: 60.0,
            cloud_points: 1,
        },
        grass: grass_default(),
        point_step: 0.2,
        jitter: 0.02,
        spread_radius: 10.0,
        neighbors_min: 4,
        neighbors_max: 10,
    }
}

/// Высокая густая трава.
fn tall_grass() -> PlantConfig {
    PlantConfig {
        seed: Seed::default(),
        preset: PlantPreset::TallGrass,
        plant_type: PlantType::Grass,
        branches: Vec::new(),
        branch_color: Color::srgb(0.24, 0.4, 0.14),
        max_branches: 0,
        foliage: FoliageConfig {
            style: FoliageStyle::Leafy,
            placement: FoliagePlacement::Terminal { min_level: 0 },
            color: FOLIAGE_GREEN_DARK,
            length: 0.1,
            width: 0.05,
            per_anchor: 1,
            anchors: 1,
            pitch_jitter: 0.0,
            cloud_points: 1,
        },
        grass: GrassConfig {
            tuft_height_min: 0.6,
            tuft_height_max: 1.4,
            tuft_radius: 0.35,
            blades_min: 150,
            blades_max: 300,
            blade_width: 0.015,
            blade_bend: 0.95,
            color: FOLIAGE_GREEN_MID,
            tuft_cloud_points: 4,
            field_radius: 50.0,
            tufts_min: 7500,
            tufts_max: 10000,
        },
        point_step: 0.1,
        jitter: 0.02,
        spread_radius: 13.0,
        neighbors_min: 0,
        neighbors_max: 0,
    }
}

/// Низкая редкая трава.
fn sparse_grass() -> PlantConfig {
    PlantConfig {
        seed: Seed::default(),
        preset: PlantPreset::SparseGrass,
        plant_type: PlantType::Grass,
        branches: Vec::new(),
        branch_color: Color::srgb(0.24, 0.4, 0.14),
        max_branches: 0,
        foliage: FoliageConfig {
            style: FoliageStyle::Leafy,
            placement: FoliagePlacement::Terminal { min_level: 0 },
            color: FOLIAGE_GREEN_DARK,
            length: 0.1,
            width: 0.05,
            per_anchor: 1,
            anchors: 1,
            pitch_jitter: 0.0,
            cloud_points: 1,
        },
        grass: GrassConfig {
            tuft_height_min: 0.12,
            tuft_height_max: 0.4,
            tuft_radius: 0.25,
            blades_min: 100,
            blades_max: 150,
            blade_width: 0.0015,
            blade_bend: 0.95,
            color: FOLIAGE_GREEN_DARK,
            tuft_cloud_points: 4,
            field_radius: 50.0,
            tufts_min: 7500,
            tufts_max: 10000,
        },
        point_step: 0.1,
        jitter: 0.02,
        spread_radius: 10.0,
        neighbors_min: 0,
        neighbors_max: 0,
    }
}

/// Пустой (неиспользуемый для деревьев/кустов) блок параметров травы.
fn grass_default() -> GrassConfig {
    GrassConfig {
        tuft_height_min: 0.4,
        tuft_height_max: 0.8,
        tuft_radius: 0.15,
        blades_min: 8,
        blades_max: 16,
        blade_width: 0.01,
        blade_bend: 0.3,
        color: FOLIAGE_GREEN_MID,
        tuft_cloud_points: 2,
        field_radius: 10.0,
        tufts_min: 40,
        tufts_max: 100,
    }
}

/// Ограниченный разброс ключевых параметров в пределах пресета.
fn jitter(cfg: &mut PlantConfig, preset: PlantPreset, rng: &mut StdRng) {
    let j = |rng: &mut StdRng, lo: f64, hi: f64| -> f64 { rng.random_range(lo..=hi) };

    // Ветки: лёгкое «дыхание» длин и «объёма» (разброс по кругу в целом).
    // Дрожим центр и размах интервалов, а не края по отдельности — иначе
    // при тесных прототипных диапазонах (0.8..0.85) min может «перепрыгнуть»
    // max, и `random_range` получит пустой интервал.
    for level in &mut cfg.branches {
        for b in level {
            let len_center = (b.length_min + b.length_max) * 0.5;
            let len_spread = (b.length_max - b.length_min) * 0.5;
            let ls = j(rng, 0.9, 1.1) as f32;
            b.length_min = (len_center - len_spread) * ls;
            b.length_max = (len_center + len_spread) * ls;

            // Для уровня 0 радиусы абсолютные — дрожим аккуратно, чтобы ствол
            // не стал ни слишком толстым, ни совсем тонким. Общий множитель
            // сохраняет порядок min <= max.
            let rk = if b.level == 0 { j(rng, 0.9, 1.1) as f32 } else { j(rng, 0.85, 1.2) as f32 };
            b.radius_min *= rk;
            b.radius_max *= rk;

            let v_center = (b.vertical_min + b.vertical_max) * 0.5;
            let v_spread = (b.vertical_max - b.vertical_min) * 0.5;
            // Один общий сдвиг центра для обоих краёв + размах >= 0: min<=max.
            let vc = v_center + j(rng, -4.0, 4.0) as f32;
            let vs = v_spread + j(rng, 0.0, 1.5) as f32;
            b.vertical_min = vc - vs;
            b.vertical_max = vc + vs;
        }
    }

    // Крона: небольшое изменение размера и насыщенности.
    cfg.foliage.length *= j(rng, 0.85, 1.15) as f32;
    cfg.foliage.width *= j(rng, 0.85, 1.15) as f32;
    if preset.plant_type() != PlantType::Grass {
        cfg.foliage.per_anchor = cfg
            .foliage
            .per_anchor
            .saturating_add(rng.random_range(0..=1));
    }

    // Пучки травы: пучки одного пресета различаются формой и густотой.
    cfg.grass.tuft_height_min *= j(rng, 0.85, 1.15) as f32;
    cfg.grass.tuft_height_max *= j(rng, 0.9, 1.1) as f32;
    cfg.grass.tuft_radius *= j(rng, 0.8, 1.2) as f32;
    if rng.random::<bool>() {
        cfg.grass.blades_max += 2;
        cfg.grass.blades_min = cfg.grass.blades_min.saturating_add(1);
    }

    // Расстановка: число соседей «дрожит» в пределах пресета.
    cfg.neighbors_max += rng.random_range(0..=2);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_is_deterministic() {
        for preset in PlantPreset::ALL {
            for seed in 0..4u64 {
                let a = resolve(preset, Seed::new(seed));
                let b = resolve(preset, Seed::new(seed));
                assert_eq!(a.preset, preset);
                assert_eq!(a.seed, Seed::new(seed));
                assert_eq!(a.branches.len(), b.branches.len());
                assert_eq!(a.foliage.length, b.foliage.length);
                assert_eq!(a.grass.blades_min, b.grass.blades_min);
                assert_eq!(a.neighbors_max, b.neighbors_max);
            }
        }
    }

    #[test]
    fn presets_are_distinct() {
        let tree = resolve(PlantPreset::DeciduousTree, Seed::new(1));
        let bush = resolve(PlantPreset::Bush, Seed::new(1));
        // У дерева ствол заметно толще кустового стволика.
        let tree_trunk = tree.branches[0][0].radius_max;
        let bush_trunk = bush.branches[0][0].radius_max;
        assert!(tree_trunk > bush_trunk * 5.0, "ствол дерева {tree_trunk} vs куста {bush_trunk}");
    }

    #[test]
    fn grass_presets_differ_in_height() {
        let tall = resolve(PlantPreset::TallGrass, Seed::new(1));
        let short = resolve(PlantPreset::SparseGrass, Seed::new(1));
        assert!(
            tall.grass.tuft_height_min > short.grass.tuft_height_max,
            "высокая трава должна быть выше низкой"
        );
        assert!(tall.grass.blades_max > short.grass.blades_max);
    }
}
