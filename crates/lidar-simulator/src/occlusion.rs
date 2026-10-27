//! Двусторонние пересечения луча с геометрией. Пробелы между точками облака
//! не становятся отверстиями для лучей; листья BVH содержат до 8 треугольников.
use bevy::prelude::*;

pub(crate) struct Occlusion {
    min: Vec3,
    max: Vec3,
    triangles: Vec<[Vec3; 3]>,
    children: Option<Box<[Self; 2]>>,
}

impl Occlusion {
    pub fn new(mut triangles: Vec<[Vec3; 3]>) -> Self {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for triangle in &triangles {
            for &p in triangle {
                min = min.min(p);
                max = max.max(p);
            }
        }
        let children = if triangles.len() > 8 {
            let extent = max - min;
            let axis = if extent.x >= extent.y && extent.x >= extent.z {
                0
            } else if extent.y >= extent.z {
                1
            } else {
                2
            };
            triangles.sort_by(|a, b| {
                let ca = a[0][axis] + a[1][axis] + a[2][axis];
                let cb = b[0][axis] + b[1][axis] + b[2][axis];
                ca.total_cmp(&cb)
            });
            let right = triangles.split_off(triangles.len() / 2);
            Some(Box::new([
                Self::new(std::mem::take(&mut triangles)),
                Self::new(right),
            ]))
        } else {
            None
        };
        Self {
            min,
            max,
            triangles,
            children,
        }
    }

    pub fn blocked(&self, origin: Vec3, direction: Vec3, limit: f32) -> bool {
        let mut near = 0.0_f32;
        let mut far = limit;
        for axis in 0..3 {
            if direction[axis].abs() < 1e-8 {
                if origin[axis] < self.min[axis] || origin[axis] > self.max[axis] {
                    return false;
                }
            } else {
                let a = (self.min[axis] - origin[axis]) / direction[axis];
                let b = (self.max[axis] - origin[axis]) / direction[axis];
                near = near.max(a.min(b));
                far = far.min(a.max(b));
            }
            if near > far {
                return false;
            }
        }
        if let Some(children) = &self.children {
            return children
                .iter()
                .any(|node| node.blocked(origin, direction, limit));
        }
        self.triangles.iter().any(|&[a, b, c]| {
            let e1 = b - a;
            let e2 = c - a;
            let h = direction.cross(e2);
            let det = e1.dot(h);
            if det.abs() < 1e-8 {
                return false;
            }
            let s = origin - a;
            let u = s.dot(h) / det;
            let q = s.cross(e1);
            let v = direction.dot(q) / det;
            let t = e2.dot(q) / det;
            u >= -1e-6 && v >= -1e-6 && u + v <= 1.0 + 1e-6 && t > 1e-5 && t < limit
        })
    }
}
