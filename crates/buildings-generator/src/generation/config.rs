//! Настройки генерации здания — полностью детерминированы от seed.

use bevy::ecs::resource::Resource;
use generation_core::Seed;
use serde::{Deserialize, Serialize};

/// Preset controlling the building footprint.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuildingPreset {
    #[default]
    Rectangular,
    LShaped,
    WithAnnex,
    ApartmentBlock,
    MultiEntranceApartment,
    Skyscraper,
    Hangar,
    Garage,
    Barn,
}
impl BuildingPreset {
    pub const ALL: [Self; 9] = [
        Self::Rectangular,
        Self::LShaped,
        Self::WithAnnex,
        Self::ApartmentBlock,
        Self::MultiEntranceApartment,
        Self::Skyscraper,
        Self::Hangar,
        Self::Garage,
        Self::Barn,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Rectangular => "Прямоугольный",
            Self::LShaped => "Г-образный",
            Self::WithAnnex => "Дом с пристройкой",
            Self::ApartmentBlock => "Многоквартирный дом",
            Self::MultiEntranceApartment => "Многоподъездный дом",
            Self::Skyscraper => "Небоскрёб",
            Self::Hangar => "Ангар",
            Self::Garage => "Гараж",
            Self::Barn => "Амбар",
        }
    }

    pub fn is_tall(self) -> bool {
        matches!(
            self,
            Self::ApartmentBlock | Self::MultiEntranceApartment | Self::Skyscraper
        )
    }

    pub fn is_outbuilding(self) -> bool {
        matches!(self, Self::Hangar | Self::Garage | Self::Barn)
    }

    pub fn roof_styles(self) -> &'static [RoofStyle] {
        match self {
            Self::Hangar => &[RoofStyle::Gable, RoofStyle::Barrel],
            Self::Garage => &[RoofStyle::Flat, RoofStyle::Gable, RoofStyle::Shed],
            Self::Barn => &[RoofStyle::Gable],
            _ if self.is_tall() => &[RoofStyle::Flat],
            _ => &RoofStyle::ALL,
        }
    }
}

/// Roof style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RoofStyle {
    /// Плоская (с парапетом или со свесом).
    Flat,
    /// Односкатная.
    Shed,
    /// Двускатная.
    Gable,
    /// Вальмовая (шатровая).
    Hip,
    /// Полувальмовая четырёхскатная (голландская): нижние вальмы и
    /// небольшие фронтоны над ними.
    HalfHip,
    /// Полувальмовая двускатная: два основных ската, фронтоны и малые
    /// вальмы, срезающие концы конька.
    HalfHipGable,
    /// Сводчатая (бочарная): арка по всей длине.
    Barrel,
}

impl RoofStyle {
    /// Все типы крыш (для случайного выбора).
    pub const ALL: [Self; 7] = [
        Self::Flat,
        Self::Shed,
        Self::Gable,
        Self::Hip,
        Self::HalfHip,
        Self::HalfHipGable,
        Self::Barrel,
    ];

    /// Человекочитаемое имя.
    pub fn label(self) -> &'static str {
        match self {
            Self::Flat => "плоская",
            Self::Shed => "односкатная",
            Self::Gable => "двускатная",
            Self::Hip => "вальмовая",
            Self::HalfHip => "полувальмовая (четырёхскатная)",
            Self::HalfHipGable => "полувальмовая (двускатная)",
            Self::Barrel => "сводчатая",
        }
    }

    /// Требует ли наклон (pitch).
    pub fn is_pitched(self) -> bool {
        !matches!(self, Self::Flat)
    }
}

/// Конфигурация генерации одного здания.
#[derive(Resource, Debug, Clone, Serialize, Deserialize)]
pub struct BuildingConfig {
    #[serde(default)]
    pub preset: BuildingPreset,
    /// Глобальный seed — всё остальное детерминировано от него.
    pub seed: Seed,

    // --- габариты ---
    /// Минимальная сторона основания, м.
    pub min_size: f32,
    /// Максимальная сторона основания, м.
    pub max_size: f32,
    /// Максимальное отношение длинной стороны к короткой (от квадрата до 1:2).
    pub max_ratio: f32,

    // --- этажи ---
    pub min_floors: u32,
    pub max_floors: u32,

    // --- стены ---
    /// Высота этажа (min).
    pub floor_height_min: f32,
    /// Высота этажа (max).
    pub floor_height_max: f32,
    /// Толщина стен, м.
    pub wall_thickness: f32,

    // --- крыша ---
    /// Свес крыши (min), м.
    pub roof_overhang_min: f32,
    /// Свес крыши (max), м.
    pub roof_overhang_max: f32,
    /// Наклон скатов (min), градусы.
    pub roof_pitch_min: f32,
    /// Наклон скатов (max), градусы.
    pub roof_pitch_max: f32,
    /// Наклон односкатной крыши (min), градусы — положе остальных.
    pub shed_pitch_min: f32,
    /// Наклон односкатной крыши (max), градусы.
    pub shed_pitch_max: f32,
    /// Максимальный сдвиг гребня двускатной/полувальмовой крыши от центра
    /// (доля полуширины) — углы скатов при этом различаются.
    pub max_ridge_shift: f32,
    /// Толщина «карнизных досок», м.
    pub roof_thickness: f32,
    /// Вероятность парапета у плоской крыши.
    pub parapet_probability: f32,
    /// Высота парапета, м.
    pub parapet_height: f32,

    // --- окна и двери ---
    pub window_width: f32,
    pub window_height: f32,
    /// Отступ окон от углов, м.
    pub window_margin: f32,
    /// Минимальный зазор между окнами, м.
    pub window_gap: f32,
    pub door_width: f32,
    pub door_height: f32,

    // --- балконы ---
    /// Вероятность того, что на не-первом этаже появится балкон.
    pub balcony_probability: f32,
    /// Доля остеклённых балконов среди появившихся (0 — все открытые, 1 — все закрытые).
    #[serde(default = "default_enclosed_balcony_probability")]
    pub enclosed_balcony_probability: f32,
    #[serde(default = "default_door_canopy_probability")]
    pub door_canopy_probability: f32,
    #[serde(default = "default_balcony_canopy_probability")]
    pub balcony_canopy_probability: f32,
    pub balcony_width: f32,
    pub balcony_depth: f32,
    pub balcony_railing_height: f32,
    /// Высота пола балкона над уровнем этажа, м.
    pub balcony_floor_level: f32,

    // --- мансарда ---
    /// Вероятность мансарды у двускатной/вальмовой крыши.
    pub dormer_probability: f32,
    pub dormer_width: f32,
    pub dormer_depth: f32,
    pub dormer_height: f32,

    // --- дымоход ---
    /// Вероятность дымохода на крыше.
    pub chimney_probability: f32,

    // --- облако точек ---
    /// Шаг сетки облака точек, м.
    pub point_step: f32,
    /// Шаг сетки облака точек на Больших стенах (крупнее, чтобы стены не
    /// выглядели плотнее крыши).
    pub wall_point_step: f32,
    /// Амплитуда шума сетки, м.
    pub point_jitter: f32,
}

impl Default for BuildingConfig {
    fn default() -> Self {
        Self {
            preset: BuildingPreset::default(),
            seed: Seed::default(),
            min_size: 5.0,
            max_size: 15.0,
            max_ratio: 2.0,
            min_floors: 1,
            max_floors: 3,
            floor_height_min: 2.6,
            floor_height_max: 3.2,
            wall_thickness: 0.24,
            roof_overhang_min: 0.25,
            roof_overhang_max: 0.5,
            roof_pitch_min: 14.0,
            roof_pitch_max: 40.0,
            shed_pitch_min: 5.0,
            shed_pitch_max: 20.0,
            max_ridge_shift: 0.35,
            roof_thickness: 0.14,
            parapet_probability: 0.5,
            parapet_height: 0.6,
            window_width: 1.0,
            window_height: 1.25,
            window_margin: 0.45,
            window_gap: 0.6,
            door_width: 0.9,
            door_height: 2.05,
            balcony_probability: 0.6,
            enclosed_balcony_probability: default_enclosed_balcony_probability(),
            door_canopy_probability: default_door_canopy_probability(),
            balcony_canopy_probability: default_balcony_canopy_probability(),
            balcony_width: 1.6,
            balcony_depth: 1.0,
            balcony_railing_height: 0.95,
            balcony_floor_level: 0.7,
            dormer_probability: 0.5,
            dormer_width: 2.4,
            dormer_depth: 1.8,
            dormer_height: 1.7,
            chimney_probability: 0.6,
            point_step: 0.25,
            wall_point_step: 0.38,
            point_jitter: 0.02,
        }
    }
}

fn default_door_canopy_probability() -> f32 {
    0.55
}
fn default_balcony_canopy_probability() -> f32 {
    0.6
}

fn default_enclosed_balcony_probability() -> f32 {
    0.35
}
