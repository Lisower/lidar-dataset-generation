//! Здание: данные, генерация и сборка облака точек.

use bevy::color::Color;
use bevy::math::Vec2;
use bevy::math::Vec3;
use bevy::transform::components::Transform;
use generation_core::occupancy::{Occupancy, OccupancyShape};
use generation_core::PointCloud;
use generation_core::Seed;
use rand::RngExt;

use super::config::{BuildingConfig, RoofStyle};
use super::elements::{
    Facade, FaceCutout, WallCtx, balcony_part, door_part, wall_part, window_part,
};
use super::palette::{DOORS, ROOFS, WALLS, WINDOWS, pick};
use super::part::Part;
use super::roof::roof_part;

/// Цвета элементов здания.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BuildingColors {
    /// Стены.
    pub wall: Color,
    /// Крыша (и парапет, и кровля мансарды).
    pub roof: Color,
    /// Окна (стекло).
    pub window: Color,
    /// Двери.
    pub door: Color,
    /// Акцент (ограждение балконов) — может быть отдельным.
    pub railing: Color,
}

/// Сгенерированное здание.
#[derive(Debug, Clone)]
pub struct Building {
    /// Конфигурация, из которой оно получено.
    pub config: BuildingConfig,
    /// Seed генерации (сохранён для отчёта).
    pub seed: Seed,

    /// Габариты: длина (X), ширина (Y), высота стен (Z).
    pub size: Vec3,
    /// Число этажей.
    pub floors: u32,
    /// Высота этажа.
    pub floor_height: f32,

    /// Тип крыши.
    pub roof: RoofStyle,
    /// Наклон скатов, градусы (0 для плоской).
    pub roof_pitch_deg: f32,
    /// Свес крыши.
    pub overhang: f32,
    /// Есть парапет.
    pub parapet: bool,
    /// Есть мансарда.
    pub dormer: bool,

    /// Цвета элементов.
    pub colors: BuildingColors,
    /// Корневая часть-дерево здания (этажи → стены → окна и т.д.).
    pub root: Part,
    /// Занимаемая часть территории (для terrains-generator).
    pub occupancy: Occupancy,
    /// Полное облако точек здания (в системе координат здания).
    pub point_cloud: PointCloud,
}

impl Building {
    /// Сгенерировать здание по конфигурации (детерминированно от seed).
    pub fn generate(config: &BuildingConfig) -> Building {
        let seed = config.seed;
        let mut rng = seed.rng();

        // --- габариты: от квадрата до прямоугольника 1:2 ---
        let short = rng.random_range(config.min_size..=config.max_size);
        let ratio = rng.random_range(1.0..=config.max_ratio);
        let long = (short * ratio).min(config.max_size);
        let swap = rng.random::<bool>();
        let (length, width) = if swap { (long, short) } else { (short, long) };

        // --- этажи ---
        let floors = rng.random_range(config.min_floors..=config.max_floors);
        let floor_height = rng.random_range(config.floor_height_min..=config.floor_height_max);
        let wall_height = floor_height * floors as f32;

        // --- крыша ---
        let roof = RoofStyle::ALL[rng.random_range(0..RoofStyle::ALL.len())];
        // Односкатным — заметно более пологий наклон (5–20°).
        let roof_pitch_deg = if roof == RoofStyle::Shed {
            rng.random_range(config.shed_pitch_min..=config.shed_pitch_max)
        } else if roof.is_pitched() {
            rng.random_range(config.roof_pitch_min..=config.roof_pitch_max)
        } else {
            0.0
        };
        let overhang = rng.random_range(config.roof_overhang_min..=config.roof_overhang_max);
        let parapet = roof == RoofStyle::Flat && rng.random::<f32>() < config.parapet_probability;
        let dormer =
            roof.is_pitched() && rng.random::<f32>() < config.dormer_probability;

        // --- цвета ---
        let colors = BuildingColors {
            wall: pick(&mut rng, &WALLS),
            roof: pick(&mut rng, &ROOFS),
            window: pick(&mut rng, &WINDOWS),
            door: pick(&mut rng, &DOORS),
            railing: pick(&mut rng, &DOORS),
        };

        // --- построение дерева частей ---
        let mut root = Part::group("здание", colors.wall, Transform::IDENTITY);

        // Фасад с входной дверью — случайный.
        let front = Facade::ALL[rng.random_range(0..Facade::ALL.len())];

        for floor_idx in 0..floors {
            // На не-первом этаже с некоторой вероятностью появится балкон
            // на одном из фасадов.
            let balcony_facade = if floor_idx > 0 && rng.random::<f32>() < config.balcony_probability {
                Some(Facade::ALL[rng.random_range(0..Facade::ALL.len())])
            } else {
                None
            };

            let mut floor = Part::group(
                "этаж",
                colors.wall,
                Transform::from_xyz(0.0, 0.0, floor_idx as f32 * floor_height),
            );

            for facade in Facade::ALL {
                // North/South стены короче на свою толщину: торцы упираются во
                // внутренние грани East/West стен, углы не «наслаиваются»
                // (иначе меш в меше и мерцание на углах).
                let length_along = match facade {
                    Facade::North | Facade::South => length - 2.0 * config.wall_thickness,
                    Facade::East | Facade::West => width,
                };
                let ctx = WallCtx {
                    facade,
                    center: wall_center(facade, length, width, config.wall_thickness),
                    thickness: config.wall_thickness,
                };

                let balcony_here = balcony_facade == Some(facade);

                // Окна: случайное число (немного), случайная ширина (высота
                // общая), ровный ряд по горизонтали одного этажа, сдвиг по
                // вертикали от этажа к этажу. Зоны двери и балкона не
                // пересекаются, от углов и между собой окна не вплотную.
                let mut reserved: Vec<(f32, f32)> = Vec::new();
                if floor_idx == 0 && facade == front {
                    let half = config.door_width * 0.5 + 0.6;
                    reserved.push((-half, half));
                }
                if balcony_here {
                    let half = config.balcony_width * 0.5 + 0.5;
                    reserved.push((-half, half));
                }
                let windows = window_specs(
                    seed.with_label(&format!("окна-{floor_idx}-{}", facade.label())),
                    length_along,
                    config,
                    &reserved,
                );

                // Проёмы на внешней грани стены (в локальных координатах от
                // центра стены) — чтобы облако стены не дублировало точки
                // окон, дверей и балкона.
                let hz = floor_height * 0.5;
                let pad = 0.08;
                let mut cutouts: Vec<FaceCutout> = Vec::new();
                for w in &windows {
                    cutouts.push(FaceCutout {
                        along_min: w.center - w.width * 0.5 - pad,
                        along_max: w.center + w.width * 0.5 + pad,
                        z_min: w.z_from_center - config.window_height * 0.5 - pad,
                        z_max: w.z_from_center + config.window_height * 0.5 + pad,
                    });
                }
                if floor_idx == 0 && facade == front {
                    // Дверь: от пола этажа до верха двери.
                    cutouts.push(FaceCutout {
                        along_min: -config.door_width * 0.5 - pad,
                        along_max: config.door_width * 0.5 + pad,
                        z_min: -hz,
                        z_max: -hz + config.door_height + pad,
                    });
                }
                if balcony_here {
                    // Дверь на балкон: только её саму, чтобы стена вокруг
                    // двери сохранила облако точек.
                    let bdoor_h =
                        (floor_height - config.balcony_floor_level).clamp(1.4, config.door_height);
                    cutouts.push(FaceCutout {
                        along_min: -config.door_width * 0.5 - pad,
                        along_max: config.door_width * 0.5 + pad,
                        z_min: config.balcony_floor_level - floor_height * 0.5 - pad,
                        z_max: config.balcony_floor_level - floor_height * 0.5 + bdoor_h + pad,
                    });
                }

                let mut wall = wall_part(
                    &ctx,
                    length_along,
                    floor_height,
                    colors.wall,
                    config,
                    &cutouts,
                );

                for w in &windows {
                    wall = wall.child(window_part(
                        &ctx,
                        w.center,
                        w.z_from_center,
                        w.width,
                        colors.window,
                        config,
                    ));
                }

                if balcony_here {
                    wall = wall.child(balcony_part(
                        &ctx,
                        floor_height,
                        colors.door,
                        colors.wall,
                        config,
                    ));
                }

                if floor_idx == 0 && facade == front {
                    // Дверь: низ коробки — на уровне пола этажа (земли).
                    wall = wall.child(door_part(
                        &ctx,
                        (config.door_height + 0.14) * 0.5 - floor_height * 0.5,
                        colors.door,
                        config.door_height,
                        config,
                    ));
                }

                floor = floor.child(wall);
            }

            root = root.child(floor);
        }

        // Крыша.
        let roof_part = roof_part(
            length,
            width,
            wall_height,
            roof,
            overhang,
            roof_pitch_deg,
            parapet,
            dormer,
            &colors,
            config,
        );
        root = root.child(roof_part);

        // Занимаемая территория (с небольшим запасом под свесы).
        let occupancy = Occupancy {
            center: Vec2::ZERO,
            rotation: 0.0,
            shape: OccupancyShape::Square {
                half_extent: Vec2::new(
                    length * 0.5 + overhang + 0.2,
                    width * 0.5 + overhang + 0.2,
                ),
            },
        };

        let point_cloud = root.collect_cloud();

        Building {
            config: config.clone(),
            seed,
            size: Vec3::new(length, width, wall_height),
            floors,
            floor_height,
            roof,
            roof_pitch_deg,
            overhang,
            parapet,
            dormer,
            colors,
            root,
            occupancy,
            point_cloud,
        }
    }

    /// Высота с учётом крыши (для камеры).
    pub fn total_height(&self) -> f32 {
        // Гребень может идти вдоль длинной или короткой стороны (поворот
        // крыши), поэтому берём большую из сторон — для камеры важно не
        // обрезать конёк.
        let roof_rise = if self.roof.is_pitched() {
            (self.roof_pitch_deg.to_radians()).tan()
                * (self.size.x.max(self.size.y) * 0.5 + self.overhang)
        } else {
            self.config.roof_thickness
        };
        self.size.z + roof_rise + 1.0
    }

    /// Диаметр (габарит по основанию) для камеры.
    pub fn footprint_diagonal(&self) -> f32 {
        (self.size.x + self.overhang * 2.0).hypot(self.size.y + self.overhang * 2.0)
    }
}

/// Центр стены по толщине (мировые координаты здания).
fn wall_center(facade: Facade, length: f32, width: f32, thickness: f32) -> Vec3 {
    let t = thickness * 0.5;
    match facade {
        Facade::North => Vec3::new(0.0, width * 0.5 - t, 0.0),
        Facade::South => Vec3::new(0.0, -width * 0.5 + t, 0.0),
        Facade::East => Vec3::new(length * 0.5 - t, 0.0, 0.0),
        Facade::West => Vec3::new(-length * 0.5 + t, 0.0, 0.0),
    }
}

/// Окно этажа на фасаде.
struct WindowSpec {
    /// Смещение центра вдоль стены.
    center: f32,
    /// Ширина окна (высота — общая из конфигурации).
    width: f32,
    /// Смещение центра по вертикали от центра стены.
    z_from_center: f32,
}

/// Разложить окна вдоль стены (детерминировано от `seed`).
///
/// Окон намеренно немного; они одинаковой высоты, но случайной ширины и
/// ровным рядом по горизонтали в пределах этажа; от этажа к этажу ряд может
/// сдвигаться по вертикали. Зоны `reserved` (дверь/балкон) пропускаются,
/// от углов — запас.
fn window_specs(
    seed: Seed,
    length_along: f32,
    config: &BuildingConfig,
    reserved: &[(f32, f32)],
) -> Vec<WindowSpec> {
    let mut rng = seed.rng();
    const MIN_W: f32 = 0.6;
    const MAX_W: f32 = 1.35;
    const GAP: f32 = 0.55;
    let corner = config.window_margin.max(0.9);

    let usable = length_along - 2.0 * corner;
    if usable <= MIN_W + GAP {
        return Vec::new();
    }

    // Свободные сегменты: весь ряд минус зарезервированные зоны.
    let lo = -usable * 0.5;
    let hi = usable * 0.5;
    let mut right = lo;
    let mut segments: Vec<(f32, f32)> = Vec::new();
    for &(a, b) in reserved {
        let a = a.max(lo);
        let b = b.min(hi);
        if b <= a {
            continue;
        }
        if right < a {
            segments.push((right, a));
        }
        right = right.max(b);
    }
    if right < hi {
        segments.push((right, hi));
    }
    segments.retain(|&(a, b)| b - a >= MIN_W + GAP);
    if segments.is_empty() {
        return Vec::new();
    }

    // Число окон — заметно меньше максимума (окон должно быть «поменьше»).
    let cap: usize = segments
        .iter()
        .map(|&(a, b)| ((b - a - GAP) / (MIN_W + GAP)).max(0.0).floor() as usize)
        .sum();
    if cap == 0 {
        return Vec::new();
    }
    let k = ((cap as f32) * rng.random_range(0.35..0.8))
        .round()
        .max(1.0) as usize;
    let k = k.min(cap);

    // Все окна этажа — одна строка по вертикали (сдвиг общий для фасада).
    let z_shift = rng.random_range(-0.3..0.15);
    let total: f32 = segments.iter().map(|&(a, b)| b - a).sum();

    let mut windows = Vec::new();
    let mut remaining = k;
    for &(a, b) in &segments {
        if remaining == 0 {
            break;
        }
        let n_seg = if segments.len() == 1 {
            remaining
        } else {
            ((((k as f32) * (b - a)) / total).round() as usize).min(remaining)
        };
        if n_seg == 0 {
            continue;
        }
        remaining -= n_seg;
        let slot = (b - a) / n_seg as f32;
        for i in 0..n_seg {
            let jitter = rng.random_range(-0.5..0.5) * slot * 0.3;
            let center =
                (a + slot * (i as f32 + 0.5) + jitter).clamp(a + MIN_W * 0.35, b - MIN_W * 0.35);
            let mut width = rng.random_range(MIN_W..MAX_W);
            width = width.min((slot - GAP).max(MIN_W));
            windows.push(WindowSpec {
                center,
                width,
                z_from_center: z_shift,
            });
        }
    }
    windows
}
