//! Reusable grass geometry produced by the existing generate_tuft implementation.
//! Instance transforms are shared by headless cloud export and the Bevy renderer.
use super::{
    config::PlantConfig,
    grass::{GrassTuft, generate_tuft},
};
use bevy::math::{Mat4, Quat, Vec3};
use generation_core::{PointCloud, Seed};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GrassInstance {
    pub variant: usize,
    /// World position in generator coordinates (Z up).
    pub position: Vec3,
    pub rotation: f32,
    pub scale: f32,
}
impl GrassInstance {
    pub fn matrix(self) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            Vec3::splat(self.scale),
            Quat::from_rotation_z(self.rotation),
            self.position,
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct GrassField {
    pub variants: Vec<GrassTuft>,
    pub instances: Vec<GrassInstance>,
}
impl GrassField {
    pub fn len(&self) -> usize {
        self.instances.len()
    }
    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }
    /// Geometry cost depends on the variant count, never the instance count.
    pub fn variants(configs: &[PlantConfig], per_preset: usize, seed: Seed) -> Self {
        let variants = configs
            .iter()
            .enumerate()
            .flat_map(|(kind, config)| {
                (0..per_preset).map(move |variant| {
                    generate_tuft(config, seed.child(kind as u64).child(variant as u64))
                })
            })
            .collect();
        Self {
            variants,
            instances: Vec::new(),
        }
    }
    pub fn point_cloud(&self) -> PointCloud {
        let mut cloud = PointCloud::default();
        for instance in &self.instances {
            let mut points = self.variants[instance.variant].point_cloud.clone();
            points.transform_by(instance.matrix());
            cloud.merge(&points);
        }
        cloud
    }
}
