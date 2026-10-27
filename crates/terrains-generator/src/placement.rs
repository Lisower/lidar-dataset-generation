//! Placement of circular object footprints on the generated 100 x 100 m landscape.
//! The horizontal map is XY; elevation is Z.

use bevy::math::{Vec2, Vec3};
use generation_core::{Occupancy, Seed};
use landscapes_generator::Landscape;
use rand::RngExt;

#[derive(Debug, Clone, Copy)]
pub struct PlacementRules {
    /// Radius of the object's ground footprint, in meters.
    pub radius: f32,
    /// Additional clearance around other objects and water, in meters.
    pub clearance: f32,
    /// Maximum slope in degrees at the footprint (sampled approximation).
    pub max_slope_deg: f32,
}

impl Default for PlacementRules {
    fn default() -> Self {
        Self {
            radius: 1.0,
            clearance: 0.5,
            max_slope_deg: 25.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacedObject {
    pub position: Vec3,
    pub occupancy: Occupancy,
}

pub struct PlacementMap<'a> {
    landscape: &'a Landscape,
    occupied: Vec<Occupancy>,
}

impl<'a> PlacementMap<'a> {
    pub fn new(landscape: &'a Landscape) -> Self {
        Self {
            landscape,
            occupied: Vec::new(),
        }
    }

    /// Reserve the footprint of an already placed object (e.g. a house).
    pub fn reserve(&mut self, footprint: Occupancy) {
        self.occupied.push(footprint);
    }

    pub fn occupied(&self) -> &[Occupancy] {
        &self.occupied
    }

    /// Try to place an object, rejecting overlap, steep ground, water and map edges.
    pub fn try_place(&mut self, center: Vec2, rules: PlacementRules) -> Option<PlacedObject> {
        if !rules.radius.is_finite()
            || rules.radius <= 0.0
            || !rules.clearance.is_finite()
            || rules.clearance < 0.0
            || !rules.max_slope_deg.is_finite()
            || !(0.0..90.0).contains(&rules.max_slope_deg)
            || !center.is_finite()
        {
            return None;
        }

        let hf = &self.landscape.heightfield;
        let safe_radius = rules.radius + rules.clearance;
        let limit = hf.size_m * 0.5 - safe_radius;
        if limit <= 0.0 || center.x.abs() > limit || center.y.abs() > limit {
            return None;
        }

        let proposed = Occupancy::circle(center, rules.radius);
        // Extra clearance, including against existing rectangles; conservative circular bound.
        if self.occupied.iter().any(|existing| {
            existing.intersects(&proposed)
                || existing.center.distance(center) < existing.shape.bounding_radius() + safe_radius
        }) {
            return None;
        }

        // Check wet grid nodes under/near the entire footprint, not just its center.
        // Add one grid step as a conservative margin for interpolation at water edges.
        let wet_radius = safe_radius + hf.step;
        let n = hf.n;
        let min_i = (((center.x - wet_radius + hf.size_m * 0.5) / hf.step).floor() as isize).max(0)
            as usize;
        let max_i = (((center.x + wet_radius + hf.size_m * 0.5) / hf.step).ceil() as isize)
            .min(n as isize - 1) as usize;
        let min_j = (((center.y - wet_radius + hf.size_m * 0.5) / hf.step).floor() as isize).max(0)
            as usize;
        let max_j = (((center.y + wet_radius + hf.size_m * 0.5) / hf.step).ceil() as isize)
            .min(n as isize - 1) as usize;
        for j in min_j..=max_j {
            for i in min_i..=max_i {
                if hf.node_xy(i, j).distance_squared(center) <= wet_radius * wet_radius
                    && self.landscape.water.levels[hf.index(i, j)].is_some()
                {
                    return None;
                }
            }
        }

        // Sample several gradients across the footprint, not only the center.
        let d = hf.step.max(0.1);
        for p in [
            center,
            center + Vec2::new(rules.radius, 0.0),
            center - Vec2::new(rules.radius, 0.0),
            center + Vec2::new(0.0, rules.radius),
            center - Vec2::new(0.0, rules.radius),
        ] {
            let dx = (hf.height_at(p.x + d, p.y) - hf.height_at(p.x - d, p.y)) / (2.0 * d);
            let dy = (hf.height_at(p.x, p.y + d) - hf.height_at(p.x, p.y - d)) / (2.0 * d);
            let slope = dx.hypot(dy).atan().to_degrees();
            if !slope.is_finite() || slope > rules.max_slope_deg {
                return None;
            }
        }

        self.occupied.push(proposed);
        Some(PlacedObject {
            position: Vec3::new(center.x, center.y, hf.height_at(center.x, center.y)),
            occupancy: proposed,
        })
    }

    /// Seeded rejection sampling; returns fewer than `count` if terrain is crowded.
    pub fn scatter(
        &mut self,
        seed: Seed,
        count: usize,
        rules: PlacementRules,
        attempts_per_object: usize,
    ) -> Vec<PlacedObject> {
        let mut rng = seed.with_label("terrain-placement").rng();
        let half = self.landscape.heightfield.size_m * 0.5;
        let mut placed = Vec::with_capacity(count);
        for _ in 0..count {
            for _ in 0..attempts_per_object {
                let p = Vec2::new(rng.random_range(-half..half), rng.random_range(-half..half));
                if let Some(object) = self.try_place(p, rules) {
                    placed.push(object);
                    break;
                }
            }
        }
        placed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use landscapes_generator::{LandscapePreset, resolve};

    fn plain() -> Landscape {
        let config = resolve(LandscapePreset::Plain, Seed::new(42));
        landscapes_generator::generation::landscape::generate(&config)
    }

    #[test]
    fn never_overlaps() {
        let terrain = plain();
        let mut map = PlacementMap::new(&terrain);
        let objects = map.scatter(Seed::new(1), 100, PlacementRules::default(), 100);
        assert!(!objects.is_empty());
        for (i, a) in objects.iter().enumerate() {
            for b in &objects[..i] {
                assert!(!a.occupancy.intersects(&b.occupancy));
            }
        }
    }

    #[test]
    fn repeatable_for_same_seed() {
        let terrain = plain();
        let rules = PlacementRules::default();
        let a = PlacementMap::new(&terrain).scatter(Seed::new(9), 20, rules, 100);
        let b = PlacementMap::new(&terrain).scatter(Seed::new(9), 20, rules, 100);
        assert_eq!(a, b);
    }

    #[test]
    fn rejects_map_edges() {
        let terrain = plain();
        let mut map = PlacementMap::new(&terrain);
        assert!(
            map.try_place(Vec2::new(50.0, 0.0), PlacementRules::default())
                .is_none()
        );
    }
}
