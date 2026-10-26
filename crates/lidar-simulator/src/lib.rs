//! Сканирование плотных облаков: поле зрения, затенение геометрией, шаг и шум.
//! Все координаты API — мировые, Y вверх. Рендеринг для расчётов не нужен.

pub mod demo;
mod occlusion;

use std::collections::BTreeMap;

use bevy::prelude::*;
use buildings_generator::{Building, generation::Part};
use generation_core::{Point, PointClass, PointCloud, Seed};
use rand::RngExt;

use occlusion::Occlusion;

/// Независимые от плотности генератора параметры измерения.
#[derive(Clone, Debug)]
pub struct ScannerConfig {
    /// Размер мировой ячейки отбора, м: один ближайший видимый отсчёт на ячейку.
    pub sample_step: f32,
    pub horizontal_fov: f32,
    pub vertical_fov: f32,
    pub min_range: f32,
    pub max_range: f32,
    /// Вероятность ДОБАВИТЬ шумовую точку к каждому принятому отсчёту.
    pub noise_probability: f32,
    /// Максимальное смещение добавленной точки по каждой оси, м.
    pub noise_extent: f32,
    /// Допуск к поверхности, м (для численных ошибок и jitter генератора).
    pub surface_tolerance: f32,
}

impl Default for ScannerConfig {
    fn default() -> Self {
        Self {
            sample_step: 0.25,
            horizontal_fov: 90.0_f32.to_radians(),
            vertical_fov: 70.0_f32.to_radians(),
            min_range: 0.1,
            max_range: 150.0,
            noise_probability: 0.02,
            noise_extent: 0.12,
            surface_tolerance: 0.005,
        }
    }
}

impl ScannerConfig {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !self.sample_step.is_finite() || self.sample_step <= 0.0 {
            return Err("sample_step должен быть конечным и положительным");
        }
        for angle in [self.horizontal_fov, self.vertical_fov] {
            if !angle.is_finite() || angle <= 0.0 || angle >= std::f32::consts::PI {
                return Err("поле зрения должно быть между 0 и PI");
            }
        }
        if !self.min_range.is_finite()
            || !self.max_range.is_finite()
            || self.min_range < 0.0
            || self.max_range <= self.min_range
        {
            return Err("некорректный диапазон дальности");
        }
        if !self.noise_probability.is_finite() || !(0.0..=1.0).contains(&self.noise_probability) {
            return Err("вероятность шума должна быть от 0 до 1");
        }
        if [self.noise_extent, self.surface_tolerance]
            .iter()
            .any(|v| !v.is_finite() || *v < 0.0)
        {
            return Err("амплитуда шума и допуск должны быть конечными и неотрицательными");
        }
        Ok(())
    }
}

/// Положение и ориентация сканера, локальный взгляд вдоль -Z.
#[derive(Clone, Copy, Debug)]
pub struct ScannerPose {
    pub position: Vec3,
    pub rotation: Quat,
}

impl ScannerPose {
    pub fn looking_at(position: Vec3, target: Vec3) -> Self {
        let transform = Transform::from_translation(position).looking_at(target, Vec3::Y);
        Self {
            position,
            rotation: transform.rotation,
        }
    }
}

/// Исходные точки и непрозрачные треугольники, ускоренные деревом AABB.
pub struct ScanScene {
    pub cloud: PointCloud,
    occlusion: Occlusion,
}

impl ScanScene {
    pub fn new(cloud: PointCloud, triangles: Vec<[Vec3; 3]>) -> Self {
        Self {
            cloud,
            occlusion: Occlusion::new(triangles),
        }
    }

    /// Применяет всю иерархию Transform и переводит Z-up генератора в Y-up.
    pub fn from_building(building: &Building) -> Self {
        fn collect(
            part: &Part,
            parent: Mat4,
            cloud: &mut PointCloud,
            triangles: &mut Vec<[Vec3; 3]>,
        ) {
            let world = parent
                * Mat4::from_scale_rotation_translation(
                    part.transform.scale,
                    part.transform.rotation,
                    part.transform.translation,
                );
            let mut local = part.cloud.clone();
            local.transform_by(world);
            cloud.merge(&local);
            for face in &part.faces {
                let vertices: Vec<_> = face
                    .triangulate()
                    .map(|p| world.transform_point3(p))
                    .collect();
                for triangle in vertices.chunks_exact(3) {
                    triangles.push([triangle[0], triangle[1], triangle[2]]);
                }
            }
            for child in &part.children {
                collect(child, world, cloud, triangles);
            }
        }
        let mut cloud = PointCloud::default();
        let mut triangles = Vec::new();
        collect(
            &building.root,
            Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_2),
            &mut cloud,
            &mut triangles,
        );
        Self::new(cloud, triangles)
    }

    /// Проверка видимости предшествует прореживанию: скрытая точка не занимает ячейку.
    pub fn scan(
        &self,
        pose: ScannerPose,
        config: &ScannerConfig,
        seed: Seed,
    ) -> Result<ScanFrame, &'static str> {
        config.validate()?;
        if !pose.position.is_finite()
            || !pose.rotation.is_finite()
            || !pose.rotation.is_normalized()
        {
            return Err("некорректная поза сканера");
        }
        let mut cells = BTreeMap::<(i64, i64, i64), (usize, f32)>::new();
        let tan_h = (config.horizontal_fov * 0.5).tan();
        let tan_v = (config.vertical_fov * 0.5).tan();
        for (index, point) in self.cloud.points.iter().enumerate() {
            let delta = point.position - pose.position;
            let distance = delta.length();
            if !distance.is_finite()
                || distance < config.min_range
                || distance > config.max_range
                || distance <= f32::EPSILON
            {
                continue;
            }
            let local = pose.rotation.inverse() * delta;
            let depth = -local.z;
            if depth <= 0.0 || local.x.abs() > depth * tan_h || local.y.abs() > depth * tan_v {
                continue;
            }
            if self.occlusion.blocked(
                pose.position,
                delta / distance,
                distance - config.surface_tolerance,
            ) {
                continue;
            }
            let p = (point.position / config.sample_step).floor();
            let key = (p.x as i64, p.y as i64, p.z as i64);
            let cell = cells.entry(key).or_insert((index, distance));
            if distance < cell.1 {
                *cell = (index, distance);
            }
        }
        let mut source_indices: Vec<_> = cells.values().map(|(index, _)| *index).collect();
        source_indices.sort_unstable();
        let mut cloud = PointCloud::default();
        cloud
            .points
            .extend(source_indices.iter().map(|&i| self.cloud.points[i]));
        let mut rng = seed.rng();
        for &index in &source_indices {
            if rng.random::<f32>() < config.noise_probability {
                let offset = Vec3::new(
                    rng.random_range(-config.noise_extent..=config.noise_extent),
                    rng.random_range(-config.noise_extent..=config.noise_extent),
                    rng.random_range(-config.noise_extent..=config.noise_extent),
                );
                cloud.points.push(Point::new(
                    self.cloud.points[index].position + offset,
                    PointClass::Noise,
                ));
            }
        }
        Ok(ScanFrame {
            source_indices,
            cloud,
        })
    }
}

/// Сначала в cloud идут исходные отсчёты, затем добавленные Low Noise.
#[derive(Clone, Debug)]
pub struct ScanFrame {
    pub source_indices: Vec<usize>,
    pub cloud: PointCloud,
}

/// Накопитель одной сцены: повторные наблюдения исходных точек не дублируются.
pub struct ScanAccumulator {
    pub seen: Vec<bool>,
    pub cloud: PointCloud,
    pub frames: usize,
}

impl ScanAccumulator {
    pub fn new(source_count: usize) -> Self {
        Self {
            seen: vec![false; source_count],
            cloud: PointCloud::default(),
            frames: 0,
        }
    }

    pub fn add(&mut self, frame: &ScanFrame) {
        for (&index, &point) in frame.source_indices.iter().zip(&frame.cloud.points) {
            if !self.seen[index] {
                self.seen[index] = true;
                self.cloud.points.push(point);
            }
        }
        self.cloud
            .points
            .extend_from_slice(&frame.cloud.points[frame.source_indices.len()..]);
        self.frames += 1;
    }
}

/// Замкнутая круговая траектория: последнее положение не повторяет первое.
pub fn orbit_trajectory(target: Vec3, radius: f32, height: f32, frames: usize) -> Vec<ScannerPose> {
    (0..frames)
        .map(|i| {
            let angle = std::f32::consts::TAU * i as f32 / frames as f32;
            ScannerPose::looking_at(
                Vec3::new(
                    target.x + radius * angle.sin(),
                    height,
                    target.z + radius * angle.cos(),
                ),
                target,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests;
