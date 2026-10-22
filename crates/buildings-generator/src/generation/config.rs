//! Настройки генерации здания — полностью детерминированы от seed.

use bevy::ecs::resource::Resource;
use generation_core::Seed;
use serde::{Deserialize, Serialize};

/// Тип крыши.
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
    /// Полувальмовая (голландская): вальма, усечённая небольшим фронтоном.
    HalfHip,
    /// Сводчатая (бочарная): арка по всей длине.
    Barrel,
}

impl RoofStyle {
    /// Все типы крыш (для случайного выбора).
    pub const ALL: [Self; 6] = [
        Self::Flat,
        Self::Shed,
        Self::Gable,
        Self::Hip,
        Self::HalfHip,
        Self::Barrel,
    ];

    /// Человекочитаемое имя.
    pub fn label(self) -> &'static str {
        match self {
            Self::Flat => "плоская",
            Self::Shed => "односкатная",
            Self::Gable => "двускатная",
            Self::Hip => "вальмовая",
            Self::HalfHip => "полувальмовая",
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
            balcony_width: 1.6,
            balcony_depth: 1.0,
            balcony_railing_height: 0.95,
            balcony_floor_level: 0.7,
            dormer_probability: 0.5,
            dormer_width: 1.0,
            dormer_depth: 0.9,
            dormer_height: 1.1,
            chimney_probability: 0.6,
            point_step: 0.25,
            wall_point_step: 0.38,
            point_jitter: 0.02,
        }
    }
}
