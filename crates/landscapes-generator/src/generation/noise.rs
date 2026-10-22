//! Сэмплер детерминированного fBm-шума для каменистости грунта.

use bevy::math::Vec2;
use generation_core::Seed;
use noise::{Fbm, MultiFractal, NoiseFn, Perlin};

use super::config::TerrainNoise;

/// Сэмплер fBm, собранный один раз (строить [`Fbm`] на каждую точку дорого).
pub struct NoiseSampler {
    fbm: Fbm<Perlin>,
    amplitude: f32,
}

impl NoiseSampler {
    /// Собрать сэмплер из параметров шума и seed.
    pub fn new(cfg: &TerrainNoise, seed: Seed) -> Self {
        let s = (seed.value() & 0xFFFF_FFFF) as u32;
        let fbm = Fbm::<Perlin>::new(s)
            .set_octaves(cfg.octaves.max(1))
            .set_frequency(f64::from(cfg.frequency))
            .set_lacunarity(f64::from(cfg.lacunarity))
            .set_persistence(f64::from(cfg.persistence));
        Self {
            fbm,
            amplitude: cfg.amplitude,
        }
    }

    /// Шум в метрах высоты (диапазон примерно `±amplitude`).
    pub fn sample(&self, p: Vec2) -> f32 {
        let v = self.fbm.get([f64::from(p.x), f64::from(p.y)]);
        v as f32 * self.amplitude
    }
}
