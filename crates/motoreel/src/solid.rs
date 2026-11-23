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
//!    visible faces: the nearer one is drawn later. A deterministic
//!    topological sort turns the pairwise orders into one, breaking any
//!    cycle at its farthest object.
//! 2. **Faces within an object** are sorted far to near by their maximum
//!    view depth, then their minimum, then their index.
//!
//! Drawing whole objects in order is what removes the zigzag a single
//! face-level sort leaves where two solids meet, and what keeps a large
//! face of one box from sorting over a small box resting on it. Every
//! comparison is `f64::total_cmp`, so ties never depend on sort stability
//! or on threads.
//!
//! **Speed without a new answer (R-0022, SPEC-0022 §2).** The pairwise
//! test asks, plane by plane in a fixed order, which side of the plane
//! each object is on. What each object's own vertices say about each of
//! its own planes is worked out once per frame, not once per pair; the
//! other object's vertices are then read only as far as the first one that
//! settles the question. Every side is the same expression on the same
//! numbers as before, and the first separating plane in the same order
//! still decides, so the order is the one R-0018 and R-0021 defined, bit
//! for bit (the oracle is `solid_reference.rs`).

use garust::Motor3;

use crate::camera::Projection;
use crate::finish::{Facing, Surface};
use crate::mesh::{dot, face_normal, Mesh, V3};
use crate::prim::Pt2;
use crate::sandwich::Sandwich;
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

/// A face plane in view space, with what every pairwise test needs of it
/// worked out once per frame.
struct Plane {
    /// The face normal, unnormalised.
    n: V3,
    /// The face's first vertex.
    p0: V3,
    /// `|n|`.
    len: f64,
    /// `len` is finite and positive: a plane either test may use.
    valid: bool,
    /// Whether the eye sees the plane's positive side; `None` when the
    /// plane cannot separate (invalid, or the eye lies on it).
    eye: Option<bool>,
}

impl Plane {
    fn new(n: V3, p0: V3, projection: Projection) -> Plane {
        let len = dot(n, n).sqrt();
        let valid = len.is_finite() && len > 0.0;
        let mut plane = Plane {
            n,
            p0,
            len,
            valid,
            eye: None,
        };
        if valid {
            // The eye's side: positive means the eye sees the positive side.
            let eye = match projection {
                Projection::Orthographic => n[2],
                Projection::Pinhole { .. } => plane.side(&[0.0, 0.0, 0.0]),
            };
            if !(eye.abs() <= SEPARATION_TOL || eye.is_nan()) {
                plane.eye = Some(eye > 0.0);
            }
        }
        plane
    }

    /// The signed distance of `v` from the plane, in view units.
    fn side(&self, v: &V3) -> f64 {
        dot(
            self.n,
            [v[0] - self.p0[0], v[1] - self.p0[1], v[2] - self.p0[2]],
        ) / self.len
    }
}

/// Which sides of a plane a set of vertices keeps to, within
/// [`SEPARATION_TOL`]: `neg` when none is farther than it on the positive
/// side, `pos` when none is on the negative side. A vertex whose side is
/// NaN breaks neither, as the range fold it replaces ignored NaN.
#[derive(Clone, Copy)]
struct Sides {
    neg: bool,
    pos: bool,
}

impl Sides {
    /// The sides `vertices` keep to, read only as far as needed: a side
    /// not asked for (`want`) comes back `false`, and the scan stops once
    /// every side asked for is broken.
    fn of(plane: &Plane, vertices: &[V3], want: Sides) -> Sides {
        let mut sides = want;
        for v in vertices {
            if !(sides.neg || sides.pos) {
                break;
            }
            let s = plane.side(v);
            if s > SEPARATION_TOL {
                sides.neg = false;
            }
            if s < -SEPARATION_TOL {
                sides.pos = false;
            }
        }
        sides
    }
}

/// One object's visible faces and what the object-level order needs.
struct Node {
    index: usize,
    view: Vec<V3>,
    /// Every face's plane, visible or not.
    planes: Vec<Plane>,
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
    let mut nodes: Vec<Node> = group
        .iter()
        .filter_map(|posed| node(posed, projection))
        .collect();
    let order = order(&nodes);
    let mut out = Vec::with_capacity(nodes.iter().map(|n| n.faces.len()).sum());
    for k in order {
        out.extend(
            std::mem::take(&mut nodes[k].faces)
                .into_iter()
                .map(|f| f.surface),
        );
    }
    out
}

/// One object's visible faces, sorted far to near; `None` if none survive.
fn node(posed: &Posed, projection: Projection) -> Option<Node> {
    let vertices = posed.mesh.vertices();
    let to_view = Sandwich::new(&posed.to_view);
    let mut view = Vec::with_capacity(vertices.len());
    let mut image = Vec::with_capacity(vertices.len());
    for p in vertices {
        // One sandwich serves view and image: `project` is
        // `project_euclidean` of the moved point, and `Sandwich` is
        // `transform` bit for bit, so a camera-facing face is still
        // bit-equal to the planar path `Scene::eval` projects.
        let e = to_view.apply(p);
        view.push([e.0, e.1, e.2]);
        image.push(projection.project_euclidean(e));
    }
    let mut planes = Vec::with_capacity(posed.mesh.faces().len());
    let mut faces = Vec::with_capacity(posed.mesh.faces().len());
    let mut pts: Vec<Pt2> = Vec::new();
    for (fi, face) in posed.mesh.faces().iter().enumerate() {
        let at = |k: usize| view[face.indices[k]];
        let n = face_normal(at(0), at(1), at(2));
        planes.push(Plane::new(n, at(0), projection));
        pts.clear();
        let projects = face.indices.iter().all(|&k| match image[k] {
            Some(p) => {
                pts.push(p);
                true
            }
            None => false,
        });
        if !projects {
            continue; // a vertex fails to project: culled whole (R-0002)
        }
        if !front_facing(n, at(0), projection) {
            continue;
        }
        let depths = face.indices.iter().map(|&k| -view[k][2]);
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

/// What each node's own vertices say about each of its own planes,
/// worked out the first time a pair asks and kept for the frame
/// (SPEC-0022 §2.2).
struct OwnSides {
    /// Where node `k`'s planes start in `sides`.
    start: Vec<usize>,
    sides: Vec<Option<Sides>>,
}

impl OwnSides {
    fn new(nodes: &[Node]) -> OwnSides {
        let mut start = Vec::with_capacity(nodes.len());
        let mut total = 0;
        for node in nodes {
            start.push(total);
            total += node.planes.len();
        }
        OwnSides {
            start,
            sides: vec![None; total],
        }
    }

    /// The sides node `k`'s vertices keep to of its plane `p`.
    fn get(&mut self, nodes: &[Node], k: usize, p: usize) -> Sides {
        let slot = &mut self.sides[self.start[k] + p];
        *slot.get_or_insert_with(|| {
            let all = Sides {
                neg: true,
                pos: true,
            };
            Sides::of(&nodes[k].planes[p], &nodes[k].view, all)
        })
    }
}

/// The painter's order of `nodes`, as indices (SPEC-0018 §2.4).
fn order(nodes: &[Node]) -> Vec<usize> {
    let n = nodes.len();
    let mut own = OwnSides::new(nodes);
    // `before[a]` lists the nodes `a` must be drawn before.
    // with whether the order comes from an enclosure.
    let mut before: Vec<Vec<(usize, bool)>> = vec![Vec::new(); n];
    let mut pending = vec![0_usize; n];
    let mut pending_enclosed = vec![0_usize; n];
    for a in 0..n {
        for b in a + 1..n {
            if !overlap(&nodes[a], &nodes[b]) {
                continue;
            }
            let (a_first, enclosed) = draws_first(nodes, a, b, &mut own);
            let (first, then) = if a_first { (a, b) } else { (b, a) };
            before[first].push((then, enclosed));
            pending[then] += 1;
            pending_enclosed[then] += usize::from(enclosed);
        }
    }
    let mut done = vec![false; n];
    let mut out = Vec::with_capacity(n);
    while out.len() < n {
        // Among the nodes nothing waits on, the farthest. If a cycle leaves
        // none, the farthest of those not waiting on a part they enclose,
        // so a cycle never draws a shell before its inside (R-0021 AC1);
        // failing that, the farthest of all. A pick drops its pending
        // constraints.
        let farthest = |keep: &dyn Fn(usize) -> bool| {
            (0..n)
                .filter(|&k| !done[k] && keep(k))
                .min_by(|&x, &y| farther_first(&nodes[x], &nodes[y]))
        };
        let Some(k) = farthest(&|k| pending[k] == 0)
            .or_else(|| farthest(&|k| pending_enclosed[k] == 0))
            .or_else(|| farthest(&|_| true))
        else {
            break;
        };
        done[k] = true;
        out.push(k);
        for &(m, enclosed) in &before[k] {
            pending[m] = pending[m].saturating_sub(1);
            pending_enclosed[m] = pending_enclosed[m].saturating_sub(usize::from(enclosed));
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

/// Whether node `a` must be drawn before node `b` (SPEC-0018 §2.4), and
/// whether that order comes from an enclosure, which a cycle never breaks
/// (SPEC-0021 §2.1).
fn draws_first(nodes: &[Node], a: usize, b: usize, own: &mut OwnSides) -> (bool, bool) {
    let (na, nb) = (&nodes[a], &nodes[b]);
    // The face planes of `a`, then those of `b`: the first that separates
    // decides, as it always has.
    for (p, plane) in na.planes.iter().enumerate() {
        if let Some(a_first) = separated(plane, || own.get(nodes, a, p), &nb.view, true) {
            return (a_first, false);
        }
    }
    for (p, plane) in nb.planes.iter().enumerate() {
        if let Some(a_first) = separated(plane, || own.get(nodes, b, p), &na.view, false) {
            return (a_first, false);
        }
    }
    // No separating face plane. A part inside every face plane of the
    // other is enclosed by it (a bone in its muscle in its skin) and goes
    // first, however off-centre it sits (R-0021 AC1).
    if inside(nb, na) {
        return (false, true);
    }
    if inside(na, nb) {
        return (true, true);
    }
    // Interpenetrating, or separated only edge to edge, or an inverted
    // hull around its part. The one whose visible faces lie nearer on average
    // goes on top. Not the nearest point: an inverted hull's far-side
    // bottom face reaches as near as the part it outlines, and would be
    // drawn over it (SPEC-0018 §2.4).
    let a_first = nb
        .mid
        .total_cmp(&na.mid)
        .then(nb.near.total_cmp(&na.near))
        .then(nb.far.total_cmp(&na.far))
        .then(na.index.cmp(&nb.index))
        .is_lt();
    (a_first, false)
}

/// Whether every vertex of `inner` lies on the inner side of every face
/// plane of `outer` (within [`SEPARATION_TOL`]): inside the intersection of
/// `outer`'s half-spaces, so inside `outer` whatever its shape. An
/// inverted hull's planes face inward, so nothing is ever inside one.
fn inside(inner: &Node, outer: &Node) -> bool {
    outer
        .planes
        .iter()
        .all(|plane| plane.valid && inner.view.iter().all(|v| plane.side(v) <= SEPARATION_TOL))
}

/// If `plane`, a face plane of one of two objects, has `a` on one side and
/// `b` on the other, whether `a` is drawn first (it is on the side away
/// from the eye); `None` if the plane does not separate them or the eye
/// lies on it. `own` gives the sides the plane's object keeps to; `other`
/// is the other object's vertices; `a_owns` says which object the plane
/// is a face of.
///
/// The answer is the one the range test of R-0018 gave: `a` behind when
/// all of `a` is on the negative side and all of `b` on the positive one,
/// in front for the reverse. Only the reading is lazier.
fn separated(
    plane: &Plane,
    own: impl FnOnce() -> Sides,
    other: &[V3],
    a_owns: bool,
) -> Option<bool> {
    let eye = plane.eye?;
    let own = own();
    if !(own.neg || own.pos) {
        return None;
    }
    // The other object must keep to the side opposite one the owner keeps
    // to: only those sides are worth reading.
    let want = Sides {
        neg: own.pos,
        pos: own.neg,
    };
    let other = Sides::of(plane, other, want);
    let (a, b) = if a_owns { (own, other) } else { (other, own) };
    let a_behind = if a.neg && b.pos {
        true // a on the negative side, b on the positive
    } else if a.pos && b.neg {
        false
    } else {
        return None;
    };
    // The object on the eye's side is nearer and is drawn second.
    Some(eye == a_behind)
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
