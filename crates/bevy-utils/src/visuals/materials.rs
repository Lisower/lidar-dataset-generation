//! Утилиты материалов (PBR).

use bevy::color::Color;
use bevy::pbr::StandardMaterial;
use bevy::prelude::default;

/// Обычный непрозрачный материал с заданным цветом и шероховатостью.
pub fn material(color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.85,
        ..default()
    }
}

/// Двусторонний материал — для тонких поверхностей (крыши-скаты, плинтусы),
/// у которых нормали могут смотреть «внутрь» под произвольным углом обзора.
pub fn material_double_sided(color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.8,
        cull_mode: None,
        ..default()
    }
}
