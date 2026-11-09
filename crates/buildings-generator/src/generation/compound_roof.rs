//! Roof sections meet at the footprint joint without hidden end walls.
use super::super::roof::roof_faces;
use super::super::{Building, BuildingConfig, Part};
use bevy::math::Vec3;
use bevy::transform::components::Transform;
use generation_core::geometry::Polygon;

const EPS: f32 = 1e-4;

#[derive(Clone, Copy)]
pub(super) struct Section {
    x0: f32,
    x1: f32,
    y0: f32,
    y1: f32,
    height: f32,
}

impl Section {
    pub(super) fn new(ax: f32, bx: f32, ay: f32, by: f32, height: f32) -> Self {
        Self {
            x0: ax.min(bx),
            x1: ax.max(bx),
            y0: ay.min(by),
            y1: ay.max(by),
            height,
        }
    }

    fn faces(self, b: &Building, config: &BuildingConfig) -> Vec<Polygon> {
        let dx = self.x1 - self.x0;
        let dy = self.y1 - self.y0;
        // Align the ridge with the longer side of each volume.
        let swap = dy > dx;
        let (length, width) = if swap { (dy, dx) } else { (dx, dy) };
        roof_faces(b.roof, length, width, 0.0, b.roof_pitch_deg, config, 0.0)
            .iter()
            // These sections have no overhang. The standard fascia boards
            // extend below the eaves and coincide with the facade surface,
            // causing z-fighting along the whole wall/roof junction.
            .filter(|face| !b.roof.is_pitched() || face.points().iter().any(|p| p.z > EPS))
            .filter_map(|face| {
                clean(
                    face.points()
                        .iter()
                        .map(|p| {
                            let (x, y) = if swap { (-p.y, p.x) } else { (p.x, p.y) };
                            Vec3::new(
                                x + (self.x0 + self.x1) * 0.5,
                                y + (self.y0 + self.y1) * 0.5,
                                p.z + self.height,
                            )
                        })
                        .collect(),
                )
            })
            .collect()
    }
}

// A half-space is n.dot(point) <= distance. Roof triangles extruded down
// to the ground form convex solids, even for the non-convex Dutch roof.
type Plane = (Vec3, f32);

fn solids(section: Section, faces: &[Polygon]) -> Vec<Vec<Plane>> {
    let mut out = vec![vec![
        (Vec3::Y, section.y1),
        (-Vec3::Y, -section.y0),
        (Vec3::Z, section.height),
    ]];
    for face in faces.iter().filter(|f| f.normal().z > EPS) {
        let pts = face.points();
        for i in 1..pts.len() - 1 {
            let tri = [pts[0], pts[i], pts[i + 1]];
            let normal = (tri[1] - tri[0]).cross(tri[2] - tri[0]).normalize_or_zero();
            if normal.z <= EPS {
                continue;
            }
            let mut planes = vec![(normal, normal.dot(tri[0]))];
            for edge in 0..3 {
                let a = tri[edge];
                let v = tri[(edge + 1) % 3] - a;
                let n = Vec3::new(v.y, -v.x, 0.0).normalize();
                planes.push((n, n.dot(a)));
            }
            out.push(planes);
        }
    }
    out
}

pub(super) fn build(
    main: Section,
    wing: Section,
    joint: f32,
    b: &Building,
    config: &BuildingConfig,
) -> Part {
    let main_faces = main.faces(b, config);
    let wing_faces = wing.faces(b, config);
    let main_solids = solids(main, &main_faces);
    let wing_solids = solids(wing, &wing_faces);
    let mut walls = Vec::new();
    let mut group = Part::group("compound-roof", b.colors.roof, Transform::IDENTITY);
    for (name, section, faces, other) in [
        ("main-roof", main, main_faces, &wing_solids),
        ("wing-roof", wing, wing_faces, &main_solids),
    ] {
        let mut roofing = Vec::new();
        for face in faces {
            let vertical = face.normal().z.abs() < EPS;
            let gable = vertical
                && b.roof.is_pitched()
                && face.points().iter().any(|p| p.z > section.height + EPS);
            let on_joint = face.points().iter().all(|p| (p.x - joint).abs() < EPS);
            let fragments = if on_joint {
                exposed(face, other)
            } else {
                vec![face]
            };
            if gable {
                walls.extend(fragments);
            } else {
                roofing.extend(fragments);
            }
        }
        group = group.child(Part::from_faces(
            name,
            roofing,
            b.colors.roof,
            Transform::IDENTITY,
            true,
            config,
        ));
    }
    if wing.height < main.height {
        let outward = if wing.x0 >= joint - EPS {
            Vec3::X
        } else {
            -Vec3::X
        };
        let wall = Polygon::quad(
            Vec3::new(joint, wing.y0, wing.height),
            Vec3::new(joint, wing.y1, wing.height),
            Vec3::new(joint, wing.y1, main.height),
            Vec3::new(joint, wing.y0, main.height),
        )
        .oriented(outward);
        walls.extend(exposed(wall, &wing_solids));
    }
    group.child(Part::from_faces(
        "compound-gables",
        walls,
        b.colors.wall,
        Transform::IDENTITY,
        true,
        config,
    ))
}

fn exposed(face: Polygon, solids: &[Vec<Plane>]) -> Vec<Polygon> {
    let mut remaining = vec![face];
    for solid in solids {
        let mut next = Vec::new();
        for face in remaining {
            let mut inside = Some(face);
            for &(n, d) in solid {
                let Some(fragment) = inside.take() else {
                    break;
                };
                if fragment.points().iter().any(|p| n.dot(*p) - d > EPS) {
                    if let Some(outside) = clip(&fragment, -n, -d) {
                        next.push(outside);
                    }
                }
                inside = clip(&fragment, n, d);
            }
            // The part inside every half-space is hidden by the other volume.
        }
        remaining = next;
    }
    remaining
}

fn clip(face: &Polygon, n: Vec3, d: f32) -> Option<Polygon> {
    let pts = face.points();
    let mut out = Vec::new();
    for i in 0..pts.len() {
        let a = pts[i];
        let b = pts[(i + 1) % pts.len()];
        let da = n.dot(a) - d;
        let db = n.dot(b) - d;
        if da <= EPS {
            out.push(a);
        }
        if (da <= EPS && db > EPS) || (da > EPS && db <= EPS) {
            out.push(a.lerp(b, (da / (da - db)).clamp(0.0, 1.0)));
        }
    }
    clean(out)
}

fn clean(points: Vec<Vec3>) -> Option<Polygon> {
    let mut out = Vec::new();
    for p in points {
        if out.last().is_none_or(|q: &Vec3| p.distance(*q) > EPS) {
            out.push(p);
        }
    }
    if out.len() > 1 && out[0].distance(*out.last().unwrap()) < EPS {
        out.pop();
    }
    loop {
        if out.len() < 3 {
            return None;
        }
        let collinear = (0..out.len()).find(|&i| {
            let a = out[(i + out.len() - 1) % out.len()];
            let b = out[i];
            let c = out[(i + 1) % out.len()];
            (b - a).cross(c - b).length() < EPS
        });
        if let Some(i) = collinear {
            out.remove(i);
        } else {
            break;
        }
    }
    Some(Polygon::from_points(out))
}

#[cfg(test)]
mod tests {
    use super::super::super::RoofStyle;
    use super::*;

    #[test]
    fn roofs_without_overhang_do_not_overlap_the_wall_top_band() {
        let config = BuildingConfig::default();
        let mut b = Building::generate(&config);
        let section = Section::new(-4.0, 4.0, -3.0, 3.0, 6.0);
        for style in RoofStyle::ALL {
            b.roof = style;
            b.roof_pitch_deg = if style.is_pitched() { 30.0 } else { 0.0 };
            let faces = section.faces(&b, &config);
            assert!(!faces.is_empty());
            for face in &faces {
                assert!(
                    face.points().iter().all(|p| p.z >= section.height - EPS),
                    "{style:?}: roof overlaps the wall: {:?}",
                    face.points()
                );
            }
            assert!(
                faces
                    .iter()
                    .flat_map(|f| f.points())
                    .any(|p| (p.z - section.height).abs() < EPS),
                "{style:?}: roof must meet the wall without a gap"
            );
        }
    }

    #[test]
    fn joint_wall_covers_the_area_above_the_wing_roof() {
        let config = BuildingConfig {
            point_step: 0.5,
            point_jitter: 0.0,
            ..Default::default()
        };
        let mut b = Building::generate(&config);
        b.roof = RoofStyle::Gable;
        b.roof_pitch_deg = 28.366875;

        // Same footprint as the annex preset: the wing is longer along X,
        // so its gable end faces the main volume at the joint.
        let main = Section::new(-1.8, 7.5, -5.5870824, 5.5870824, 6.1112523);
        let wing = Section::new(-7.5, -1.8, -5.5870824, 0.0, 1.986157);
        let joint = -1.8;
        let roof = build(main, wing, joint, &b, &config);
        let gables = roof
            .children
            .iter()
            .find(|part| part.name == "compound-gables")
            .expect("compound-gables");

        // A point above the far roof slope but below the near slope's
        // extension used to fall into a triangular hole.
        let p = Vec3::new(joint, -1.0, 3.5);
        let covered = gables.faces.iter().any(|face| {
            face.points().iter().all(|q| (q.x - joint).abs() < EPS)
                && face.normal().x.abs() > 0.9
                && point_in_convex(face.points(), p)
        });
        assert!(covered, "joint wall has a hole at {p:?}");
    }

    fn point_in_convex(poly: &[Vec3], p: Vec3) -> bool {
        let mut sign = 0.0_f32;
        for i in 0..poly.len() {
            let a = poly[i];
            let b = poly[(i + 1) % poly.len()];
            let cross = (b.y - a.y) * (p.z - a.z) - (b.z - a.z) * (p.y - a.y);
            if cross.abs() < 1e-6 {
                continue;
            }
            if sign == 0.0 {
                sign = cross.signum();
            } else if cross.signum() != sign {
                return false;
            }
        }
        true
    }

    #[test]
    fn shared_end_walls_are_trimmed_to_the_other_roof_profile() {
        let config = BuildingConfig {
            point_step: 0.5,
            point_jitter: 0.0,
            ..Default::default()
        };
        let mut b = Building::generate(&config);
        for style in RoofStyle::ALL {
            b.roof = style;
            b.roof_pitch_deg = if style.is_pitched() { 30.0 } else { 0.0 };
            for height in [3.0, 7.0] {
                for sx in [-1.0, 1.0] {
                    for sy in [-1.0, 1.0] {
                        let main = Section::new(-6.0 * sx, 1.5 * sx, -4.0, 4.0, height);
                        let wing = Section::new(1.5 * sx, 6.0 * sx, -4.0 * sy, 0.0, 3.0);
                        let joint = 1.5 * sx;
                        for (section, other) in [(main, wing), (wing, main)] {
                            let other_faces = other.faces(&b, &config);
                            let occluders = solids(other, &other_faces);
                            for face in section
                                .faces(&b, &config)
                                .into_iter()
                                .filter(|f| f.points().iter().all(|p| (p.x - joint).abs() < EPS))
                            {
                                for fragment in exposed(face, &occluders) {
                                    let center = fragment.points().iter().copied().sum::<Vec3>()
                                        / fragment.len() as f32;
                                    if center.y < other.y0 + EPS || center.y > other.y1 - EPS {
                                        continue;
                                    }
                                    let top =
                                        surface_height(&other_faces, center).max(other.height);
                                    assert!(
                                        center.z >= top - EPS,
                                        "{style:?}: hidden end wall at {center:?}, roof={top}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Independent vertical ray through the original roof mesh.
    fn surface_height(faces: &[Polygon], p: Vec3) -> f32 {
        let mut height = f32::NEG_INFINITY;
        for face in faces.iter().filter(|f| f.normal().z > EPS) {
            let pts = face.points();
            for i in 1..pts.len() - 1 {
                let a = pts[0];
                let u = (pts[i] - a).truncate();
                let v = (pts[i + 1] - a).truncate();
                let q = (p - a).truncate();
                let det = u.perp_dot(v);
                if det.abs() < EPS {
                    continue;
                }
                let s = q.perp_dot(v) / det;
                let t = u.perp_dot(q) / det;
                if s >= -EPS && t >= -EPS && s + t <= 1.0 + EPS {
                    height = height.max(a.z + s * (pts[i].z - a.z) + t * (pts[i + 1].z - a.z));
                }
            }
        }
        height
    }
}
