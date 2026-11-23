//! R-0021 — layers and the peel («capas», «pelar»): the acceptance suite,
//! one `mod` per criterion of `requirements/0021-layers.md`.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use garust::{pga, Motor3};
use motoreel::{
    Axes, Camera, Faceted, Finish, FrameSink, Glass, Glow, Layer, Light, Look, Material, Mesh,
    MeshFace, Obj, Object, Peel, PeelError, PpmSink, Rgb, Scene, Surface, Sweep, XRay,
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

fn ramp() -> Material {
    Material::new(&[rgb(18, 110, 168), rgb(38, 190, 230), rgb(160, 240, 255)]).expect("valid")
}

fn rose() -> Material {
    Material::new(&[rgb(120, 40, 60), rgb(196, 84, 104), rgb(236, 140, 150)]).expect("valid")
}

fn bone() -> Material {
    Material::new(&[rgb(160, 240, 255), rgb(222, 246, 255), rgb(246, 248, 255)])
        .expect("valid")
        .with_layer(Layer::Inner)
}

fn xyz(p: &pga::Point) -> [f64; 3] {
    let (x, y, z) = p.to_euclidean();
    [x, y, z]
}

/// An axis-aligned box from `lo` to `hi`, the cube's faces.
fn boxed(lo: [f64; 3], hi: [f64; 3], material: Material) -> Mesh {
    let cube = Mesh::cube(1.0, material).expect("valid");
    let vertices = cube
        .vertices()
        .iter()
        .map(|p| {
            let [x, y, z] = xyz(p);
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

/// A pinhole camera `d` from the origin at azimuth `az`, elevation `el`.
fn orbit(d: f64, az: f64, el: f64) -> Camera {
    let pose = turn(az, 0.0, 1.0, 0.0)
        .compose(&turn(-el, 1.0, 0.0, 0.0))
        .compose(&Motor3::translator(0.0, 0.0, d));
    Camera::pinhole(pose, 1.2)
}

fn scene_of(meshes: &[Mesh], camera: Camera) -> Scene {
    let mut scene = Scene::new(1.0);
    scene.camera = camera;
    for mesh in meshes {
        scene.add(Object::mesh(mesh.clone()));
    }
    scene
}

/// The objects in paint order, each once.
fn object_order(scene: &Scene) -> Vec<u64> {
    let batch: Vec<Surface> = scene.surfaces(0.0).into_iter().flatten().collect();
    let mut out: Vec<u64> = Vec::new();
    for s in &batch {
        let o = s.id() >> 32;
        if out.last() != Some(&o) {
            out.push(o);
        }
    }
    out
}

/// Every azimuth in 16 steps, two elevations.
fn views() -> impl Iterator<Item = Camera> {
    (0..16).flat_map(|k| {
        [0.15, 0.5]
            .into_iter()
            .map(move |el| orbit(4.0, f64::from(k) * std::f64::consts::TAU / 16.0, el))
    })
}

fn peel() -> Peel {
    Peel::new(0.25, [0.0, 1.0, 0.0], (1.0, -1.0), 0.5).expect("valid")
}

// --- AC1: nested parts are drawn inside out -----------------------------------

mod ac1 {
    use super::*;

    fn nest() -> [Mesh; 3] {
        [
            boxed([-0.2, -0.2, -0.2], [0.2, 0.2, 0.2], bone()),
            boxed([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5], rose()),
            boxed([-1.0, -1.0, -1.0], [1.0, 1.0, 1.0], ramp()),
        ]
    }

    #[test]
    fn three_nested_cubes_draw_inside_out_in_any_order() {
        let [b, m, s] = nest();
        let orders: [[usize; 3]; 6] = [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];
        let layers = [b, m, s];
        for perm in orders {
            let meshes: Vec<Mesh> = perm.iter().map(|&k| layers[k].clone()).collect();
            for camera in views() {
                let scene = scene_of(&meshes, camera);
                let painted: Vec<usize> = object_order(&scene)
                    .into_iter()
                    .map(|o| perm[o as usize])
                    .collect();
                assert_eq!(painted, [0, 1, 2], "insertion {perm:?}");
            }
        }
    }

    /// A part pushed against the front of its shell: its visible faces lie
    /// nearer on average than the shell's, so the mean-depth key alone
    /// would paint it over the shell that hides it.
    #[test]
    fn an_off_centre_part_stays_under_its_shell() {
        let shell = boxed([-1.0, -1.0, -1.0], [1.0, 1.0, 1.0], ramp());
        let part = boxed([0.3, -0.2, 0.55], [0.9, 0.4, 0.95], rose());
        let camera = Camera::orthographic(
            turn(0.35, 0.0, 1.0, 0.0)
                .compose(&turn(-0.3, 1.0, 0.0, 0.0))
                .compose(&Motor3::translator(0.0, 0.0, 5.0)),
        );
        let scene = scene_of(&[shell.clone(), part.clone()], camera);
        // The premise: the part's visible faces are nearer on average.
        let batch: Vec<Surface> = scene.surfaces(0.0).into_iter().flatten().collect();
        let mean = |o: u64| {
            let d: Vec<f64> = batch
                .iter()
                .filter(|s| s.id() >> 32 == o)
                .map(Surface::depth)
                .collect();
            d.iter().sum::<f64>() / d.len() as f64
        };
        assert!(mean(1) < mean(0), "the test's premise");
        assert_eq!(object_order(&scene), [1, 0]);
        for camera in views() {
            assert_eq!(
                object_order(&scene_of(&[shell.clone(), part.clone()], camera)),
                [1, 0]
            );
        }
    }

    #[test]
    fn a_peeled_shell_still_covers_its_inside() {
        let [b, m, s] = nest();
        let peeled = peel().apply(&s, 0.6).expect("valid");
        for camera in views() {
            let scene = scene_of(&[peeled.clone(), m.clone(), b.clone()], camera);
            assert_eq!(object_order(&scene), [2, 1, 0]);
        }
    }

    #[test]
    fn an_ink_hull_keeps_its_order() {
        let part = boxed([-0.6, -0.1, -0.42], [0.6, 0.0, 0.42], ramp());
        let big = boxed([-0.61, -0.11, -0.43], [0.61, 0.01, 0.43], ramp());
        let faces = big
            .faces()
            .iter()
            .map(|f| MeshFace::new(f.indices.iter().rev().copied().collect(), 0))
            .collect();
        let hull = Mesh::new(big.vertices().to_vec(), faces, vec![ramp()]).expect("valid");
        for camera in views() {
            let scene = scene_of(&[part.clone(), hull.clone()], camera);
            assert_eq!(object_order(&scene), [1, 0]);
        }
    }
}

// --- AC2: the peel -------------------------------------------------------------

mod ac2 {
    use super::*;

    #[test]
    fn bad_fields_are_named() {
        let up = [0.0, 1.0, 0.0];
        assert!(matches!(
            Peel::new(f64::NAN, up, (0.0, 1.0), 0.0),
            Err(PeelError::Lift(_))
        ));
        assert!(matches!(
            Peel::new(f64::INFINITY, up, (0.0, 1.0), 0.0),
            Err(PeelError::Lift(_))
        ));
        assert_eq!(
            Peel::new(0.1, [0.0; 3], (0.0, 1.0), 0.0),
            Err(PeelError::Axis)
        );
        assert_eq!(Peel::new(0.1, up, (1.0, 1.0), 0.0), Err(PeelError::Span));
        assert_eq!(
            Peel::new(0.1, up, (0.0, f64::NAN), 0.0),
            Err(PeelError::Span)
        );
        assert_eq!(
            Peel::new(0.1, up, (0.0, 1.0), 1.0),
            Err(PeelError::Stagger(1.0))
        );
        assert_eq!(
            Peel::new(0.1, up, (0.0, 1.0), -0.1),
            Err(PeelError::Stagger(-0.1))
        );
    }

    /// One unit square facing +z, lifted half way.
    #[test]
    fn faces_move_along_their_normals_and_fade() {
        let square = Mesh::new(
            vec![
                pga::Point::new(0.0, 0.0, 0.0),
                pga::Point::new(2.0, 0.0, 0.0),
                pga::Point::new(2.0, 2.0, 0.0),
                pga::Point::new(0.0, 2.0, 0.0),
            ],
            vec![MeshFace::new(vec![0, 1, 2, 3], 0)],
            vec![ramp()],
        )
        .expect("valid");
        let p = Peel::new(0.4, [0.0, 1.0, 0.0], (0.0, 1.0), 0.0).expect("valid");
        let half = p.apply(&square, 0.5).expect("valid");
        // smoothstep(0.5) = 0.5: lifted 0.2 along +z, opacity 0.5.
        let got: Vec<[f64; 3]> = half.vertices().iter().map(xyz).collect();
        assert_eq!(
            got,
            [
                [0.0, 0.0, 0.2],
                [2.0, 0.0, 0.2],
                [2.0, 2.0, 0.2],
                [0.0, 2.0, 0.2]
            ]
        );
        assert_eq!(half.materials()[0].opacity(), 0.5);
        assert_eq!(half.materials()[0].tones(), ramp().tones());
    }

    #[test]
    fn the_stagger_runs_along_the_axis() {
        // A tall box: its top starts first, its bottom at the stagger.
        let tall = boxed([-0.5, -1.0, -0.5], [0.5, 1.0, 0.5], ramp());
        let p = peel(); // span 1 → −1 along +y, stagger 0.5
        let at = |u: f64| p.apply(&tall, u).expect("valid");
        let opacity = |m: &Mesh, normal_y: f64| {
            m.faces()
                .iter()
                .find(|f| {
                    let v: Vec<[f64; 3]> =
                        f.indices.iter().map(|&i| xyz(&m.vertices()[i])).collect();
                    v.iter().all(|q| (q[1] - normal_y).abs() < 0.3)
                })
                .map(|f| m.materials()[f.material].opacity())
                .expect("the face")
        };
        let early = at(0.25);
        assert!(opacity(&early, 1.0) < 1.0, "the top has started");
        assert_eq!(opacity(&early, -1.0), 1.0, "the bottom waits");
        let late = at(0.75);
        assert!(opacity(&late, -1.0) < 1.0, "the bottom has started");
        assert!(opacity(&late, 1.0) < opacity(&late, -1.0), "the top leads");
    }

    #[test]
    fn nothing_moves_at_zero_and_all_is_gone_at_one() {
        let cube = boxed([-1.0; 3], [1.0; 3], ramp());
        let zero = peel().apply(&cube, 0.0).expect("valid");
        assert!(zero.materials().iter().all(|m| *m == ramp()));
        let one = peel().apply(&cube, 1.0).expect("valid");
        assert!(one.materials().iter().all(|m| m.opacity() == 0.0));
        // Every face moved out by the full lift along its own normal.
        for f in one.faces() {
            let v: Vec<[f64; 3]> = f.indices.iter().map(|&i| xyz(&one.vertices()[i])).collect();
            let off = v[0].iter().map(|c| c.abs()).fold(0.0, f64::max);
            assert!((off - 1.25).abs() < 1e-12, "{v:?}");
        }
        assert_eq!(one.faces().len(), cube.faces().len());
    }

    #[test]
    fn a_peel_at_zero_renders_the_mesh() {
        let cube = boxed([-1.0; 3], [1.0; 3], ramp());
        let finish = Faceted::new(3, 0.004).expect("valid");
        let a = render(std::slice::from_ref(&cube), "r0021_ac2_mesh", &finish);
        let b = render(
            &[peel().apply(&cube, 0.0).expect("valid")],
            "r0021_ac2_peel0",
            &finish,
        );
        assert!(a == b);
    }
}

// --- AC3: highlight --------------------------------------------------------------

mod ac3 {
    use super::*;

    #[test]
    fn mix_is_exact_at_the_ends_and_rounds_between() {
        let lit = Material::new(&[rgb(196, 90, 10), rgb(255, 138, 31), rgb(255, 200, 90)])
            .expect("valid")
            .with_opacity(0.5)
            .expect("valid");
        let base = rose().with_layer(Layer::Skin);
        assert_eq!(base.mix(&lit, 0.0), base);
        let full = base.mix(&lit, 1.0);
        assert_eq!(full.tones(), lit.tones());
        assert_eq!(full.opacity(), 0.5);
        assert_eq!(full.layer(), Layer::Skin, "the layer is the first's");
        let half = base.mix(&lit, 0.5);
        // 120 + (196 − 120)/2 = 158; 40 + 50/2 = 65; 60 + (10 − 60)/2 = 35.
        assert_eq!(half.tones()[0], rgb(158, 65, 35));
        assert_eq!(half.opacity(), 0.75);
        assert_eq!(base.mix(&lit, -3.0), base);
        assert_eq!(base.mix(&lit, f64::NAN), base);
        assert_eq!(base.mix(&lit, 7.0).tones(), lit.tones());
        // A shorter ramp lends its last tone.
        let one = Material::new(&[rgb(0, 0, 0)]).expect("valid");
        assert_eq!(base.mix(&one, 1.0).tones(), [rgb(0, 0, 0); 3]);
    }
}

// --- AC4, AC5: the anatomy assets -----------------------------------------------

fn anatomy(file: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/anatomy")
        .join(file);
    fs::read_to_string(path).expect("a committed anatomy file")
}

/// Segment names from `joints.tsv` (header skipped).
fn segments() -> BTreeSet<String> {
    anatomy("joints.tsv")
        .lines()
        .skip(1)
        .filter_map(|l| l.split('\t').next().map(str::to_owned))
        .collect()
}

fn layer(file: &str) -> Vec<(String, Mesh)> {
    Obj::parse(&anatomy(file), Axes::ZUp)
        .unwrap_or_else(|e| panic!("{file}: {e}"))
        .meshes(|_| Some(ramp()))
        .unwrap_or_else(|e| panic!("{file}: {e}"))
}

mod ac4 {
    use super::*;

    #[test]
    fn every_part_rides_a_rig_segment() {
        let rig = segments();
        assert_eq!(rig.len(), 16, "{rig:?}");
        // Every joint names a known parent (the pelvis is the root).
        for line in anatomy("joints.tsv").lines().skip(1) {
            let cols: Vec<&str> = line.split('\t').collect();
            assert_eq!(cols.len(), 5, "{line}");
            assert!(cols[1] == "-" || rig.contains(cols[1]), "{line}");
            for c in &cols[2..] {
                assert!(c.parse::<f64>().is_ok_and(f64::is_finite), "{line}");
            }
        }
        for file in ["skin.obj", "muscle.obj", "bone.obj"] {
            let mut covered = BTreeSet::new();
            for (name, _) in layer(file) {
                let segment = name.split('.').next().unwrap_or_default().to_owned();
                assert!(rig.contains(&segment), "{file}: {name}");
                covered.insert(segment);
            }
            if file != "muscle.obj" {
                assert_eq!(covered, rig, "{file} covers every segment");
            }
        }
    }

    /// Whether every vertex of `inner` is inside every face plane of
    /// `shell` (the depth sort's enclosure test, AC1).
    fn encloses(shell: &Mesh, inner: &Mesh) -> bool {
        let v: Vec<[f64; 3]> = shell.vertices().iter().map(xyz).collect();
        shell.faces().iter().all(|f| {
            let (a, b, c) = (v[f.indices[0]], v[f.indices[1]], v[f.indices[2]]);
            let (u, w) = (
                [b[0] - a[0], b[1] - a[1], b[2] - a[2]],
                [c[0] - a[0], c[1] - a[1], c[2] - a[2]],
            );
            let n = [
                u[1] * w[2] - u[2] * w[1],
                u[2] * w[0] - u[0] * w[2],
                u[0] * w[1] - u[1] * w[0],
            ];
            inner
                .vertices()
                .iter()
                .map(xyz)
                .all(|p| n[0] * (p[0] - a[0]) + n[1] * (p[1] - a[1]) + n[2] * (p[2] - a[2]) < 0.0)
        })
    }

    /// Every muscle and every bone lies inside a skin part of its own
    /// segment, so it is drawn under it from every side (AC1), and the
    /// muscles keep clear of their segment's bones.
    #[test]
    fn every_inner_part_is_inside_its_segments_skin() {
        let skin = layer("skin.obj");
        for file in ["muscle.obj", "bone.obj"] {
            for (name, part) in layer(file) {
                let segment = name.split('.').next().unwrap_or_default();
                let inside = skin.iter().any(|(s, shell)| {
                    s.split('.').next() == Some(segment)
                        && !s.ends_with("_ink")
                        && encloses(shell, &part)
                });
                assert!(inside, "{file}: {name} pokes out of its skin");
            }
        }
    }
}

mod ac5 {
    use super::*;

    fn materials(file: &str) -> BTreeSet<String> {
        anatomy(file)
            .lines()
            .filter_map(|l| l.strip_prefix("usemtl "))
            .map(str::to_owned)
            .collect()
    }

    fn parts(file: &str) -> BTreeSet<String> {
        layer(file)
            .into_iter()
            .map(|(n, _)| n.split('.').nth(1).unwrap_or_default().to_owned())
            .collect()
    }

    #[test]
    fn the_layers_have_their_parts_and_stay_low_poly() {
        let muscles = materials("muscle.obj");
        for group in [
            "biceps",
            "triceps",
            "deltoid",
            "lats",
            "pecs",
            "glutes",
            "quads",
            "hamstrings",
            "calves",
        ] {
            assert!(muscles.contains(&format!("muscle_{group}")), "{group}");
        }
        let bones = parts("bone.obj");
        for part in [
            "skull",
            "vertebra_1",
            "rib_1",
            "pelvis",
            "scapula",
            "clavicle",
            "humerus",
            "radius",
            "ulna",
            "femur",
            "tibia",
            "fibula",
            "hand",
            "foot",
        ] {
            assert!(bones.contains(part), "{part}: {bones:?}");
        }
        let skin = parts("skin.obj");
        for part in ["head", "headband", "eye", "torso", "upper_arm", "forearm"] {
            assert!(skin.contains(part), "{part}: {skin:?}");
        }
        assert!(materials("bone.obj").iter().all(|m| m.starts_with("inner")));
        assert!(materials("skin.obj").iter().all(|m| m.starts_with("skin")));
        for file in ["skin.obj", "muscle.obj", "bone.obj"] {
            let triangles: usize = layer(file)
                .iter()
                .flat_map(|(_, m)| m.faces())
                .map(|f| f.indices.len() - 2)
                .sum();
            assert!(triangles <= 4000, "{file}: {triangles}");
        }
    }
}

// --- AC6: determinism and the golden ------------------------------------------------

const SIZE: (u32, u32) = (160, 90);
const CYAN: Rgb = Rgb {
    r: 34,
    g: 211,
    b: 238,
};

fn look(finish: &dyn Finish) -> Look<'_> {
    Look {
        finish,
        light: Light::new([-0.6, 0.6, 0.52]).expect("valid"),
        seed: 3,
    }
}

/// `meshes` under a trig-free orthographic three-quarter view, one PPM.
fn render(meshes: &[Mesh], name: &str, finish: &dyn Finish) -> Vec<u8> {
    let around = Motor3::from_unit_quaternion(0.96, 0.0, 0.28, 0.0);
    let down = Motor3::from_unit_quaternion(0.96, -0.28, 0.0, 0.0);
    let camera = Camera::orthographic(
        around
            .compose(&down)
            .compose(&Motor3::translator(0.0, 0.0, 5.0)),
    );
    let scene = scene_of(meshes, camera);
    let dir = tmp_dir(name);
    let mut sink = PpmSink::with_view(&dir, SIZE, (3.2, 1.8)).expect("ppm sink");
    sink.frame(0, &scene.eval_with(0.0, &look(finish)))
        .expect("frame");
    fs::read(dir.join("frame_00000.ppm")).expect("read frame")
}

mod ac6 {
    use super::*;

    /// A skin cube half peeled over a muscle box over a bone octahedron,
    /// swept half way from faceted to x-ray. Trig-free.
    fn golden(name: &str) -> Vec<u8> {
        let skin = boxed([-0.7; 3], [0.7; 3], ramp().with_layer(Layer::Skin));
        let muscle = boxed([-0.45; 3], [0.45; 3], rose().with_layer(Layer::Skin));
        let bone = Mesh::octahedron(0.35, bone()).expect("valid");
        let peeled = Peel::new(0.3, [0.0, 1.0, 0.0], (0.7, -0.7), 0.5)
            .expect("valid")
            .apply(&skin, 0.5)
            .expect("valid");
        let faceted = Faceted::new(3, 0.004).expect("valid");
        let xray = XRay::new(
            faceted,
            Glass {
                tint: CYAN,
                alpha: 0.2,
                edge_width: 0.004,
                edge_alpha: 0.3,
            },
            Glow {
                width: 0.02,
                alpha: 0.3,
            },
        )
        .expect("valid");
        let sweep = Sweep::new(
            &xray,
            &faceted,
            motoreel::Pt2 { x: 0.0, y: 0.0 },
            motoreel::Pt2 { x: 1.0, y: 0.0 },
        )
        .expect("valid");
        render(&[peeled, muscle, bone], name, &sweep)
    }

    #[test]
    fn two_renders_and_four_threads_are_byte_identical() {
        let reference = golden("r0021_twice_a");
        assert!(reference == golden("r0021_twice_b"));
        std::thread::scope(|s| {
            let hs: Vec<_> = (0..4)
                .map(|i| s.spawn(move || golden(&format!("r0021_threads_{i}"))))
                .collect();
            for h in hs {
                assert!(h.join().expect("thread") == reference);
            }
        });
    }

    #[test]
    fn the_ppm_golden_matches_byte_for_byte() {
        let got = golden("r0021_golden");
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/r0021_layers.ppm");
        if std::env::var_os("MOTOREEL_BLESS").is_some_and(|v| v == "1") {
            fs::write(&path, &got).expect("bless the golden fixture");
            return;
        }
        let want = fs::read(&path).unwrap_or_else(|_| {
            panic!(
                "{} is missing: MOTOREEL_BLESS=1 cargo test -p motoreel --test r0021_layers",
                path.display()
            )
        });
        assert!(want == got, "r0021_layers.ppm drifted");
    }

    #[test]
    fn the_peel_code_is_trig_free() {
        let code = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/peel.rs"))
            .expect("read source");
        for banned in [
            ".sin(", ".cos(", ".tan(", ".atan", ".exp(", ".ln(", ".powf(", "mul_add",
        ] {
            assert!(!code.contains(banned), "peel.rs uses {banned}");
        }
    }
}
