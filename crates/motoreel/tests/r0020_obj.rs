//! R-0020 — loading models («cargar modelos»): the acceptance suite, one
//! `mod` per criterion of `requirements/0020-load-models.md`.

use std::fs;
use std::path::{Path, PathBuf};

use garust::{pga, Motor3};
use motoreel::obj::layer_of;
use motoreel::{
    Axes, Camera, Faceted, FrameSink, Glass, Glow, Layer, Light, Look, Material, Mesh, MeshError,
    MeshFace, Obj, ObjError, ObjErrorKind, Object, PpmSink, Rgb, Scene, Sweep, XRay,
};
use proptest::prelude::*;

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

fn steel() -> Material {
    Material::new(&[rgb(120, 128, 150), rgb(168, 171, 179), rgb(205, 212, 228)]).expect("valid")
}

/// Binds every name to the ramp.
fn any(_: &str) -> Option<Material> {
    Some(ramp())
}

fn xyz(p: &pga::Point) -> [f64; 3] {
    let (x, y, z) = p.to_euclidean();
    [x, y, z]
}

fn parse(text: &str) -> Obj {
    Obj::parse(text, Axes::YUp).expect("valid OBJ")
}

/// `mesh` written as OBJ text, every number in Rust's shortest exact form.
fn to_obj(mesh: &Mesh) -> String {
    let mut out = String::from("o cube\nusemtl paint\n");
    for p in mesh.vertices() {
        let [x, y, z] = xyz(p);
        out += &format!("v {x} {y} {z}\n");
    }
    for f in mesh.faces() {
        let idx: Vec<String> = f.indices.iter().map(|i| (i + 1).to_string()).collect();
        out += &format!("f {}\n", idx.join(" "));
    }
    out
}

/// Twice the signed area of a triangle projected on z, summed.
fn area_z(mesh: &Mesh) -> f64 {
    mesh.faces()
        .iter()
        .map(|f| {
            let p: Vec<[f64; 3]> = f
                .indices
                .iter()
                .map(|&i| xyz(&mesh.vertices()[i]))
                .collect();
            (1..p.len() - 1)
                .map(|k| {
                    (p[k][0] - p[0][0]) * (p[k + 1][1] - p[0][1])
                        - (p[k][1] - p[0][1]) * (p[k + 1][0] - p[0][0])
                })
                .sum::<f64>()
        })
        .sum()
}

fn kind(text: &str) -> (usize, ObjErrorKind) {
    match Obj::parse(text, Axes::YUp) {
        Err(ObjError { line, kind }) => (line, kind),
        Ok(_) => panic!("accepted: {text:?}"),
    }
}

fn the_props() -> Vec<(&'static str, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/props");
    [
        "power_rack",
        "cable_machine",
        "flat_bench",
        "dumbbell",
        "barbell",
        "incline_bench",
        "preacher_bench",
    ]
    .into_iter()
    .map(|name| {
        let path = dir.join(format!("{name}.obj"));
        (name, fs::read_to_string(&path).expect("a committed prop"))
    })
    .collect()
}

// --- AC1: the statements read ------------------------------------------------

mod ac1 {
    use super::*;

    #[test]
    fn the_cube_round_trips_exactly() {
        let cube = Mesh::cube(0.35, ramp()).expect("valid");
        let loaded = parse(&to_obj(&cube)).mesh(any).expect("bound");
        assert_eq!(loaded, cube);
    }

    #[test]
    fn numbers_parse_correctly_rounded() {
        let mesh = parse("v 0.1 1e-3 -2.5E2\nv 1 0 0\nv 0 1 0\nf 1 2 3\n")
            .mesh(any)
            .expect("bound");
        assert_eq!(xyz(&mesh.vertices()[0]), [0.1, 0.001, -250.0]);
    }

    #[test]
    fn every_index_form_and_relative_indices_read_the_same() {
        let plain = "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n";
        let forms = [
            "v 0 0 0\nv 1 0 0\nv 0 1 0\nvt 0 0\nvn 0 0 1\nf 1/1 2/1 3/1\n",
            "v 0 0 0\nv 1 0 0\nv 0 1 0\nvn 0 0 1\nf 1//1 2//1 3//1\n",
            "v 0 0 0\nv 1 0 0\nv 0 1 0\nvt 0 0\nvn 0 0 1\nf 1/1/1 2/1/1 3/1/1\n",
            "v 0 0 0\nv 1 0 0\nv 0 1 0\nf -3 -2 -1\n",
            "# a comment\nmtllib x.mtl\ng group\ns 1\nv 0 0 0 # trailing\nv 1 0 0\nv 0 1 0\nf 1 2 3\n",
            "v 0 0 0 1 1 1\nv 1 0 0 1 1 1\nv 0 1 0 1 1 1\nf 1 2 3\n",
        ];
        let want = parse(plain).mesh(any).expect("bound");
        for text in forms {
            assert_eq!(parse(text).mesh(any).expect("bound"), want, "{text:?}");
        }
    }

    #[test]
    fn objects_become_separate_meshes_with_their_own_vertices() {
        let text = "v 9 9 9\no a\nv 0 0 0\nv 1 0 0\nv 0 1 0\nf 2 3 4\n\
                    o empty\no b\nv 0 0 1\nf 4 3 5\n";
        let obj = parse(text);
        assert_eq!(obj.object_names().collect::<Vec<_>>(), ["a", "b"]);
        let meshes = obj.meshes(any).expect("bound");
        assert_eq!(meshes.len(), 2);
        let (name, b) = &meshes[1];
        assert_eq!(name, "b");
        // Only the vertices b's face uses, in file order.
        let got: Vec<[f64; 3]> = b.vertices().iter().map(xyz).collect();
        assert_eq!(got, [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
        assert_eq!(b.faces(), [MeshFace::new(vec![1, 0, 2], 0)]);
    }
}

// --- AC2: faces --------------------------------------------------------------

mod ac2 {
    use super::*;

    #[test]
    fn a_convex_planar_ngon_stays_whole() {
        let hex = "v 2 0 0\nv 1 2 0\nv -1 2 0\nv -2 0 0\nv -1 -2 0\nv 1 -2 0\nf 1 2 3 4 5 6\n";
        let mesh = parse(hex).mesh(any).expect("bound");
        assert_eq!(mesh.faces(), [MeshFace::new(vec![0, 1, 2, 3, 4, 5], 0)]);
    }

    /// An L: six corners, one reflex (at vertex 4), counter-clockwise.
    const L: &str = "v 0 0 0\nv 2 0 0\nv 2 1 0\nv 1 1 0\nv 1 2 0\nv 0 2 0\nf 1 2 3 4 5 6\n";

    #[test]
    fn a_concave_ngon_is_ear_clipped_first_ear_first() {
        let mesh = parse(L).mesh(any).expect("bound");
        // Corner 0 is not an ear (the reflex corner 3 sits on its
        // diagonal), so the first ear is corner 1.
        assert_eq!(mesh.faces().len(), 4);
        assert_eq!(mesh.faces()[0].indices, [0, 1, 2]);
        // The triangles tile the L (area 3) and each keeps its winding.
        assert_eq!(area_z(&mesh), 6.0);
        for f in mesh.faces() {
            assert_eq!(f.indices.len(), 3);
            let one =
                Mesh::new(mesh.vertices().to_vec(), vec![f.clone()], vec![ramp()]).expect("valid");
            assert!(area_z(&one) > 0.0);
        }
    }

    #[test]
    fn clipping_is_deterministic_and_winding_preserving_reversed() {
        let reversed = "v 0 0 0\nv 2 0 0\nv 2 1 0\nv 1 1 0\nv 1 2 0\nv 0 2 0\nf 6 5 4 3 2 1\n";
        let a = parse(reversed).mesh(any).expect("bound");
        assert_eq!(a, parse(reversed).mesh(any).expect("bound"));
        assert_eq!(area_z(&a), -6.0);
    }

    #[test]
    fn straight_and_repeated_corners_are_dropped() {
        // A square with a midpoint on one edge and a repeated corner.
        let text = "v 0 0 0\nv 1 0 0\nv 2 0 0\nv 2 2 0\nv 0 2 0\nf 1 2 3 4 4 5\n";
        let mesh = parse(text).mesh(any).expect("bound");
        assert_eq!(mesh.faces().len(), 1);
        assert_eq!(mesh.faces()[0].indices.len(), 4);
    }

    #[test]
    fn a_bent_quad_becomes_two_triangles() {
        let text = "v 0 0 0\nv 1 0 0\nv 1 1 0.25\nv 0 1 0\nf 1 2 3 4\n";
        let mesh = parse(text).mesh(any).expect("bound");
        assert_eq!(mesh.faces().len(), 2);
        assert!(mesh.faces().iter().all(|f| f.indices.len() == 3));
    }
}

// --- AC3: axes and units -----------------------------------------------------

mod ac3 {
    use super::*;

    #[test]
    fn z_up_is_blender_and_y_up_is_the_identity() {
        let text = "v 1 2 3\nv 0 0 0\nv 1 0 0\nf 1 2 3\n";
        let z = Obj::parse(text, Axes::ZUp)
            .expect("valid")
            .mesh(any)
            .expect("bound");
        assert_eq!(xyz(&z.vertices()[0]), [1.0, 3.0, -2.0]);
        let y = parse(text).mesh(any).expect("bound");
        assert_eq!(xyz(&y.vertices()[0]), [1.0, 2.0, 3.0]);
    }

    #[test]
    fn blenders_front_faces_motoreels_camera() {
        // Blender's front face of a unit cube is the one at −y; motoreel's
        // default camera looks down −z from +z.
        let blender_front = "v 0 -1 0\nv 1 -1 0\nv 1 -1 1\nv 0 -1 1\nf 1 2 3 4\n";
        let mesh = Obj::parse(blender_front, Axes::ZUp)
            .expect("valid")
            .mesh(any)
            .expect("bound");
        let p: Vec<[f64; 3]> = mesh.vertices().iter().map(xyz).collect();
        let (u, v) = (
            [p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]],
            [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]],
        );
        let nz = u[0] * v[1] - u[1] * v[0];
        assert!(nz > 0.0, "the front face's normal points at +z");
        assert!(p.iter().all(|q| q[2] == 1.0));
    }
}

// --- AC4: materials ----------------------------------------------------------

mod ac4 {
    use super::*;

    const TWO: &str = "v 0 0 0\nv 1 0 0\nv 0 1 0\nv 1 1 0\n\
                       usemtl steel\nf 1 2 3\nusemtl inner_stack\nf 2 4 3\nusemtl steel\nf 1 3 2\n";

    #[test]
    fn names_bind_to_material_slots_in_usemtl_order() {
        let mesh = parse(TWO)
            .mesh(|name| match name {
                "steel" => Some(steel()),
                "inner_stack" => Some(ramp()),
                _ => None,
            })
            .expect("bound");
        let slots: Vec<usize> = mesh.faces().iter().map(|f| f.material).collect();
        assert_eq!(slots, [0, 1, 0]);
        assert_eq!(mesh.materials()[0], steel());
        assert_eq!(mesh.materials()[1], ramp().with_layer(Layer::Inner));
    }

    #[test]
    fn the_name_sets_the_layer() {
        assert_eq!(layer_of("inner_stack"), Some(Layer::Inner));
        assert_eq!(layer_of("inner"), Some(Layer::Inner));
        assert_eq!(layer_of("skin.001"), Some(Layer::Skin));
        assert_eq!(layer_of("skin_ink"), Some(Layer::Skin));
        assert_eq!(layer_of("steel"), None);
        assert_eq!(layer_of("innertube"), None);
        // Without a tag, the bound material keeps its own layer.
        let mesh = parse("v 0 0 0\nv 1 0 0\nv 0 1 0\nusemtl paint\nf 1 2 3\n")
            .mesh(|_| Some(ramp().with_layer(Layer::Skin)))
            .expect("bound");
        assert_eq!(mesh.materials()[0].layer(), Layer::Skin);
    }

    #[test]
    fn an_unbound_name_is_refused_at_its_first_face() {
        let got = parse(TWO).mesh(|name| (name == "steel").then(steel));
        assert_eq!(
            got,
            Err(ObjError {
                line: 8,
                kind: ObjErrorKind::Unbound("inner_stack".into())
            })
        );
    }

    #[test]
    fn faces_before_any_usemtl_are_the_empty_name() {
        let got = parse("v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n").mesh(|name| {
            assert_eq!(name, "");
            None
        });
        assert_eq!(
            got.map_err(|e| e.kind),
            Err(ObjErrorKind::Unbound(String::new()))
        );
    }
}

// --- AC5: errors, never panics -----------------------------------------------

mod ac5 {
    use super::*;

    #[test]
    fn each_malformed_line_is_named() {
        let tri = "v 0 0 0\nv 1 0 0\nv 0 1 0\n";
        let cases: [(String, usize, ObjErrorKind); 11] = [
            ("v 1 2\n".into(), 1, ObjErrorKind::VertexArity(2)),
            ("v 1 two 3\n".into(), 1, ObjErrorKind::Number("two".into())),
            ("\nv 1 inf 3\n".into(), 2, ObjErrorKind::NonFinite),
            ("v 1 NaN 3\n".into(), 1, ObjErrorKind::NonFinite),
            (format!("{tri}f 1 2 4\n"), 4, ObjErrorKind::Index(4)),
            (format!("{tri}f 0 1 2\n"), 4, ObjErrorKind::Index(0)),
            (format!("{tri}f -4 1 2\n"), 4, ObjErrorKind::Index(-4)),
            (format!("{tri}f 1 2\n"), 4, ObjErrorKind::TooFewIndices(2)),
            (
                format!("{tri}l 1 2\n"),
                4,
                ObjErrorKind::Unsupported("l".into()),
            ),
            (format!("{tri}usemtl\n"), 4, ObjErrorKind::MissingName),
            (
                format!("{tri}f 1 2 x\n"),
                4,
                ObjErrorKind::Number("x".into()),
            ),
        ];
        for (text, line, want) in cases {
            assert_eq!(kind(&text), (line, want), "{text:?}");
        }
    }

    #[test]
    fn a_face_with_no_area_is_refused() {
        let text = "v 0 0 0\nv 1 0 0\nv 2 0 0\nf 1 2 3\n";
        assert_eq!(kind(text), (4, ObjErrorKind::Degenerate));
    }

    #[test]
    fn a_self_crossing_face_is_refused() {
        // A bow tie: two triangles meeting at a crossing.
        let text = "v 0 0 0\nv 2 2 0\nv 2 0 0\nv 0 2 0\nf 1 2 3 4\n";
        let (line, k) = kind(text);
        assert_eq!(line, 5);
        assert!(
            matches!(k, ObjErrorKind::NotSimple | ObjErrorKind::Degenerate),
            "{k:?}"
        );
    }

    #[test]
    fn errors_read_with_their_line() {
        let e = ObjError {
            line: 7,
            kind: ObjErrorKind::Mesh(MeshError::NonConvex(0)),
        };
        assert_eq!(e.to_string(), "line 7: face 0: not strictly convex");
    }

    proptest! {
        /// Any text, however mangled, is a value or an error: no panic.
        #[test]
        fn arbitrary_text_never_panics(text in "([vfo] ?(-?[0-9]{0,2}(/[0-9]?){0,2} ?){0,6}\n|usemtl [a-z_]{0,6}\n|#.*\n){0,12}") {
            if let Ok(obj) = Obj::parse(&text, Axes::ZUp) {
                let _ = obj.meshes(any);
            }
        }
    }
}

// --- AC6: a loaded mesh renders like the hand-built one -------------------------

mod ac6 {
    use super::*;

    const SIZE: (u32, u32) = (160, 90);
    const CYAN: Rgb = Rgb {
        r: 34,
        g: 211,
        b: 238,
    };

    fn xray() -> XRay {
        XRay::new(
            Faceted::new(3, 0.004).expect("valid"),
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
        .expect("valid")
    }

    /// A skin cube over an inner octahedron, both as Blender would write
    /// them (z up), and the same two meshes built by hand in motoreel's
    /// frame.
    fn loaded() -> Vec<Mesh> {
        let text = "o shell\n\
            v -1 -1 -1\nv 1 -1 -1\nv -1 1 -1\nv 1 1 -1\nv -1 -1 1\nv 1 -1 1\nv -1 1 1\nv 1 1 1\n\
            usemtl skin_body\n\
            f 1 3 4 2\nf 5 6 8 7\nf 1 2 6 5\nf 3 7 8 4\nf 1 5 7 3\nf 2 4 8 6\n\
            o bone\n\
            v 0.5 0 0\nv -0.5 0 0\nv 0 0.5 0\nv 0 -0.5 0\nv 0 0 0.5\nv 0 0 -0.5\n\
            usemtl inner_bone\n\
            f 9 11 13\nf 11 10 13\nf 10 12 13\nf 12 9 13\nf 11 9 14\nf 10 11 14\nf 12 10 14\nf 9 12 14\n";
        Obj::parse(text, Axes::ZUp)
            .expect("valid")
            .meshes(|_| Some(ramp()))
            .expect("bound")
            .into_iter()
            .map(|(_, m)| m)
            .collect()
    }

    fn by_hand() -> Vec<Mesh> {
        let blender = |p: [f64; 3]| pga::Point::new(p[0], p[2], 0.0 - p[1]);
        let cube: Vec<pga::Point> = (0..8)
            .map(|i| {
                let s = |bit: usize| if i & bit == 0 { -1.0 } else { 1.0 };
                blender([s(1), s(2), s(4)])
            })
            .collect();
        let quads = [
            [0, 2, 3, 1],
            [4, 5, 7, 6],
            [0, 1, 5, 4],
            [2, 6, 7, 3],
            [0, 4, 6, 2],
            [1, 3, 7, 5],
        ];
        let shell = Mesh::new(
            cube,
            quads.iter().map(|q| MeshFace::new(q.to_vec(), 0)).collect(),
            vec![ramp().with_layer(Layer::Skin)],
        )
        .expect("valid");
        let octa = [
            [0.5, 0.0, 0.0],
            [-0.5, 0.0, 0.0],
            [0.0, 0.5, 0.0],
            [0.0, -0.5, 0.0],
            [0.0, 0.0, 0.5],
            [0.0, 0.0, -0.5],
        ]
        .map(blender)
        .to_vec();
        let tris = [
            [0, 2, 4],
            [2, 1, 4],
            [1, 3, 4],
            [3, 0, 4],
            [2, 0, 5],
            [1, 2, 5],
            [3, 1, 5],
            [0, 3, 5],
        ];
        let bone = Mesh::new(
            octa,
            tris.iter().map(|t| MeshFace::new(t.to_vec(), 0)).collect(),
            vec![ramp().with_layer(Layer::Inner)],
        )
        .expect("valid");
        vec![shell, bone]
    }

    fn render(meshes: Vec<Mesh>, name: &str, finish: &dyn motoreel::Finish) -> Vec<u8> {
        let around = Motor3::from_unit_quaternion(0.96, 0.0, 0.28, 0.0);
        let down = Motor3::from_unit_quaternion(0.96, -0.28, 0.0, 0.0);
        let mut scene = Scene::new(1.0);
        scene.camera = Camera::orthographic(
            around
                .compose(&down)
                .compose(&Motor3::translator(0.0, 0.0, 5.0)),
        );
        for m in meshes {
            scene.add(Object::mesh(m));
        }
        let look = Look {
            finish,
            light: Light::new([-0.6, 0.6, 0.52]).expect("valid"),
            seed: 3,
        };
        let dir = tmp_dir(name);
        let mut sink = PpmSink::with_view(&dir, SIZE, (3.2, 1.8)).expect("ppm sink");
        sink.frame(0, &scene.eval_with(0.0, &look)).expect("frame");
        fs::read(dir.join("frame_00000.ppm")).expect("read frame")
    }

    #[test]
    fn the_loaded_meshes_equal_the_hand_built_ones() {
        assert_eq!(loaded(), by_hand());
    }

    #[test]
    fn faceted_xray_and_a_sweep_render_the_same_bytes() {
        let faceted = Faceted::new(3, 0.004).expect("valid");
        let xray = xray();
        let sweep = Sweep::new(
            &xray,
            &faceted,
            motoreel::Pt2 { x: 0.0, y: 0.0 },
            motoreel::Pt2 { x: 1.0, y: 0.0 },
        )
        .expect("valid");
        let finishes: [(&str, &dyn motoreel::Finish); 3] =
            [("faceted", &faceted), ("xray", &xray), ("sweep", &sweep)];
        for (name, finish) in finishes {
            let a = render(loaded(), &format!("r0020_ac6_{name}_loaded"), finish);
            let b = render(by_hand(), &format!("r0020_ac6_{name}_hand"), finish);
            assert!(a == b, "{name}: the loaded render differs");
        }
    }
}

// --- AC7: determinism ---------------------------------------------------------

mod ac7 {
    use super::*;

    #[test]
    fn two_parses_and_two_renders_are_identical() {
        for (name, text) in the_props() {
            let a = Obj::parse(&text, Axes::ZUp).expect("valid").meshes(any);
            let b = Obj::parse(&text, Axes::ZUp).expect("valid").meshes(any);
            assert_eq!(a, b, "{name}");
        }
        let render = |dir: &str| {
            let (_, text) = &the_props()[0];
            let mut scene = Scene::new(1.0);
            scene.camera = Camera::pinhole(
                Motor3::translator(0.0, 1.1, 0.0).compose(&Motor3::translator(0.0, 0.0, 6.0)),
                40.0,
            );
            for (_, mesh) in Obj::parse(text, Axes::ZUp)
                .expect("valid")
                .meshes(any)
                .expect("bound")
            {
                scene.add(Object::mesh(mesh));
            }
            let finish = Faceted::new(3, 0.01).expect("valid");
            let look = Look {
                finish: &finish,
                light: Light::new([-0.6, 0.6, 0.52]).expect("valid"),
                seed: 0,
            };
            let dir = tmp_dir(dir);
            let mut sink = PpmSink::with_view(&dir, (90, 160), (9.0, 16.0)).expect("sink");
            sink.frame(0, &scene.eval_with(0.0, &look)).expect("frame");
            fs::read(dir.join("frame_00000.ppm")).expect("read")
        };
        assert!(render("r0020_ac7_a") == render("r0020_ac7_b"));
    }
}

// --- AC8: the committed props ---------------------------------------------------

mod ac8 {
    use super::*;

    /// The names `tools/blender/props.py` writes, and nothing else.
    const NAMES: [&str; 8] = [
        "ink",
        "skin_ink",
        "steel",
        "chrome",
        "pad",
        "rubber",
        "skin_shroud",
        "inner_stack",
    ];

    #[test]
    fn every_prop_loads_low_poly_with_its_hulls_first() {
        for (name, text) in the_props() {
            let obj = Obj::parse(&text, Axes::ZUp).unwrap_or_else(|e| panic!("{name}: {e}"));
            let meshes = obj
                .meshes(|m| NAMES.contains(&m).then(ramp))
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            let triangles: usize = meshes
                .iter()
                .flat_map(|(_, m)| m.faces())
                .map(|f| f.indices.len() - 2)
                .sum();
            assert!(triangles <= 1500, "{name}: {triangles} triangles");
            // Each `_ink` hull is followed by its part.
            for pair in meshes.windows(2) {
                if let Some(stem) = pair[0].0.strip_suffix("_ink") {
                    let part = stem.replacen('a', "b", 1);
                    assert_eq!(pair[1].0, part, "{name}");
                }
            }
        }
    }

    #[test]
    fn the_cable_machine_marks_its_inside() {
        let (_, text) = the_props()
            .into_iter()
            .find(|(n, _)| *n == "cable_machine")
            .expect("the cable machine");
        let meshes = Obj::parse(&text, Axes::ZUp)
            .expect("valid")
            .meshes(any)
            .expect("bound");
        let layers = |part: &str| -> Vec<Layer> {
            meshes
                .iter()
                .filter(|(n, _)| n.ends_with(part))
                .map(|(_, m)| m.materials()[0].layer())
                .collect()
        };
        assert_eq!(layers("_shroud"), [Layer::Skin]);
        assert_eq!(layers("_plate"), [Layer::Inner; 8]);
    }
}
