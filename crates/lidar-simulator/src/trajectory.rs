//! Воспроизводимые маршруты и автоматическое наведение по границам сцены.
use crate::{ScannerConfig, ScannerPose, orbit_trajectory};
use bevy::prelude::*;
use generation_core::{PointCloud, Seed};
use rand::RngExt;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TrajectoryKind {
    #[default]
    Orbit,
    Line,
}

#[derive(Clone, Copy, Debug)]
pub struct SceneBounds {
    pub min: Vec3,
    pub max: Vec3,
}

impl SceneBounds {
    pub fn from_cloud(cloud: &PointCloud) -> Option<Self> {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for p in cloud.positions().filter(|p| p.is_finite()) {
            min = min.min(p);
            max = max.max(p);
        }
        min.is_finite().then_some(Self { min, max })
    }

    pub fn center(self) -> Vec3 {
        (self.min + self.max) * 0.5
    }
    pub fn radius(self) -> f32 {
        ((self.max - self.min).length() * 0.5).max(1.0)
    }

    /// Сфера вокруг AABB гарантирует, что траектории не пересекают объект.
    /// Прямые проходят через расширенную область, с произвольным азимутом
    /// и боковым смещением: возможны пролёты сверху и сбоку от карты.
    pub fn trajectory(self, kind: TrajectoryKind, frames: usize, seed: Seed) -> Vec<ScannerPose> {
        let mut rng = seed.with_label("scanner-trajectory").rng();
        let center = self.center();
        let radius = self.radius();
        match kind {
            TrajectoryKind::Orbit => {
                let height = rng.random_range(self.min.y..self.max.y + 1.5 * radius);
                let phase = rng.random_range(0.0..std::f32::consts::TAU);
                let rotation = Quat::from_rotation_y(phase);
                orbit_trajectory(center, radius * rng.random_range(1.3..2.5), height, frames)
                    .into_iter()
                    .map(|pose| {
                        ScannerPose::looking_at(
                            center + rotation * (pose.position - center),
                            center,
                        )
                    })
                    .collect()
            }
            TrajectoryKind::Line => {
                let angle = rng.random_range(0.0..std::f32::consts::TAU);
                let direction = Vec3::new(angle.cos(), 0.0, angle.sin());
                let side = Vec3::new(-direction.z, 0.0, direction.x);
                let offset = rng.random_range(-1.8_f32..1.8) * radius;
                let min_height = if offset.abs() > radius {
                    self.min.y
                } else {
                    self.max.y + 0.4 * radius
                };
                let start_height = rng.random_range(min_height..self.max.y + 1.5 * radius);
                let end_height = rng.random_range(min_height..self.max.y + 1.5 * radius);
                let midpoint = Vec3::new(center.x, 0.0, center.z) + side * offset;
                (0..frames)
                    .map(|i| {
                        let t = if frames == 1 {
                            0.5
                        } else {
                            i as f32 / (frames - 1) as f32
                        };
                        ScannerPose::looking_at(
                            midpoint
                                + direction * ((t - 0.5) * 6.0 * radius)
                                + Vec3::Y * (start_height + t * (end_height - start_height)),
                            center,
                        )
                    })
                    .collect()
            }
        }
    }

    /// Поле зрения охватывает угловые проекции всех восьми углов AABB.
    /// Дальность увеличивается до границ сцены; шаг и шум сохраняются.
    pub fn fit_config(self, pose: ScannerPose, base: &ScannerConfig) -> ScannerConfig {
        let mut horizontal = 0.0_f32;
        let mut vertical = 0.0_f32;
        for x in [self.min.x, self.max.x] {
            for y in [self.min.y, self.max.y] {
                for z in [self.min.z, self.max.z] {
                    let p = pose.rotation.inverse() * (Vec3::new(x, y, z) - pose.position);
                    horizontal = horizontal.max(p.x.abs().atan2(-p.z));
                    vertical = vertical.max(p.y.abs().atan2(-p.z));
                }
            }
        }
        let mut config = base.clone();
        config.max_range = config
            .max_range
            .max(pose.position.distance(self.center()) + self.radius() + 1.0);
        config.horizontal_fov = (horizontal * 2.0 + 5.0_f32.to_radians())
            .clamp(10.0_f32.to_radians(), 175.0_f32.to_radians());
        config.vertical_fov = (vertical * 2.0 + 5.0_f32.to_radians())
            .clamp(10.0_f32.to_radians(), 175.0_f32.to_radians());
        config
    }
}
