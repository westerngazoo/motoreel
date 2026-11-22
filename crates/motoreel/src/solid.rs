//! Faceted 3D (R-0018, SPEC-0018 §2.2–2.5): posed meshes become a batch of
//! [`Surface`]s in painter's order.
//!
//! Per object: every vertex rides the object's composed motor into view
//! space, a face with any vertex the projection culls is dropped whole,
//! back faces and edge-on faces are dropped, and the survivors become
//! surfaces. Then the painter's order, in two levels:
//!
//! 1. **Objects are ordered as wholes.** Two objects whose image boxes
//!    overlap are ordered by a separating plane: a face plane of either
//!    with one object on each side; the one on the eye's side is nearer
//!    (the classic test of Newell, Newell & Sancha, 1972, and the
//!    separating-axis theorem for convex solids). Objects with no such
//!    plane (interpenetrating parts at a joint, a bone inside its limb, an
//!    ink hull around its part) are ordered by the mean depth of their
//!    visible faces: the nearer one is drawn later. A deterministic topological sort turns the pairwise
//!    orders into one, breaking any cycle at its farthest object.
//! 2. **Faces within an object** are sorted far to near by their maximum
//!    view depth, then their minimum, then their index.
//!
//! Drawing whole objects in order is what removes the zigzag a single
//! face-level sort leaves where two solids meet, and what keeps a large
//! face of one box from sorting over a small box resting on it. Every
//! comparison is `f64::total_cmp`, so ties never depend on sort stability
//! or on threads.

use garust::Motor3;

use crate::camera::Projection;
use crate::finish::{Facing, Surface};
use crate::mesh::{dot, face_normal, Mesh, V3};
use crate::prim::Pt2;
use crate::shapes::polygon;

/// A separating plane may pass this close (view units) to a vertex of the
/// object on its far side and still separate: touching boxes share a face
/// plane, computed with rounding on each side.
const SEPARATION_TOL: f64 = 1e-9;

/// One mesh object at one instant.
pub(crate) struct Posed<'a> {
    /// The object's index in the scene: the high half of its surfaces' ids.
    pub(crate) index: usize,
    /// The geometry.
    pub(crate) mesh: &'a Mesh,
    /// Model to view: the camera's inverse composed with the pose.
    pub(crate) to_view: Motor3,
}

/// A visible face before ordering.
struct Visible {
    index: usize,
    near: f64,
    far: f64,
    surface: Surface,
}

/// One object's visible faces and what the object-level order needs.
struct Node {
    index: usize,
    view: Vec<V3>,
    /// Every face's plane (normal, a point on it), visible or not.
    planes: Vec<(V3, V3)>,
    faces: Vec<Visible>,
    near: f64,
    far: f64,
    /// The mean of the visible faces' mid depths: the fallback key.
    mid: f64,
    lo: Pt2,
    hi: Pt2,
}

/// The surfaces of `group`, far to near (SPEC-0018 §2.4). Surface ids are
/// `(object index << 32) | face index`.
pub(crate) fn surfaces(group: &[Posed], projection: Projection) -> Vec<Surface> {
    let nodes: Vec<Node> = group
        .iter()
        .filter_map(|posed| node(posed, projection))
        .collect();
    order(&nodes, projection)
        .into_iter()
        .flat_map(|k| nodes[k].faces.iter().map(|f| f.surface.clone()))
        .collect()
}

/// One object's visible faces, sorted far to near; `None` if none survive.
fn node(posed: &Posed, projection: Projection) -> Option<Node> {
    let view: Vec<V3> = posed
        .mesh
        .vertices()
        .iter()
        .map(|p| {
            let (x, y, z) = p.transform(&posed.to_view).to_euclidean();
            [x, y, z]
        })
        .collect();
    // `project` on the transformed point, as `Scene::eval` does for every
    // other shape: a camera-facing face is then bit-equal to a planar path.
    let image: Vec<Option<Pt2>> = posed
        .mesh
        .vertices()
        .iter()
        .map(|p| projection.project(&p.transform(&posed.to_view)))
        .collect::<Vec<_>>();
    let mut planes = Vec::with_capacity(posed.mesh.faces().len());
    let mut faces = Vec::new();
    for (fi, face) in posed.mesh.faces().iter().enumerate() {
        let v: Vec<V3> = face.indices.iter().map(|&k| view[k]).collect();
        let n = face_normal(v[0], v[1], v[2]);
        planes.push((n, v[0]));
        let Some(pts) = face
            .indices
            .iter()
            .map(|&k| image[k])
            .collect::<Option<Vec<Pt2>>>()
        else {
            continue; // a vertex fails to project: culled whole (R-0002)
        };
        if !front_facing(n, v[0], projection) {
            continue;
        }
        let depths = v.iter().map(|p| -p[2]);
        let far = depths.clone().fold(f64::NEG_INFINITY, f64::max);
        let near = depths.fold(f64::INFINITY, f64::min);
        let outline = polygon(&pts);
        let id = ((posed.index as u64) << 32) | fi as u64;
        let material = posed.mesh.materials()[face.material];
        let Ok(surface) = Surface::new(outline, Facing::Normal(n), far, id, material)
            .and_then(|s| s.with_frame([pts[0], pts[1], pts[2]]))
        else {
            continue; // collapsed in the image: nothing to paint
        };
        faces.push(Visible {
            index: fi,
            near,
            far,
            surface,
        });
    }
    if faces.is_empty() {
        return None;
    }
    faces.sort_by(|a, b| {
        b.far
            .total_cmp(&a.far)
            .then(b.near.total_cmp(&a.near))
            .then(a.index.cmp(&b.index))
    });
    let far = faces
        .iter()
        .map(|f| f.far)
        .fold(f64::NEG_INFINITY, f64::max);
    let near = faces.iter().map(|f| f.near).fold(f64::INFINITY, f64::min);
    let mid = faces.iter().map(|f| (f.near + f.far) / 2.0).sum::<f64>() / faces.len() as f64;
    let (mut lo, mut hi) = (
        Pt2 {
            x: f64::INFINITY,
            y: f64::INFINITY,
        },
        Pt2 {
            x: f64::NEG_INFINITY,
            y: f64::NEG_INFINITY,
        },
    );
    for f in &faces {
        for s in f.surface.outline() {
            for p in std::iter::once(s.start).chain(s.segs.iter().map(|seg| match *seg {
                crate::path::Seg::Line(p) | crate::path::Seg::Cubic(_, _, p) => p,
            })) {
                lo = Pt2 {
                    x: lo.x.min(p.x),
                    y: lo.y.min(p.y),
                };
                hi = Pt2 {
                    x: hi.x.max(p.x),
                    y: hi.y.max(p.y),
                };
            }
        }
    }
    Some(Node {
        index: posed.index,
        view,
        planes,
        faces,
        near,
        far,
        mid,
        lo,
        hi,
    })
}

/// Whether a face with outward normal `n` through view point `p` faces
/// the eye (R-0018 AC3). Edge-on is not facing.
fn front_facing(n: V3, p: V3, projection: Projection) -> bool {
    match projection {
        // The eye looks along −z from +∞.
        Projection::Orthographic => n[2] > 0.0,
        // The eye is the view origin: facing when (p − eye) · n < 0.
        Projection::Pinhole { .. } => dot(p, n) < 0.0,
    }
}

/// The painter's order of `nodes`, as indices (SPEC-0018 §2.4).
fn order(nodes: &[Node], projection: Projection) -> Vec<usize> {
    let n = nodes.len();
    // `before[a]` lists the nodes `a` must be drawn before.
    let mut before: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut pending = vec![0_usize; n];
    for a in 0..n {
        for b in a + 1..n {
            if !overlap(&nodes[a], &nodes[b]) {
                continue;
            }
            let (first, then) = if draws_first(&nodes[a], &nodes[b], projection) {
                (a, b)
            } else {
                (b, a)
            };
            before[first].push(then);
            pending[then] += 1;
        }
    }
    let mut done = vec![false; n];
    let mut out = Vec::with_capacity(n);
    while out.len() < n {
        // Among the nodes nothing waits on, the farthest; if a cycle leaves
        // none, the farthest of all that remain (its constraints dropped).
        let pick = |free: bool| {
            (0..n)
                .filter(|&k| !done[k] && (!free || pending[k] == 0))
                .min_by(|&x, &y| farther_first(&nodes[x], &nodes[y]))
        };
        let Some(k) = pick(true).or_else(|| pick(false)) else {
            break;
        };
        done[k] = true;
        out.push(k);
        for &m in &before[k] {
            pending[m] = pending[m].saturating_sub(1);
        }
    }
    out
}

/// The priority among free nodes: farthest first, then the one whose
/// nearest point is farther, then the lower object index.
fn farther_first(a: &Node, b: &Node) -> std::cmp::Ordering {
    b.far
        .total_cmp(&a.far)
        .then(b.near.total_cmp(&a.near))
        .then(a.index.cmp(&b.index))
}

/// Whether two nodes' image boxes touch or overlap.
fn overlap(a: &Node, b: &Node) -> bool {
    a.lo.x <= b.hi.x && b.lo.x <= a.hi.x && a.lo.y <= b.hi.y && b.lo.y <= a.hi.y
}

/// Whether `a` must be drawn before `b` (SPEC-0018 §2.4).
fn draws_first(a: &Node, b: &Node, projection: Projection) -> bool {
    let planes = a.planes.iter().chain(&b.planes);
    for &(n, p0) in planes {
        if let Some(a_first) = separated(n, p0, a, b, projection) {
            return a_first;
        }
    }
    // No separating face plane: interpenetrating, enclosed, or separated
    // only edge to edge. The one whose visible faces lie nearer on average
    // goes on top. Not the nearest point: an inverted hull's far-side
    // bottom face reaches as near as the part it outlines, and would be
    // drawn over it (SPEC-0018 §2.4).
    b.mid
        .total_cmp(&a.mid)
        .then(b.near.total_cmp(&a.near))
        .then(b.far.total_cmp(&a.far))
        .then(a.index.cmp(&b.index))
        .is_lt()
}

/// If the plane `n · (x − p0) = 0` has `a` on one side and `b` on the
/// other, whether `a` is drawn first (it is on the side away from the eye);
/// `None` if the plane does not separate them or the eye lies on it.
fn separated(n: V3, p0: V3, a: &Node, b: &Node, projection: Projection) -> Option<bool> {
    let len = dot(n, n).sqrt();
    if !(len.is_finite() && len > 0.0) {
        return None;
    }
    let side = |v: &V3| dot(n, [v[0] - p0[0], v[1] - p0[1], v[2] - p0[2]]) / len;
    let range = |vs: &[V3]| {
        vs.iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
                let s = side(v);
                (lo.min(s), hi.max(s))
            })
    };
    let (a_lo, a_hi) = range(&a.view);
    let (b_lo, b_hi) = range(&b.view);
    let a_behind = if a_hi <= SEPARATION_TOL && b_lo >= -SEPARATION_TOL {
        true // a on the negative side, b on the positive
    } else if a_lo >= -SEPARATION_TOL && b_hi <= SEPARATION_TOL {
        false
    } else {
        return None;
    };
    // The eye's side: positive means the eye sees the positive side.
    let eye = match projection {
        Projection::Orthographic => n[2],
        Projection::Pinhole { .. } => side(&[0.0, 0.0, 0.0]),
    };
    if eye.abs() <= SEPARATION_TOL || eye.is_nan() {
        return None;
    }
    // The object on the eye's side is nearer and is drawn second.
    Some((eye > 0.0) == a_behind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::finish::Material;
    use crate::prim::Rgb;
    use garust::pga;

    fn cube_at(x: f64) -> Mesh {
        let m = Material::new(&[Rgb::WHITE]).expect("valid");
        let c = Mesh::cube(0.5, m).expect("valid");
        let moved = c
            .vertices()
            .iter()
            .map(|p| {
                let (px, py, pz) = p.to_euclidean();
                pga::Point::new(px + x, py, pz)
            })
            .collect();
        Mesh::new(moved, c.faces().to_vec(), c.materials().to_vec()).expect("valid")
    }

    /// Two cubes side by side under a pinhole camera looking down +x from
    /// the right: the right cube is nearer, so it is drawn second.
    #[test]
    fn the_nearer_of_two_separated_cubes_is_drawn_second() {
        let (left, right) = (cube_at(-1.0), cube_at(1.0));
        // Camera at +x looking along −x: turn about +y by +90°, then back.
        let s = std::f64::consts::FRAC_1_SQRT_2;
        let turn = Motor3::from_unit_quaternion(s, 0.0, s, 0.0);
        let pose = Motor3::translator(5.0, 0.0, 0.0).compose(&turn);
        let to_view = pose.inverse();
        let group = [
            Posed {
                index: 0,
                mesh: &right,
                to_view,
            },
            Posed {
                index: 1,
                mesh: &left,
                to_view,
            },
        ];
        let batch = surfaces(&group, Projection::Pinhole { focal: 1.0 });
        let first_object = batch.first().map(|s| s.id() >> 32);
        let last_object = batch.last().map(|s| s.id() >> 32);
        assert_eq!((first_object, last_object), (Some(1), Some(0)));
    }
}
