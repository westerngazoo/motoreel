//! The painter's order as R-0018 and R-0021 shipped it, kept verbatim as
//! the oracle of R-0022's differential test: the fast order in
//! [`crate::solid`] must give these surfaces, bit for bit, on every scene
//! (SPEC-0022 §2.5). Test-only; nothing in the crate calls it.
//!
//! The body below is `solid.rs` at `08cbb90` (main after R-0021) with two
//! edits: `Posed` is the one in [`crate::solid`], and the unit tests stay
//! there. Do not optimise it: its only job is to be the old algorithm.

use crate::camera::Projection;
use crate::finish::{Facing, Surface};
use crate::mesh::{dot, face_normal, V3};
use crate::prim::Pt2;
use crate::shapes::polygon;
use crate::solid::Posed;

/// A separating plane may pass this close (view units) to a vertex of the
/// object on its far side and still separate: touching boxes share a face
/// plane, computed with rounding on each side.
const SEPARATION_TOL: f64 = 1e-9;

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
    // with whether the order comes from an enclosure.
    let mut before: Vec<Vec<(usize, bool)>> = vec![Vec::new(); n];
    let mut pending = vec![0_usize; n];
    let mut pending_enclosed = vec![0_usize; n];
    for a in 0..n {
        for b in a + 1..n {
            if !overlap(&nodes[a], &nodes[b]) {
                continue;
            }
            let (a_first, enclosed) = draws_first(&nodes[a], &nodes[b], projection);
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

/// Whether `a` must be drawn before `b` (SPEC-0018 §2.4), and whether
/// that order comes from an enclosure, which a cycle never breaks
/// (SPEC-0021 §2.1).
fn draws_first(a: &Node, b: &Node, projection: Projection) -> (bool, bool) {
    let planes = a.planes.iter().chain(&b.planes);
    for &(n, p0) in planes {
        if let Some(a_first) = separated(n, p0, a, b, projection) {
            return (a_first, false);
        }
    }
    // No separating face plane. A part inside every face plane of the
    // other is enclosed by it (a bone in its muscle in its skin) and goes
    // first, however off-centre it sits (R-0021 AC1).
    if inside(b, a) {
        return (false, true);
    }
    if inside(a, b) {
        return (true, true);
    }
    // Interpenetrating, or separated only edge to edge, or an inverted
    // hull around its part. The one whose visible faces lie nearer on average
    // goes on top. Not the nearest point: an inverted hull's far-side
    // bottom face reaches as near as the part it outlines, and would be
    // drawn over it (SPEC-0018 §2.4).
    let a_first = b
        .mid
        .total_cmp(&a.mid)
        .then(b.near.total_cmp(&a.near))
        .then(b.far.total_cmp(&a.far))
        .then(a.index.cmp(&b.index))
        .is_lt();
    (a_first, false)
}

/// Whether every vertex of `inner` lies on the inner side of every face
/// plane of `outer` (within [`SEPARATION_TOL`]): inside the intersection of
/// `outer`'s half-spaces, so inside `outer` whatever its shape. An
/// inverted hull's planes face inward, so nothing is ever inside one.
fn inside(inner: &Node, outer: &Node) -> bool {
    outer.planes.iter().all(|&(n, p0)| {
        let len = dot(n, n).sqrt();
        len.is_finite()
            && len > 0.0
            && inner
                .view
                .iter()
                .all(|v| dot(n, [v[0] - p0[0], v[1] - p0[1], v[2] - p0[2]]) / len <= SEPARATION_TOL)
    })
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

/// R-0022 AC1: the fast order against this one, on random scenes and on
/// the house's own anatomy, at many camera angles. Equality is of the
/// `Debug` text, which prints every `f64` exactly (`-0.0` included), so it
/// is byte identity, not `PartialEq` (which takes `-0.0` for `0.0`).
mod differential {
    use garust::{pga, Motor3};
    use proptest::prelude::*;

    use crate::camera::Projection;
    use crate::faceted::Faceted;
    use crate::finish::{Finish, FrameCtx, Layer, Light, Material};
    use crate::mesh::{Mesh, MeshFace};
    use crate::obj::{layer_of, Axes, Obj};
    use crate::peel::Peel;
    use crate::prim::Rgb;
    use crate::solid::{self, Posed};
    use crate::xray::{Glass, Glow, XRay};

    /// The fast and the reference order of `group` are the same surfaces,
    /// and paint the same primitives under both house finishes.
    fn same(group: &[Posed], projection: Projection, what: &str) {
        let new = solid::surfaces(group, projection);
        let old = super::surfaces(group, projection);
        assert_eq!(format!("{new:?}"), format!("{old:?}"), "surfaces: {what}");
        let faceted = Faceted::new(3, 2.0).expect("valid");
        let glass = Glass {
            tint: Rgb {
                r: 40,
                g: 200,
                b: 230,
            },
            alpha: 0.1,
            edge_width: 1.6,
            edge_alpha: 0.3,
        };
        let glow = Glow {
            width: 12.0,
            alpha: 0.16,
        };
        let xray = XRay::new(faceted, glass, glow).expect("valid");
        let light = Light::new([0.55, 0.8, 0.45]).expect("valid");
        let ctx = FrameCtx { t: 0.0, seed: 0 };
        for finish in [&faceted as &dyn Finish, &xray] {
            assert_eq!(
                format!("{:?}", finish.paint(&new, &light, &ctx)),
                format!("{:?}", finish.paint(&old, &light, &ctx)),
                "prims: {what}"
            );
        }
    }

    /// splitmix64: a small fixed generator, so a failing seed replays.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^ (z >> 31)
        }

        /// Uniform in `[lo, hi)`.
        fn range(&mut self, lo: f64, hi: f64) -> f64 {
            lo + (hi - lo) * (self.next() >> 11) as f64 / (1_u64 << 53) as f64
        }

        fn pick(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    fn xyz(p: &pga::Point) -> [f64; 3] {
        let (x, y, z) = p.to_euclidean();
        [x, y, z]
    }

    fn ramp() -> Material {
        Material::new(&[
            Rgb {
                r: 20,
                g: 60,
                b: 120,
            },
            Rgb {
                r: 60,
                g: 140,
                b: 200,
            },
            Rgb {
                r: 160,
                g: 220,
                b: 250,
            },
        ])
        .expect("valid")
    }

    fn material(rng: &mut Rng) -> Material {
        let layer = [Layer::Solid, Layer::Skin, Layer::Inner][rng.pick(3) as usize];
        ramp().with_layer(layer)
    }

    /// `mesh` with every vertex moved by `f`, faces and materials kept.
    fn mapped(mesh: &Mesh, f: impl Fn([f64; 3]) -> [f64; 3]) -> Mesh {
        let vertices = mesh
            .vertices()
            .iter()
            .map(|p| {
                let [x, y, z] = f(xyz(p));
                pga::Point::new(x, y, z)
            })
            .collect();
        Mesh::new(vertices, mesh.faces().to_vec(), mesh.materials().to_vec()).expect("valid")
    }

    /// An axis-aligned box from `lo` to `hi`.
    fn boxed(lo: [f64; 3], hi: [f64; 3], m: Material) -> Mesh {
        let cube = Mesh::cube(1.0, m).expect("valid");
        mapped(&cube, |[x, y, z]| {
            let pick = |s: f64, a: f64, b: f64| if s < 0.0 { a } else { b };
            [
                pick(x, lo[0], hi[0]),
                pick(y, lo[1], hi[1]),
                pick(z, lo[2], hi[2]),
            ]
        })
    }

    /// The inverted hull of `mesh`: grown by `grow` about the origin and
    /// wound inside out, like the house's ink outlines.
    fn hull(mesh: &Mesh, grow: f64) -> Mesh {
        let grown = mapped(mesh, |p| p.map(|c| grow * c));
        let faces = grown
            .faces()
            .iter()
            .map(|f| MeshFace::new(f.indices.iter().rev().copied().collect(), f.material))
            .collect();
        Mesh::new(grown.vertices().to_vec(), faces, grown.materials().to_vec()).expect("valid")
    }

    /// Two disjoint boxes in one mesh: an object that is not convex.
    fn pair(rng: &mut Rng, m: Material) -> Mesh {
        let a = boxed([-0.6, -0.2, -0.2], [-0.2, 0.2, 0.2], m);
        let b = boxed([0.2, -0.3, -0.1], [0.7, 0.1, 0.3], m);
        let shift = a.vertices().len();
        let mut vertices = a.vertices().to_vec();
        vertices.extend_from_slice(b.vertices());
        let mut faces = a.faces().to_vec();
        faces.extend(
            b.faces()
                .iter()
                .map(|f| MeshFace::new(f.indices.iter().map(|i| i + shift).collect(), f.material)),
        );
        let s = rng.range(0.6, 1.4);
        mapped(&Mesh::new(vertices, faces, vec![m]).expect("valid"), |p| {
            p.map(|c| s * c)
        })
    }

    /// A unit quaternion from four uniform numbers, as a motor.
    fn turn(rng: &mut Rng) -> Motor3 {
        loop {
            let q = [0; 4].map(|_| rng.range(-1.0, 1.0));
            let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
            if n > 0.1 {
                return Motor3::from_unit_quaternion(q[0] / n, q[1] / n, q[2] / n, q[3] / n);
            }
        }
    }

    /// A random group of up to a dozen objects, each with its pose:
    /// cubes, octahedra, boxes, parts enclosed off-centre in a shell, ink
    /// hulls, peeled shells, non-convex pairs, boxes face to face and
    /// exact duplicates.
    fn objects(rng: &mut Rng) -> Vec<(Mesh, Motor3)> {
        let count = 1 + rng.pick(12) as usize;
        let mut out: Vec<(Mesh, Motor3)> = Vec::new();
        while out.len() < count {
            let m = material(rng);
            let pose = Motor3::translator(
                rng.range(-1.5, 1.5),
                rng.range(-1.5, 1.5),
                rng.range(-1.5, 1.5),
            )
            .compose(&turn(rng));
            let last = out.last().cloned();
            match (rng.pick(9), last) {
                (0, _) => out.push((Mesh::cube(rng.range(0.2, 0.8), m).expect("valid"), pose)),
                (1, _) => {
                    let r = rng.range(0.3, 0.9);
                    out.push((Mesh::octahedron(r, m).expect("valid"), pose));
                }
                (2, _) => {
                    let lo = [0; 3].map(|_| rng.range(-0.8, -0.1));
                    let hi = [0; 3].map(|_| rng.range(0.1, 0.8));
                    out.push((boxed(lo, hi, m), pose));
                }
                (3, _) => {
                    out.push((boxed([-0.6; 3], [0.6; 3], m), pose));
                    let c = [0; 3].map(|_| rng.range(-0.3, 0.3));
                    let h = rng.range(0.05, 0.25);
                    let inner = boxed(c.map(|v| v - h), c.map(|v| v + h), material(rng));
                    out.push((inner, pose));
                }
                (4, Some((mesh, at))) => out.push((hull(&mesh, 1.05), at)),
                (5, _) => {
                    let peel = Peel::new(
                        rng.range(-0.1, 0.3),
                        [rng.range(-1.0, 1.0), 1.0, rng.range(-1.0, 1.0)],
                        (-0.5, 0.5),
                        rng.range(0.0, 0.9),
                    )
                    .expect("valid");
                    let shell = boxed([-0.5; 3], [0.5; 3], m);
                    let peeled = peel.apply(&shell, rng.range(0.0, 1.0)).expect("valid");
                    out.push((peeled, pose));
                }
                (6, _) => out.push((pair(rng, m), pose)),
                (7, Some((_, at))) => {
                    out.push((boxed([-1.0, -0.5, -0.5], [0.0, 0.5, 0.5], m), at));
                    out.push((boxed([0.0, -0.5, -0.5], [1.0, 0.5, 0.5], m), at));
                }
                (8, Some(twin)) => out.push(twin),
                _ => {}
            }
        }
        out
    }

    /// The view motor of a camera `distance` from `target` at `azimuth`
    /// and `elevation` (radians), looking at it.
    fn view(target: [f64; 3], azimuth: f64, elevation: f64, distance: f64) -> Motor3 {
        let about = |a: f64, x: f64, y: f64, z: f64| {
            let (s, c) = ((a / 2.0).sin(), (a / 2.0).cos());
            Motor3::from_unit_quaternion(c, x * s, y * s, z * s)
        };
        Motor3::translator(target[0], target[1], target[2])
            .compose(&about(azimuth, 0.0, 1.0, 0.0))
            .compose(&about(-elevation, 1.0, 0.0, 0.0))
            .compose(&Motor3::translator(0.0, 0.0, distance))
            .inverse()
    }

    fn posed<'a>(meshes: &'a [(Mesh, Motor3)], view: &Motor3) -> Vec<Posed<'a>> {
        meshes
            .iter()
            .enumerate()
            .map(|(index, (mesh, pose))| Posed {
                index,
                mesh,
                to_view: view.compose(pose),
            })
            .collect()
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(400))]

        /// Random groups, random cameras, both projections.
        #[test]
        fn random_scenes_order_the_same(seed in any::<u64>()) {
            let mut rng = Rng(seed);
            let meshes = objects(&mut rng);
            for _ in 0..4 {
                let to_view = view(
                    [0.0; 3],
                    rng.range(0.0, std::f64::consts::TAU),
                    rng.range(-0.6, 1.3),
                    rng.range(2.5, 9.0),
                );
                let projection = if rng.pick(4) == 0 {
                    Projection::Orthographic
                } else {
                    Projection::Pinhole { focal: rng.range(0.8, 3.0) }
                };
                same(&posed(&meshes, &to_view), projection, &format!("seed {seed}"));
            }
        }
    }

    /// The anatomy (skin with its ink hulls, muscle, bone) and two
    /// dumbbells, whole and half peeled, orbited all round under both
    /// projections.
    #[test]
    fn the_anatomy_orders_the_same_all_round() {
        let load = |text: &str| Obj::parse(text, Axes::ZUp).expect("a committed asset loads");
        let bind = |name: &str| Some(ramp().with_layer(layer_of(name).unwrap_or(Layer::Solid)));
        let layer = |text: &str| -> Vec<Mesh> {
            let meshes = load(text).meshes(bind).expect("binds");
            meshes.into_iter().map(|(_, mesh)| mesh).collect()
        };
        let skin = layer(include_str!("../../../assets/anatomy/skin.obj"));
        let inside: Vec<Mesh> = [
            include_str!("../../../assets/anatomy/muscle.obj"),
            include_str!("../../../assets/anatomy/bone.obj"),
        ]
        .into_iter()
        .flat_map(layer)
        .collect();
        let dumbbell = load(include_str!("../../../assets/props/dumbbell.obj"))
            .mesh(bind)
            .expect("binds");
        let peel = Peel::new(0.06, [0.0, 1.0, 0.0], (1.85, 0.0), 0.6).expect("valid");
        for u in [0.0, 0.5] {
            // As the anatomy example: whole at 0, every face apart after.
            let mut meshes: Vec<(Mesh, Motor3)> = skin
                .iter()
                .map(|m| match u {
                    0.0 => m.clone(),
                    _ => peel.apply(m, u).expect("valid"),
                })
                .chain(inside.iter().cloned())
                .map(|mesh| (mesh, Motor3::identity()))
                .collect();
            for x in [-0.25, 0.25] {
                meshes.push((dumbbell.clone(), Motor3::translator(x, 0.8, 0.3)));
            }
            for step in 0..8 {
                let azimuth = (f64::from(step) + 0.3) * std::f64::consts::TAU / 8.0;
                for (elevation, projection) in [
                    (0.14, Projection::Pinhole { focal: 2.7 }),
                    (0.6, Projection::Orthographic),
                ] {
                    let to_view = view([0.0, 0.95, 0.0], azimuth, elevation, 4.6);
                    same(
                        &posed(&meshes, &to_view),
                        projection,
                        &format!("peel {u}, step {step}, elevation {elevation}"),
                    );
                }
            }
        }
    }
}
