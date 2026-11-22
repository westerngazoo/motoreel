//! R-0018 — faceted 3D («3D de facetas»): the acceptance suite.
//!
//! One `mod` per acceptance criterion of `requirements/0018-faceted-3d.md`,
//! plus `order`, which checks the painter's order against a ray cast: for
//! sample points of the image, the last surface painted there must be the
//! nearest one the ray from the eye meets.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use garust::{pga, Motor3};
use motoreel::shapes::polygon;
use motoreel::{
    Camera, Faceted, Facing, Fill, Finish, FrameCtx, FrameSink, Gradient, Layer, Light, Look,
    Material, Mesh, MeshError, MeshFace, Object, Paint, PpmSink, Prim2, Projection, Pt2, Rgb,
    Scene, Seg, Stop, Style, Surface, SvgSink, Track,
};

// --- Shared helpers ---------------------------------------------------------

fn tmp_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clean the stale test directory");
    }
    dir
}

fn rgb(r: u8, g: u8, b: u8) -> Rgb {
    Rgb { r, g, b }
}

fn pt(x: f64, y: f64) -> Pt2 {
    Pt2 { x, y }
}

fn point(p: [f64; 3]) -> pga::Point {
    pga::Point::new(p[0], p[1], p[2])
}

fn mat(c: Rgb) -> Material {
    Material::new(&[c]).expect("valid")
}

const TONES: [Rgb; 3] = [
    Rgb {
        r: 18,
        g: 110,
        b: 168,
    },
    Rgb {
        r: 38,
        g: 190,
        b: 230,
    },
    Rgb {
        r: 160,
        g: 240,
        b: 255,
    },
];

fn ramp() -> Material {
    Material::new(&TONES).expect("valid")
}

/// An axis-aligned box from `lo` to `hi`, the cube's faces.
fn boxed(lo: [f64; 3], hi: [f64; 3], material: Material) -> Mesh {
    let cube = Mesh::cube(1.0, material).expect("valid");
    let vertices = cube
        .vertices()
        .iter()
        .map(|p| {
            let (x, y, z) = p.to_euclidean();
            let pick = |s: f64, a: f64, b: f64| if s < 0.0 { a } else { b };
            pga::Point::new(
                pick(x, lo[0], hi[0]),
                pick(y, lo[1], hi[1]),
                pick(z, lo[2], hi[2]),
            )
        })
        .collect();
    Mesh::new(vertices, cube.faces().to_vec(), vec![material]).expect("valid box")
}

/// A rotation of `a` radians about the unit axis `(x, y, z)`.
fn turn(a: f64, x: f64, y: f64, z: f64) -> Motor3 {
    let (s, c) = ((a / 2.0).sin(), (a / 2.0).cos());
    Motor3::from_unit_quaternion(c, x * s, y * s, z * s)
}

/// A pinhole camera `d` from `target`, at azimuth `az` and elevation `el`.
fn orbit(target: [f64; 3], d: f64, az: f64, el: f64, focal: f64) -> Camera {
    let pose = Motor3::translator(target[0], target[1], target[2])
        .compose(&turn(az, 0.0, 1.0, 0.0))
        .compose(&turn(-el, 1.0, 0.0, 0.0))
        .compose(&Motor3::translator(0.0, 0.0, d));
    Camera::pinhole(pose, focal)
}

fn faceted() -> Faceted {
    Faceted::new(3, 0.004).expect("valid")
}

fn look(finish: &dyn Finish) -> Look<'_> {
    Look {
        finish,
        light: Light::new([-0.6, 0.6, 0.52]).expect("valid"),
        seed: 3,
    }
}

const CTX: FrameCtx = FrameCtx { t: 0.0, seed: 3 };

/// One mesh in a scene, under `camera`.
fn scene_of(meshes: Vec<(Mesh, Motor3)>, camera: Camera) -> Scene {
    let mut scene = Scene::new(1.0);
    scene.camera = camera;
    for (mesh, pose) in meshes {
        scene.add(Object::mesh(mesh).at(pose));
    }
    scene
}

/// The one depth group's surfaces.
fn batch(scene: &Scene, t: f64) -> Vec<Surface> {
    scene.surfaces(t).into_iter().next().unwrap_or_default()
}

/// The straight corners of a surface's first subpath.
fn corners(s: &Surface) -> Vec<Pt2> {
    let sub = &s.outline()[0];
    std::iter::once(sub.start)
        .chain(sub.segs.iter().map(|seg| match *seg {
            Seg::Line(p) | Seg::Cubic(_, _, p) => p,
        }))
        .collect()
}

/// The trig-free quarter turn about +y: the rotor uses `sqrt(2)/2` alone.
fn quarter_turn_y() -> Motor3 {
    let h = std::f64::consts::FRAC_1_SQRT_2;
    Motor3::from_unit_quaternion(h, 0.0, h, 0.0)
}

// --- AC1: the mesh -----------------------------------------------------------

mod ac1 {
    use super::*;

    fn square(z: [f64; 4]) -> Vec<pga::Point> {
        vec![
            pga::Point::new(0.0, 0.0, z[0]),
            pga::Point::new(1.0, 0.0, z[1]),
            pga::Point::new(1.0, 1.0, z[2]),
            pga::Point::new(0.0, 1.0, z[3]),
        ]
    }

    fn one_face(vertices: Vec<pga::Point>, indices: Vec<usize>) -> Result<Mesh, MeshError> {
        Mesh::new(
            vertices,
            vec![MeshFace::new(indices, 0)],
            vec![mat(Rgb::WHITE)],
        )
    }

    #[test]
    fn the_cube_and_the_octahedron_are_valid() {
        let cube = Mesh::cube(0.5, ramp()).expect("valid");
        assert_eq!((cube.vertices().len(), cube.faces().len()), (8, 6));
        let octa = Mesh::octahedron(0.5, ramp()).expect("valid");
        assert_eq!((octa.vertices().len(), octa.faces().len()), (6, 8));
        assert_eq!(cube.materials(), &[ramp()]);
    }

    #[test]
    fn every_error_names_its_face() {
        let ok = square([0.0; 4]);
        assert_eq!(
            one_face(ok.clone(), vec![0, 1]),
            Err(MeshError::TooFewIndices(0))
        );
        assert_eq!(
            one_face(ok.clone(), vec![0, 1, 9]),
            Err(MeshError::IndexOutOfRange(0))
        );
        assert_eq!(
            Mesh::new(
                ok.clone(),
                vec![MeshFace::new(vec![0, 1, 2], 1)],
                vec![mat(Rgb::WHITE)]
            ),
            Err(MeshError::MaterialOutOfRange(0))
        );
        assert_eq!(
            one_face(square([0.0, 0.0, 0.0, 0.1]), vec![0, 1, 2, 3]),
            Err(MeshError::NonPlanar(0))
        );
        // A reflex corner, a straight corner, and a bow tie.
        let dart = vec![
            pga::Point::new(0.0, 0.0, 0.0),
            pga::Point::new(2.0, 0.0, 0.0),
            pga::Point::new(1.0, 0.5, 0.0),
            pga::Point::new(1.0, 2.0, 0.0),
        ];
        assert_eq!(
            one_face(dart, vec![0, 1, 2, 3]),
            Err(MeshError::NonConvex(0))
        );
        let line = vec![
            pga::Point::new(0.0, 0.0, 0.0),
            pga::Point::new(1.0, 0.0, 0.0),
            pga::Point::new(2.0, 0.0, 0.0),
            pga::Point::new(1.0, 1.0, 0.0),
        ];
        assert_eq!(
            one_face(line, vec![0, 1, 2, 3]),
            Err(MeshError::NonConvex(0))
        );
        assert_eq!(
            one_face(ok.clone(), vec![0, 2, 1, 3]),
            Err(MeshError::NonConvex(0))
        );
        // A later face is named by its own index.
        let two = Mesh::new(
            ok.clone(),
            vec![
                MeshFace::new(vec![0, 1, 2], 0),
                MeshFace::new(vec![0, 0, 1], 0),
            ],
            vec![mat(Rgb::WHITE)],
        );
        assert_eq!(two, Err(MeshError::NonConvex(1)));
    }

    #[test]
    fn a_non_finite_vertex_is_named() {
        let mut v = square([0.0; 4]);
        v[2] = pga::Point::new(f64::NAN, 1.0, 0.0);
        assert_eq!(one_face(v, vec![0, 1, 3]), Err(MeshError::Vertex(2)));
    }
}

// --- AC2: posed by motors ----------------------------------------------------

mod ac2 {
    use super::*;

    /// At each key time the posed, projected vertices are exactly the key
    /// motor's action, through the camera.
    #[test]
    fn key_times_return_the_key_poses() {
        let keys = [
            (0.0, Motor3::translator(0.2, -0.1, 0.0)),
            (
                1.0,
                Motor3::translator(-0.3, 0.1, 0.0).compose(&quarter_turn_y()),
            ),
            (2.0, turn(0.7, 0.0, 0.6, 0.8)),
        ];
        let mut scene = Scene::new(2.0);
        let camera = Camera::orthographic(Motor3::translator(0.0, 0.0, 5.0));
        scene.camera = camera;
        let cube = Mesh::cube(0.4, ramp()).expect("valid");
        scene.add(Object::mesh(cube.clone()).with_track(Track::keys(keys).expect("valid")));
        for (t, key) in keys {
            let to_view = camera.pose.inverse().compose(&key);
            let image: Vec<Pt2> = cube
                .vertices()
                .iter()
                .map(|p| {
                    Projection::Orthographic
                        .project(&p.transform(&to_view))
                        .expect("in front")
                })
                .collect();
            for s in batch(&scene, t) {
                let face = &cube.faces()[(s.id() & 0xffff_ffff) as usize];
                let want: Vec<Pt2> = face.indices.iter().map(|&k| image[k]).collect();
                assert_eq!(corners(&s), want, "t = {t}");
            }
        }
    }
}

// --- AC3: culling ------------------------------------------------------------

mod ac3 {
    use super::*;

    #[test]
    fn a_cube_seen_down_an_axis_keeps_one_face() {
        let camera = Camera::orthographic(Motor3::translator(0.0, 0.0, 5.0));
        let scene = scene_of(
            vec![(Mesh::cube(0.5, ramp()).expect("valid"), Motor3::identity())],
            camera,
        );
        let faces = batch(&scene, 0.0);
        assert_eq!(faces.len(), 1, "the edge-on sides are dropped too");
        assert_eq!(faces[0].facing(), Facing::Normal([0.0, 0.0, 1.0]));
    }

    #[test]
    fn a_pinhole_sees_at_most_three_faces_of_a_cube() {
        let cube = Mesh::cube(0.5, ramp()).expect("valid");
        let mut seen = Vec::new();
        for k in 0..24 {
            let camera = orbit(
                [0.0; 3],
                4.0,
                f64::from(k) * 0.37,
                0.1 + 0.05 * f64::from(k % 7),
                1.0,
            );
            let scene = scene_of(vec![(cube.clone(), Motor3::identity())], camera);
            seen.push(batch(&scene, 0.0).len());
        }
        assert!(seen.iter().all(|&n| (1..=3).contains(&n)), "{seen:?}");
        assert!(seen.contains(&3), "{seen:?}");
    }

    /// A face with one vertex behind the eye is culled whole; faces wholly
    /// in front survive.
    #[test]
    fn a_face_behind_the_eye_is_culled_whole() {
        // A long box along z through the camera plane, seen from inside.
        let long = boxed([-0.5, -0.5, -3.0], [0.5, 0.5, 6.0], ramp());
        let camera = Camera::pinhole(Motor3::translator(0.0, 0.0, 5.0), 1.0);
        let scene = scene_of(vec![(long, Motor3::identity())], camera);
        let faces = batch(&scene, 0.0);
        // Only the far end (−z) has every vertex in front of the eye.
        assert_eq!(
            faces.len(),
            0,
            "the far end faces away from an eye inside the box"
        );
        let near = boxed([-0.5, -0.5, -3.0], [0.5, 0.5, 4.0], ramp());
        let scene = scene_of(vec![(near, Motor3::identity())], camera);
        let ids: Vec<u64> = batch(&scene, 0.0).iter().map(|s| s.id()).collect();
        assert_eq!(ids, vec![1], "the +z end, the only face facing the eye");
    }
}

// --- AC4: the order is total and deterministic --------------------------------

mod ac4 {
    use super::*;

    fn posed() -> Motor3 {
        Motor3::translator(0.1, 0.05, 0.0)
            .compose(&turn(0.6, 0.0, 1.0, 0.0))
            .compose(&turn(0.4, 1.0, 0.0, 0.0))
    }

    #[test]
    fn shuffled_faces_paint_the_same_frame() {
        let camera = Camera::pinhole(Motor3::translator(0.0, 0.0, 4.0), 2.0);
        let cube = Mesh::cube(0.5, ramp()).expect("valid");
        let mut faces = cube.faces().to_vec();
        faces.reverse();
        faces.swap(1, 4);
        let shuffled =
            Mesh::new(cube.vertices().to_vec(), faces, cube.materials().to_vec()).expect("valid");
        let paint = |m: Mesh| {
            let finish = faceted();
            scene_of(vec![(m, posed())], camera).eval_with(0.0, &look(&finish))
        };
        assert_eq!(paint(cube), paint(shuffled));
    }

    /// Two faces at exactly the same depths are ordered by face index.
    #[test]
    fn equal_depths_order_by_index() {
        let v = [
            [-1.0, 0.0, 0.0],
            [-0.1, 0.0, 0.0],
            [-0.5, 0.5, 0.0],
            [0.1, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.5, 0.5, 0.0],
        ]
        .map(point)
        .to_vec();
        let camera = Camera::orthographic(Motor3::translator(0.0, 0.0, 5.0));
        for (first, second) in [
            (vec![0, 1, 2], vec![3, 4, 5]),
            (vec![3, 4, 5], vec![0, 1, 2]),
        ] {
            let m = Mesh::new(
                v.clone(),
                vec![MeshFace::new(first.clone(), 0), MeshFace::new(second, 0)],
                vec![ramp()],
            )
            .expect("valid");
            let ids: Vec<u64> = batch(&scene_of(vec![(m, Motor3::identity())], camera), 0.0)
                .iter()
                .map(Surface::id)
                .collect();
            assert_eq!(ids, vec![0, 1], "face index order for {first:?}");
        }
    }

    #[test]
    fn a_threaded_render_equals_a_serial_one() {
        let camera = orbit([0.0; 3], 4.0, 0.5, 0.3, 2.0);
        let scene = scene_of(
            vec![
                (Mesh::cube(0.4, ramp()).expect("valid"), posed()),
                (
                    Mesh::octahedron(0.5, ramp()).expect("valid"),
                    Motor3::translator(0.9, 0.0, 0.0),
                ),
            ],
            camera,
        );
        let finish = faceted();
        let serial: Vec<Vec<Prim2>> = (0..8)
            .map(|k| scene.eval_with(f64::from(k) / 8.0, &look(&finish)))
            .collect();
        let threaded: Vec<Vec<Prim2>> = std::thread::scope(|s| {
            let hs: Vec<_> = (0..8)
                .map(|k| {
                    let (scene, finish) = (&scene, &finish);
                    s.spawn(move || scene.eval_with(f64::from(k) / 8.0, &look(finish)))
                })
                .collect();
            hs.into_iter().map(|h| h.join().expect("thread")).collect()
        });
        assert_eq!(serial, threaded);
    }
}

// --- The painter's order against a ray cast ------------------------------------

mod order {
    use super::*;

    /// Each surface's view-space plane, by id.
    fn planes(scene: &Scene) -> Vec<(u64, [f64; 3], [f64; 3])> {
        let view = scene.camera.pose.inverse();
        let mut out = Vec::new();
        for (oi, obj) in scene.objects.iter().enumerate() {
            let motoreel::Shape::Mesh(mesh) = &obj.shape else {
                continue;
            };
            let to_view = view.compose(&obj.track.eval(0.0));
            let v: Vec<[f64; 3]> = mesh
                .vertices()
                .iter()
                .map(|p| {
                    let (x, y, z) = p.transform(&to_view).to_euclidean();
                    [x, y, z]
                })
                .collect();
            for (fi, f) in mesh.faces().iter().enumerate() {
                let (a, b, c) = (v[f.indices[0]], v[f.indices[1]], v[f.indices[2]]);
                let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                let w = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                let n = [
                    u[1] * w[2] - u[2] * w[1],
                    u[2] * w[0] - u[0] * w[2],
                    u[0] * w[1] - u[1] * w[0],
                ];
                out.push((((oi as u64) << 32) | fi as u64, n, a));
            }
        }
        out
    }

    /// Strictly inside a convex outline, at least `margin` from its edges.
    fn inside(s: &Surface, q: Pt2, margin: f64) -> bool {
        let c = corners(s);
        let n = c.len();
        (0..n).all(|k| {
            let (a, b) = (c[k], c[(k + 1) % n]);
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let len = (dx * dx + dy * dy).sqrt();
            (dx * (q.y - a.y) - dy * (q.x - a.x)) / len > margin
        })
    }

    /// How many sample points the painter's `order` gets wrong.
    fn mismatches(scene: &Scene, order: &[Surface]) -> usize {
        let Projection::Pinhole { focal } = scene.camera.projection else {
            panic!("pinhole");
        };
        let planes = planes(scene);
        let plane = |id: u64| {
            planes
                .iter()
                .find(|p| p.0 == id)
                .map(|p| (p.1, p.2))
                .expect("a plane per surface")
        };
        let mut wrong = 0;
        for i in 0..80 {
            for j in 0..80 {
                let q = pt(
                    -1.6 + 3.2 * f64::from(i) / 79.0,
                    -0.9 + 1.8 * f64::from(j) / 79.0,
                );
                let hits: Vec<&Surface> = order.iter().filter(|s| inside(s, q, 1e-6)).collect();
                let Some(painted) = hits.last() else {
                    continue;
                };
                let ray = [q.x / focal, q.y / focal, -1.0];
                let depth = |s: &Surface| {
                    let (n, p0) = plane(s.id());
                    (n[0] * p0[0] + n[1] * p0[1] + n[2] * p0[2])
                        / (n[0] * ray[0] + n[1] * ray[1] + n[2] * ray[2])
                };
                let nearest = hits
                    .iter()
                    .min_by(|a, b| depth(a).total_cmp(&depth(b)))
                    .expect("a hit");
                let gap = depth(painted) - depth(nearest);
                if painted.id() != nearest.id() && gap > 1e-9 {
                    wrong += 1;
                }
            }
        }
        wrong
    }

    /// A face-level sort by maximum depth, the spike's kind of order.
    fn face_sorted(mut b: Vec<Surface>) -> Vec<Surface> {
        b.sort_by(|x, y| y.depth().total_cmp(&x.depth()).then(x.id().cmp(&y.id())));
        b
    }

    /// The spike's platform: a wood slab beside rubber strips, separate
    /// boxes, and a small block standing on the wood.
    fn platform() -> Vec<(Mesh, Motor3)> {
        let still = Motor3::identity();
        vec![
            (boxed([-0.6, -0.1, -0.42], [0.6, 0.0, 0.42], ramp()), still),
            (
                boxed([-0.9, -0.1, -0.42], [-0.6, -0.02, 0.42], ramp()),
                still,
            ),
            (boxed([0.6, -0.1, -0.42], [0.9, -0.02, 0.42], ramp()), still),
            (boxed([-0.1, 0.0, -0.1], [0.1, 0.5, 0.1], ramp()), still),
            (boxed([-1.5, -0.3, -1.2], [1.5, -0.1, 1.2], ramp()), still),
        ]
    }

    #[test]
    fn large_faces_of_separate_boxes_sort_correctly_all_around() {
        let mut face_level_wrong = 0;
        for k in 0..36 {
            let az = f64::from(k) * std::f64::consts::TAU / 36.0;
            for el in [0.15, 0.45, 0.9] {
                let scene = scene_of(platform(), orbit([0.0, 0.1, 0.0], 3.0, az, el, 1.2));
                let ours = batch(&scene, 0.0);
                assert_eq!(mismatches(&scene, &ours), 0, "az {az:.2}, el {el}");
                face_level_wrong += mismatches(&scene, &face_sorted(ours));
            }
        }
        assert!(
            face_level_wrong > 0,
            "the check has teeth: a face-level sort fails it"
        );
    }

    /// Thirty boxes of mixed sizes in a loose grid, posed by one motor and
    /// seen from many angles.
    #[test]
    fn a_crowd_of_boxes_sorts_correctly() {
        let mut rng = 0x2545_F491_4F6C_DD1D_u64;
        let mut next = || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            (rng >> 11) as f64 / (1u64 << 53) as f64
        };
        let pose = turn(0.3, 0.0, 0.6, 0.8);
        let mut meshes = Vec::new();
        for i in 0..6 {
            for j in 0..5 {
                let (cx, cz) = (f64::from(i) * 0.5 - 1.25, f64::from(j) * 0.5 - 1.0);
                let (w, h, d) = (
                    0.05 + 0.18 * next(),
                    0.05 + 0.6 * next(),
                    0.05 + 0.18 * next(),
                );
                let lo = [cx - w, 0.0, cz - d];
                meshes.push((boxed(lo, [cx + w, h, cz + d], ramp()), pose));
            }
        }
        for k in 0..12 {
            let az = f64::from(k) * 0.53;
            let scene = scene_of(meshes.clone(), orbit([0.0, 0.2, 0.0], 3.5, az, 0.5, 1.2));
            assert_eq!(mismatches(&scene, &batch(&scene, 0.0)), 0, "view {k}");
        }
    }

    /// Two boxes crossing like a plus sign cannot both be right; each is
    /// drawn whole, so their faces never interleave (no zigzag).
    #[test]
    fn interpenetrating_solids_are_drawn_whole() {
        let parts = vec![
            (
                boxed([-1.0, -0.1, -0.1], [1.0, 0.1, 0.1], ramp()),
                Motor3::identity(),
            ),
            (
                boxed([-0.1, -0.12, -1.0], [0.1, 0.12, 1.0], ramp()),
                Motor3::identity(),
            ),
        ];
        for k in 0..12 {
            let scene = scene_of(
                parts.clone(),
                orbit([0.0; 3], 3.0, f64::from(k) * 0.5, 0.4, 1.2),
            );
            let objects: Vec<u64> = batch(&scene, 0.0).iter().map(|s| s.id() >> 32).collect();
            let runs = 1 + objects.windows(2).filter(|w| w[0] != w[1]).count();
            assert_eq!(runs, 2, "{objects:?}");
        }
    }

    /// An inverted hull (an ink outline: a slightly larger box with its
    /// faces turned inside out) is drawn before the part it outlines from
    /// every side. Ordered by nearest point it was not: seen from the
    /// front, the hull's far-side bottom reaches as near as the part.
    #[test]
    fn an_ink_hull_is_drawn_before_its_part() {
        let part = boxed([-0.6, -0.1, -0.42], [0.6, 0.0, 0.42], ramp());
        let big = boxed([-0.61, -0.11, -0.43], [0.61, 0.01, 0.43], ramp());
        let faces = big
            .faces()
            .iter()
            .map(|f| MeshFace::new(f.indices.iter().rev().copied().collect(), 0))
            .collect();
        let hull = Mesh::new(big.vertices().to_vec(), faces, vec![ramp()]).expect("valid");
        for k in 0..24 {
            let az = f64::from(k) * std::f64::consts::TAU / 24.0;
            for el in [0.1, 0.4] {
                let scene = scene_of(
                    vec![
                        (part.clone(), Motor3::identity()),
                        (hull.clone(), Motor3::identity()),
                    ],
                    orbit([0.0; 3], 3.0, az, el, 1.2),
                );
                let objects: Vec<u64> = batch(&scene, 0.0).iter().map(|s| s.id() >> 32).collect();
                assert_eq!(objects.first(), Some(&1), "az {az:.2} el {el}: {objects:?}");
                assert_eq!(objects.last(), Some(&0), "az {az:.2} el {el}");
            }
        }
    }

    /// A small part inside a large one (a bone in its limb) is drawn first.
    #[test]
    fn an_enclosed_part_is_drawn_before_its_shell() {
        let parts = vec![
            (
                boxed([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5], ramp()),
                Motor3::identity(),
            ),
            (
                boxed([-0.1, -0.1, -0.1], [0.1, 0.1, 0.1], ramp()),
                Motor3::identity(),
            ),
        ];
        for k in 0..8 {
            let scene = scene_of(
                parts.clone(),
                orbit([0.0; 3], 3.0, f64::from(k) * 0.8, 0.3, 1.2),
            );
            let b = batch(&scene, 0.0);
            assert_eq!(b.first().map(|s| s.id() >> 32), Some(1));
            assert_eq!(b.last().map(|s| s.id() >> 32), Some(0));
        }
    }
}

// --- AC5: the finish paints the faces ------------------------------------------

mod ac5 {
    use super::*;

    #[test]
    fn surfaces_carry_the_face() {
        let camera = Camera::pinhole(Motor3::translator(0.0, 0.0, 4.0), 2.0);
        let cube = Mesh::cube(0.5, ramp()).expect("valid");
        let pose = turn(0.5, 0.0, 1.0, 0.0).compose(&turn(0.3, 1.0, 0.0, 0.0));
        let mut scene = Scene::new(1.0);
        scene.camera = camera;
        scene.add(Object::point(pga::Point::new(0.0, 0.0, 0.0)));
        scene.add(Object::mesh(cube.clone()).at(pose));
        let to_view = camera.pose.inverse().compose(&pose.renormalize()); // as `Track::hold` stores it
        let view: Vec<[f64; 3]> = cube
            .vertices()
            .iter()
            .map(|p| {
                let (x, y, z) = p.transform(&to_view).to_euclidean();
                [x, y, z]
            })
            .collect();
        let faces = batch(&scene, 0.0);
        assert_eq!(faces.len(), 3);
        for s in &faces {
            assert_eq!(s.id() >> 32, 1, "the object's index");
            let face = &cube.faces()[(s.id() & 0xffff_ffff) as usize];
            let v: Vec<[f64; 3]> = face.indices.iter().map(|&k| view[k]).collect();
            let (u, w) = (
                [v[1][0] - v[0][0], v[1][1] - v[0][1], v[1][2] - v[0][2]],
                [v[2][0] - v[0][0], v[2][1] - v[0][1], v[2][2] - v[0][2]],
            );
            let n = [
                u[1] * w[2] - u[2] * w[1],
                u[2] * w[0] - u[0] * w[2],
                u[0] * w[1] - u[1] * w[0],
            ];
            assert_eq!(s.facing(), Facing::Normal(n));
            let far = v.iter().map(|p| -p[2]).fold(f64::NEG_INFINITY, f64::max);
            assert_eq!(s.depth(), far);
            assert_eq!(*s.material(), ramp());
            assert_eq!(s.outline()[0].segs.len(), 3, "a closed quad");
        }
        let finish = faceted();
        let prims = scene.eval_with(0.0, &look(&finish));
        assert_eq!(prims.len(), 1 + 3, "the point, then one path per face");
        assert!(prims[1..].iter().all(|p| matches!(p, Prim2::Path { .. })));
    }
}

// --- AC6: nothing already rendered moves ----------------------------------------

mod ac6 {
    use super::*;

    /// A one-face mesh facing an orthographic camera paints the same
    /// primitive as the same polygon drawn as a planar path.
    #[test]
    fn a_camera_facing_face_equals_a_planar_path() {
        let corners2 = [pt(-0.4, -0.3), pt(0.5, -0.2), pt(0.3, 0.4), pt(-0.2, 0.5)];
        let vertices = corners2
            .iter()
            .map(|p| pga::Point::new(p.x, p.y, 0.0))
            .collect();
        let face = Mesh::new(
            vertices,
            vec![MeshFace::new(vec![0, 1, 2, 3], 0)],
            vec![ramp()],
        )
        .expect("valid");
        let pose = Motor3::translator(0.25, -0.125, 0.0);
        let camera = Camera::orthographic(Motor3::translator(0.0, 0.0, 5.0));
        let finish = faceted();
        let mesh_prims = scene_of(vec![(face, pose)], camera).eval_with(0.0, &look(&finish));
        assert_eq!(mesh_prims.len(), 1);
        let Prim2::Path { style, .. } = &mesh_prims[0] else {
            panic!("a path");
        };
        let mut flat = Scene::new(1.0);
        flat.camera = camera;
        flat.add(
            Object::planar(polygon(&corners2))
                .with_style(*style)
                .at(pose),
        );
        assert_eq!(mesh_prims, flat.eval(0.0));
    }

    /// `eval` has no finish: a mesh emits nothing there, and every other
    /// object is untouched by its presence.
    #[test]
    fn eval_skips_meshes_and_keeps_the_rest() {
        let mut scene = Scene::new(1.0);
        scene.add(Object::point(pga::Point::new(0.0, 0.0, 0.0)));
        scene.add(Object::mesh(Mesh::cube(0.3, ramp()).expect("valid")));
        scene.add(Object::point(pga::Point::new(0.5, 0.0, 0.0)));
        let mut without = Scene::new(1.0);
        without.add(Object::point(pga::Point::new(0.0, 0.0, 0.0)));
        without.add(Object::point(pga::Point::new(0.5, 0.0, 0.0)));
        assert_eq!(scene.eval(0.0), without.eval(0.0));
    }
}

// --- AC7: seams do not leak ----------------------------------------------------------

mod ac7 {
    use super::*;

    /// A square made of two triangles sharing a diagonal, opaque, one
    /// colour, over black: the 256 × 128 canvas over a 4 × 2 view.
    fn render(stroke: f64, name: &str) -> Vec<u8> {
        let v = [
            [-0.5, -0.45, 0.0],
            [0.5, -0.45, 0.0],
            [0.5, 0.45, 0.0],
            [-0.5, 0.45, 0.0],
        ]
        .map(point)
        .to_vec();
        let c = rgb(200, 120, 40);
        let mesh = Mesh::new(
            v,
            vec![
                MeshFace::new(vec![0, 1, 2], 0),
                MeshFace::new(vec![0, 2, 3], 0),
            ],
            vec![mat(c)],
        )
        .expect("valid");
        let camera = Camera::orthographic(Motor3::translator(0.0, 0.0, 5.0));
        let finish = Faceted::new(2, stroke).expect("valid");
        let mut scene = scene_of(vec![(mesh, Motor3::identity())], camera);
        scene.view = (4.0, 2.0);
        let dir = tmp_dir(name);
        let mut sink = PpmSink::with_view(&dir, (256, 128), (4.0, 2.0)).expect("sink");
        sink.frame(0, &scene.eval_with(0.0, &look(&finish)))
            .expect("frame");
        let bytes = fs::read(dir.join("frame_00000.ppm")).expect("read");
        bytes[bytes.len() - 256 * 128 * 3..].to_vec()
    }

    /// The darkest channel sum along the shared diagonal, inside the square.
    fn darkest_on_the_diagonal(px: &[u8]) -> u32 {
        // Image x = −0.5 + 1.0·s, y = −0.45 + 0.9·s; 64 px per unit.
        (8..56)
            .map(|k| {
                let s = f64::from(k) / 64.0;
                let (x, y) = (-0.5 + s, -0.45 + 0.9 * s);
                let col = ((x + 2.0) * 64.0) as usize;
                let row = ((1.0 - y) * 64.0) as usize;
                let i = (row * 256 + col) * 3;
                u32::from(px[i]) + u32::from(px[i + 1]) + u32::from(px[i + 2])
            })
            .min()
            .expect("samples")
    }

    /// A self-stroke two pixels wide closes an oblique shared edge
    /// completely; one pixel (rf-marca's 2D width) leaves at most a few
    /// levels; none leaves the background showing (SPEC-0018 §2.6).
    #[test]
    fn the_self_stroke_closes_the_shared_edge() {
        let full = 200 + 120 + 40;
        assert_eq!(
            darkest_on_the_diagonal(&render(2.0 / 64.0, "r0018_ac7_sealed")),
            full
        );
        let one_px = darkest_on_the_diagonal(&render(1.0 / 64.0, "r0018_ac7_one"));
        assert!(
            full - one_px <= 6,
            "one pixel: {} levels short",
            full - one_px
        );
        assert!(
            darkest_on_the_diagonal(&render(0.0, "r0018_ac7_open")) < one_px,
            "without the stroke the background shows: the check has teeth"
        );
    }
}

// --- AC8: gradients and finishes compose -----------------------------------------

mod ac8 {
    use super::*;

    fn stops() -> [Stop; 2] {
        [
            Stop {
                offset: 0.0,
                colour: rgb(255, 0, 0),
                alpha: 1.0,
            },
            Stop {
                offset: 1.0,
                colour: rgb(0, 0, 255),
                alpha: 1.0,
            },
        ]
    }

    /// Lays a gradient from the frame's origin to its `x_end` on each
    /// surface, through the frame.
    struct Framed;

    impl Finish for Framed {
        fn paint(&self, batch: &[Surface], _: &Light, _: &FrameCtx) -> Vec<Prim2> {
            let unit = Gradient::linear(pt(0.0, 0.0), pt(1.0, 0.0), &stops()).expect("valid");
            batch
                .iter()
                .filter_map(|s| {
                    let g = unit.map_frame(|p| s.frame_point(p.x, p.y))?;
                    Some(Prim2::Path {
                        subpaths: s.outline().to_vec(),
                        style: Style {
                            fill: Some(Fill::gradient(g, 1.0)),
                            width: 0.0,
                            ..Style::default()
                        },
                    })
                })
                .collect()
        }
    }

    fn frame_of(p: &Prim2) -> (Pt2, Pt2, Pt2) {
        let Prim2::Path { style, .. } = p else {
            panic!("a path");
        };
        let Some(Fill {
            paint: Paint::Gradient(g),
            ..
        }) = style.fill
        else {
            panic!("a gradient");
        };
        g.frame()
    }

    #[test]
    fn the_frame_is_the_exact_image_of_the_face() {
        let tri = [[-0.4, -0.3, 0.0], [0.6, -0.2, 0.0], [0.1, 0.5, 0.0]];
        let mesh = Mesh::new(
            tri.map(point).to_vec(),
            vec![MeshFace::new(vec![0, 1, 2], 0)],
            vec![ramp()],
        )
        .expect("valid");
        let pose = Motor3::translator(0.25, 0.5, -1.0).compose(&turn(0.3, 0.0, 0.0, 1.0));
        let camera = Camera::orthographic(Motor3::translator(0.0, 0.0, 5.0));
        let scene = scene_of(vec![(mesh, pose)], camera);
        let s = &batch(&scene, 0.0)[0];
        let to_view = camera.pose.inverse().compose(&pose.renormalize()); // as `Track::hold` stores it
        let image = tri.map(|v| {
            Projection::Orthographic
                .project(&point(v).transform(&to_view))
                .expect("front")
        });
        assert_eq!(s.frame(), Some(image));

        // The finish's gradient is the image of the same gradient laid on
        // the model face and posed by eval (R-0016), up to one rounding.
        let finish = Framed;
        let ours = frame_of(&scene.eval_with(0.0, &look(&finish))[0]);
        let (a, b, c) = (tri[0], tri[1], tri[2]);
        // The same unit gradient laid on the model face through the face's
        // own model frame, then posed by eval.
        let model = Gradient::linear(pt(0.0, 0.0), pt(1.0, 0.0), &stops())
            .expect("valid")
            .map_frame(|p| {
                Some(pt(
                    a[0] + p.x * (b[0] - a[0]) + p.y * (c[0] - a[0]),
                    a[1] + p.x * (b[1] - a[1]) + p.y * (c[1] - a[1]),
                ))
            })
            .expect("maps");
        let mut flat = Scene::new(1.0);
        flat.camera = camera;
        let poly: Vec<Pt2> = tri.iter().map(|v| pt(v[0], v[1])).collect();
        flat.add(
            Object::planar(polygon(&poly))
                .with_style(Style {
                    fill: Some(Fill::gradient(model, 1.0)),
                    width: 0.0,
                    ..Style::default()
                })
                .at(pose),
        );
        let theirs = frame_of(&flat.eval(0.0)[0]);
        for (p, q) in [(ours.0, theirs.0), (ours.1, theirs.1), (ours.2, theirs.2)] {
            assert!(
                (p.x - q.x).abs() < 1e-12 && (p.y - q.y).abs() < 1e-12,
                "{p:?} vs {q:?}"
            );
        }
    }
}

// --- AC9: determinism and the goldens ---------------------------------------------

mod ac9 {
    use super::*;

    const SIZE: (u32, u32) = (160, 90);

    /// A cube and an octahedron under an orthographic camera, posed by a
    /// translation and a quarter turn (a rotor of `sqrt(2)/2` alone). The
    /// camera looks down from a three-quarter view through two rotors with
    /// Pythagorean components (0.96, 0.28): no trig anywhere.
    fn golden_scene() -> Scene {
        let around = Motor3::from_unit_quaternion(0.96, 0.0, 0.28, 0.0);
        let down = Motor3::from_unit_quaternion(0.96, -0.28, 0.0, 0.0);
        let mut scene = Scene::new(1.0);
        scene.camera = Camera::orthographic(
            around
                .compose(&down)
                .compose(&Motor3::translator(0.0, 0.0, 5.0)),
        );
        scene.add(
            Object::mesh(Mesh::cube(0.35, ramp()).expect("valid"))
                .at(Motor3::translator(-0.6, 0.0, 0.0).compose(&quarter_turn_y())),
        );
        scene.add(
            Object::mesh(Mesh::octahedron(0.5, ramp()).expect("valid"))
                .at(Motor3::translator(0.6, 0.0, 0.0)),
        );
        scene
    }

    fn render(name: &str, svg: bool) -> Vec<u8> {
        let dir = tmp_dir(name);
        let finish = faceted();
        let prims = golden_scene().eval_with(0.0, &look(&finish));
        let file = if svg {
            let mut sink = SvgSink::with_view(&dir, SIZE, (3.2, 1.8)).expect("svg sink");
            sink.frame(0, &prims).expect("frame");
            "frame_00000.svg"
        } else {
            let mut sink = PpmSink::with_view(&dir, SIZE, (3.2, 1.8)).expect("ppm sink");
            sink.frame(0, &prims).expect("frame");
            "frame_00000.ppm"
        };
        fs::read(dir.join(file)).expect("read frame")
    }

    fn check_golden(fixture: &str, got: &[u8]) {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/golden")
            .join(fixture);
        if std::env::var_os("MOTOREEL_BLESS").is_some_and(|v| v == "1") {
            fs::write(&path, got).expect("bless the golden fixture");
            return;
        }
        let want = fs::read(&path).unwrap_or_else(|_| {
            panic!(
                "{} is missing: MOTOREEL_BLESS=1 cargo test -p motoreel --test r0018_mesh",
                path.display()
            )
        });
        assert!(want == got, "{fixture} drifted");
    }

    #[test]
    fn both_solids_are_on_the_frame() {
        let finish = faceted();
        let prims = golden_scene().eval_with(0.0, &look(&finish));
        assert!(prims.len() >= 4, "{} faces", prims.len());
    }

    #[test]
    fn two_renders_and_four_threads_are_byte_identical() {
        for svg in [false, true] {
            assert!(
                render(&format!("r0018_twice_a_{svg}"), svg)
                    == render(&format!("r0018_twice_b_{svg}"), svg)
            );
        }
        let reference = render("r0018_threads_ref", false);
        std::thread::scope(|s| {
            let hs: Vec<_> = (0..4)
                .map(|i| s.spawn(move || render(&format!("r0018_threads_{i}"), false)))
                .collect();
            for h in hs {
                assert!(h.join().expect("thread") == reference);
            }
        });
    }

    #[test]
    fn the_ppm_golden_matches_byte_for_byte() {
        check_golden("r0018_solids.ppm", &render("r0018_ppm_golden", false));
    }

    #[test]
    fn the_svg_golden_matches_byte_for_byte() {
        check_golden("r0018_solids.svg", &render("r0018_svg_golden", true));
    }

    #[test]
    fn the_3d_code_is_trig_free() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        for file in ["mesh.rs", "solid.rs"] {
            let code = fs::read_to_string(src.join(file)).expect("read source");
            for banned in [
                ".sin(", ".cos(", ".tan(", ".atan", ".exp(", ".ln(", ".powf(", "mul_add",
            ] {
                assert!(!code.contains(banned), "{file} uses {banned}");
            }
        }
    }
}

// --- AC10: a demo encodes -------------------------------------------------------------

mod ac10 {
    use super::*;

    /// A 120-frame turn of a cube and an octahedron under the pinhole
    /// camera, encoded by stock ffmpeg from the PPM frames. Skips (with a
    /// note) only where ffmpeg is absent, as R-0006 AC6 does.
    #[test]
    fn a_120_frame_turn_encodes_with_stock_ffmpeg() {
        let dir = tmp_dir("r0018_ac10");
        let finish = faceted();
        let mut sink = PpmSink::with_view(&dir, (96, 54), (3.2, 1.8)).expect("sink");
        for i in 0..120_u32 {
            let az = f64::from(i) * std::f64::consts::TAU / 120.0;
            let scene = scene_of(
                vec![
                    (
                        Mesh::cube(0.35, ramp()).expect("valid"),
                        Motor3::translator(-0.5, 0.0, 0.0),
                    ),
                    (
                        Mesh::octahedron(0.45, ramp()).expect("valid"),
                        Motor3::translator(0.5, 0.0, 0.0),
                    ),
                ],
                orbit([0.0; 3], 4.0, az, 0.35, 2.0),
            );
            let light = Light::new(scene.camera.view_direction([-0.5, 0.8, 0.6])).expect("valid");
            let l = Look {
                light,
                ..look(&finish)
            };
            sink.frame(i as usize, &scene.eval_with(0.0, &l))
                .expect("frame");
        }
        let mp4 = dir.join("turn.mp4");
        let run = Command::new("ffmpeg")
            .args(["-loglevel", "error", "-y", "-framerate", "30", "-i"])
            .arg(dir.join("frame_%05d.ppm"))
            .args(["-pix_fmt", "yuv420p"])
            .arg(&mp4)
            .status();
        match run {
            Ok(status) => {
                assert!(status.success(), "ffmpeg failed");
                assert!(fs::metadata(&mp4).expect("mp4").len() > 0);
            }
            Err(_) => eprintln!("R-0018 AC10 skipped: ffmpeg not found"),
        }
    }
}

// --- AC11: the kernel stays thin ----------------------------------------------------------

mod ac11 {
    use super::*;

    /// Meshes live in the core: a skin material and a mesh need no feature.
    #[test]
    fn meshes_need_no_feature() {
        let m = mat(Rgb::WHITE).with_layer(Layer::Skin);
        assert!(Mesh::cube(1.0, m).is_ok());
        let _ = Faceted::new(1, 0.0).expect("valid").paint(
            &[],
            &Light::new([0.0, 0.0, 1.0]).expect("valid"),
            &CTX,
        );
    }
}
