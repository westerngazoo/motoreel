//! R-0019 — the x-ray finish and its sweep («rayos X», «barrido»): the
//! acceptance suite, one `mod` per criterion of `requirements/0019-x-ray.md`.

use std::fs;
use std::path::{Path, PathBuf};

use garust::Motor3;
use motoreel::shapes::polygon;
use motoreel::{
    Camera, Faceted, Facing, Fill, Finish, FrameCtx, FrameSink, Glass, Glow, Layer, Light, Look,
    Material, Mesh, Object, Paint, PpmSink, Prim2, Pt2, Rgb, Scene, Style, Surface, SvgSink, Sweep,
    SweepError, XRay, XRayError,
};

// --- Shared helpers ---------------------------------------------------------

fn tmp_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clean the stale test directory");
    }
    dir
}

fn pt(x: f64, y: f64) -> Pt2 {
    Pt2 { x, y }
}

fn rgb(r: u8, g: u8, b: u8) -> Rgb {
    Rgb { r, g, b }
}

const CTX: FrameCtx = FrameCtx { t: 0.0, seed: 5 };
const CYAN: Rgb = Rgb {
    r: 34,
    g: 211,
    b: 238,
};

fn light() -> Light {
    Light::new([-0.6, 0.6, 0.52]).expect("valid")
}

fn faceted(stroke: f64) -> Faceted {
    Faceted::new(3, stroke).expect("valid")
}

fn glass() -> Glass {
    Glass {
        tint: CYAN,
        alpha: 0.2,
        edge_width: 0.0,
        edge_alpha: 0.0,
    }
}

fn glow() -> Glow {
    Glow {
        width: 0.25,
        alpha: 0.4,
    }
}

fn xray(stroke: f64) -> XRay {
    XRay::new(faceted(stroke), glass(), glow()).expect("valid")
}

fn tones() -> [Rgb; 3] {
    [rgb(18, 110, 168), rgb(38, 190, 230), rgb(160, 240, 255)]
}

fn solid() -> Material {
    Material::new(&tones()).expect("valid")
}

fn skin() -> Material {
    solid().with_layer(Layer::Skin)
}

fn bone() -> Material {
    Material::new(&[rgb(200, 200, 210), rgb(240, 240, 250)])
        .expect("valid")
        .with_layer(Layer::Inner)
}

fn surface(pts: &[Pt2], n: [f64; 3], id: u64, m: Material) -> Surface {
    Surface::new(polygon(pts), Facing::Normal(n), 0.0, id, m).expect("valid")
}

/// A canvas with an exact pixel grid: 256 × 128 over a 4 × 2 view.
fn raster(prims: &[Prim2], name: &str) -> Vec<u8> {
    let dir = tmp_dir(name);
    let mut sink = PpmSink::with_view(&dir, (256, 128), (4.0, 2.0)).expect("sink");
    sink.frame(0, prims).expect("frame");
    let bytes = fs::read(dir.join("frame_00000.ppm")).expect("read");
    bytes[bytes.len() - 256 * 128 * 3..].to_vec()
}

/// The pixel whose centre is image point `(x, y)` on [`raster`]'s canvas.
fn px(img: &[u8], x: f64, y: f64) -> [u8; 3] {
    let col = ((x + 2.0) * 64.0) as usize;
    let row = ((1.0 - y) * 64.0) as usize;
    let i = (row * 256 + col) * 3;
    [img[i], img[i + 1], img[i + 2]]
}

/// An octagon fanned into eight triangles about its centre, CCW.
fn fan(r: f64, n: [f64; 3], m: Material) -> Vec<Surface> {
    let h = r * std::f64::consts::FRAC_1_SQRT_2;
    let ring = [
        pt(r, 0.0),
        pt(h, h),
        pt(0.0, r),
        pt(-h, h),
        pt(-r, 0.0),
        pt(-h, -h),
        pt(0.0, -r),
        pt(h, -h),
    ];
    (0..8)
        .map(|k| surface(&[pt(0.0, 0.0), ring[k], ring[(k + 1) % 8]], n, k as u64, m))
        .collect()
}

// --- AC1: construction ---------------------------------------------------------

mod ac1 {
    use super::*;

    #[test]
    fn every_bad_field_is_named() {
        let f = faceted(0.01);
        let bad_glass = |g: Glass| XRay::new(f, g, glow());
        assert_eq!(
            bad_glass(Glass {
                alpha: 1.5,
                ..glass()
            }),
            Err(XRayError::GlassAlpha(1.5))
        );
        assert_eq!(
            bad_glass(Glass {
                edge_width: -1.0,
                ..glass()
            }),
            Err(XRayError::EdgeWidth(-1.0))
        );
        assert_eq!(
            bad_glass(Glass {
                edge_alpha: -0.5,
                ..glass()
            }),
            Err(XRayError::EdgeAlpha(-0.5))
        );
        assert_eq!(
            XRay::new(
                f,
                glass(),
                Glow {
                    width: f64::INFINITY,
                    ..glow()
                }
            ),
            Err(XRayError::GlowWidth(f64::INFINITY))
        );
        assert_eq!(
            XRay::new(
                f,
                glass(),
                Glow {
                    alpha: 2.0,
                    ..glow()
                }
            ),
            Err(XRayError::GlowAlpha(2.0))
        );
        assert!(XRay::new(f, glass(), glow()).is_ok());
    }

    #[test]
    fn a_bad_sweep_is_refused() {
        let (a, b) = (xray(0.01), faceted(0.01));
        assert_eq!(
            Sweep::new(&a, &b, pt(f64::NAN, 0.0), pt(1.0, 0.0)).err(),
            Some(SweepError::At)
        );
        for toward in [pt(0.0, 0.0), pt(f64::INFINITY, 0.0)] {
            assert_eq!(
                Sweep::new(&a, &b, pt(0.0, 0.0), toward).err(),
                Some(SweepError::Toward)
            );
        }
        assert!(Sweep::new(&a, &b, pt(0.0, 0.0), pt(1.0, 0.0)).is_ok());
    }
}

// --- AC2: props keep the faceted look ---------------------------------------------

mod ac2 {
    use super::*;

    #[test]
    fn solid_surfaces_are_painted_exactly_as_faceted_paints_them() {
        let tri = |dx: f64| [pt(dx, 0.0), pt(dx + 0.3, 0.0), pt(dx, 0.3)];
        let batch = [
            surface(&tri(-1.0), [0.2, 0.3, 1.0], 0, solid()),
            surface(&tri(-0.5), [0.0, 0.0, 1.0], 1, skin()),
            surface(&tri(0.0), [-0.5, 0.5, 0.6], 2, solid()),
            surface(&tri(0.5), [0.5, -0.5, 0.6], 3, bone()),
            surface(&tri(1.0), [0.6, 0.1, 0.4], 4, solid()),
        ];
        let f = faceted(0.01).paint(&batch, &light(), &CTX);
        assert_eq!(f.len(), 4, "the bone is hidden under Faceted");
        let x = xray(0.01).paint(&batch, &light(), &CTX);
        // XRay: solid, glass, solid, glow, bone core, solid.
        assert_eq!(x.len(), 6);
        assert_eq!((&x[0], &x[2], &x[5]), (&f[0], &f[2], &f[3]));
    }
}

// --- AC3: the skin is tinted glass, tinted once ----------------------------------------

mod ac3 {
    use super::*;

    #[test]
    fn a_skin_run_is_one_primitive() {
        let batch = fan(0.6, [0.0, 0.0, 1.0], skin());
        let out = xray(0.01).paint(&batch, &light(), &CTX);
        assert_eq!(out.len(), 1);
        let Prim2::Path { subpaths, style } = &out[0] else {
            panic!("a path");
        };
        assert_eq!(subpaths.len(), 8, "every face's outline");
        assert_eq!(style.fill, Some(Fill::solid(CYAN, 0.2)));
    }

    /// Over black, every pixel well inside the fan has the one-layer tint,
    /// along the shared edges too. Painted face by face, the shared edges
    /// would not: the check has teeth.
    #[test]
    fn neighbouring_faces_are_not_tinted_twice() {
        let batch = fan(0.8, [0.0, 0.0, 1.0], skin());
        let once = raster(&xray(0.01).paint(&batch, &light(), &CTX), "r0019_ac3_glass");
        let want = [7, 42, 48]; // round(0.2 · CYAN)
        let mut by_face = Vec::new();
        for s in &batch {
            by_face.extend(xray(0.01).paint(std::slice::from_ref(s), &light(), &CTX));
        }
        let twice = raster(&by_face, "r0019_ac3_by_face");
        let mut differs = false;
        for i in 0..40 {
            for j in 0..40 {
                let (x, y) = (-0.5 + f64::from(i) / 40.0, -0.5 + f64::from(j) / 40.0);
                assert_eq!(px(&once, x, y), want, "at ({x}, {y})");
                differs |= px(&twice, x, y) != want;
            }
        }
        assert!(differs, "face-by-face glass shows its seams");
    }
}

// --- AC4: the inside shows only under x-ray --------------------------------------------

mod ac4 {
    use super::*;

    #[test]
    fn inner_parts_are_opaque_banded_and_x_ray_only() {
        let batch = fan(0.5, [0.3, 0.2, 1.0], bone());
        assert!(faceted(0.01).paint(&batch, &light(), &CTX).is_empty());
        let out = xray(0.01).paint(&batch, &light(), &CTX);
        assert_eq!(out.len(), 1 + 8, "one glow, then eight cores");
        let ramp = bone().tones().to_vec();
        for p in &out[1..] {
            let Prim2::Path { style, .. } = p else {
                panic!("a path");
            };
            let fill = style.fill.expect("a fill");
            let Paint::Flat(c) = fill.paint else {
                panic!("flat");
            };
            assert_eq!(fill.alpha, 1.0);
            assert!(ramp.contains(&c));
        }
    }
}

// --- AC5: the glow does not accumulate ----------------------------------------------------

mod ac5 {
    use super::*;

    fn white_bone() -> Material {
        Material::new(&[Rgb::WHITE])
            .expect("valid")
            .with_layer(Layer::Inner)
    }

    /// Outside the core, no pixel is brighter than one layer of glow; with
    /// a glow per face the halo stacks where neighbours meet.
    #[test]
    fn the_halo_is_one_layer_everywhere() {
        let batch = fan(0.5, [0.0, 0.0, 1.0], white_bone());
        let ours = xray(0.01).paint(&batch, &light(), &CTX);
        let one_layer = (255.0_f64 * 0.4).round() as u8;
        let img = raster(&ours, "r0019_ac5_glow");
        let mut per_face = Vec::new();
        for s in &batch {
            per_face.extend(xray(0.01).paint(std::slice::from_ref(s), &light(), &CTX));
        }
        let stacked = raster(&per_face, "r0019_ac5_per_face");
        let (mut lit, mut brighter) = (0, 0);
        for i in 0..256 {
            for j in 0..128 {
                let (x, y) = (
                    -2.0 + (f64::from(i) + 0.5) / 64.0,
                    1.0 - (f64::from(j) + 0.5) / 64.0,
                );
                let r = (x * x + y * y).sqrt();
                if !(0.52..0.7).contains(&r) {
                    continue; // the core, or beyond the halo
                }
                let v = px(&img, x, y)[0];
                assert!(v <= one_layer, "{v} at ({x}, {y})");
                lit += usize::from(v == one_layer);
                brighter += usize::from(px(&stacked, x, y)[0] > one_layer);
            }
        }
        assert!(lit > 100, "the halo is there");
        assert!(brighter > 0, "a glow per face stacks: the check has teeth");
    }
}

// --- AC6: the sweep ------------------------------------------------------------------------

mod ac6 {
    use super::*;

    fn batch() -> Vec<Surface> {
        let mut b = fan(0.5, [0.1, 0.2, 1.0], skin());
        b.extend(fan(0.2, [0.3, -0.2, 1.0], bone()).into_iter().map(|s| {
            Surface::new(
                s.outline().to_vec(),
                s.facing(),
                0.0,
                s.id() + 100,
                *s.material(),
            )
            .expect("valid")
        }));
        b.push(surface(
            &[pt(0.7, -0.3), pt(1.2, -0.3), pt(1.2, 0.3), pt(0.7, 0.3)],
            [0.2, 0.1, 1.0],
            200,
            solid(),
        ));
        b
    }

    #[test]
    fn a_line_past_everything_is_one_finish() {
        let (x, f) = (xray(0.01), faceted(0.01));
        let all = batch();
        let left = Sweep::new(&x, &f, pt(-5.0, 0.0), pt(1.0, 0.0)).expect("valid");
        assert_eq!(
            left.paint(&all, &light(), &CTX),
            f.paint(&all, &light(), &CTX)
        );
        let right = Sweep::new(&x, &f, pt(5.0, 0.0), pt(1.0, 0.0)).expect("valid");
        assert_eq!(
            right.paint(&all, &light(), &CTX),
            x.paint(&all, &light(), &CTX)
        );
    }

    /// The faceted tones do not change while the line moves: both
    /// finishes always see the whole batch.
    #[test]
    fn tones_hold_while_the_line_moves() {
        let (x, f) = (xray(0.01), faceted(0.01));
        let all = batch();
        let colours = |line: f64| {
            let s = Sweep::new(&x, &f, pt(line, 0.0), pt(1.0, 0.0)).expect("valid");
            let mut c: Vec<(u8, u8, u8)> = s
                .paint(&all, &light(), &CTX)
                .iter()
                .filter_map(|p| match p {
                    Prim2::Path { style, .. } => match style.fill?.paint {
                        Paint::Flat(c) if style.fill?.alpha == 1.0 => Some((c.r, c.g, c.b)),
                        _ => None,
                    },
                    _ => None,
                })
                .collect();
            c.sort_unstable();
            c.dedup();
            c
        };
        let reference = colours(-5.0);
        for k in 0..20 {
            let line = -1.0 + f64::from(k) * 0.1;
            for c in colours(line) {
                let core = bone().tones().iter().any(|t| (t.r, t.g, t.b) == c);
                assert!(core || reference.contains(&c), "{c:?} at {line}");
            }
        }
    }

    /// Where the line cuts an opaque face, the two halves meet with no
    /// background between them: the same finish on both sides paints the
    /// same pixels as no sweep at all, the cut column included.
    #[test]
    fn the_halves_meet_without_a_gap() {
        let f = faceted(2.0 / 64.0);
        let face = [surface(
            &[pt(-0.8, -0.6), pt(0.8, -0.6), pt(0.8, 0.6), pt(-0.8, 0.6)],
            [0.0, 0.0, 1.0],
            0,
            solid(),
        )];
        let whole = raster(&f.paint(&face, &light(), &CTX), "r0019_ac6_whole");
        let cut = Sweep::new(&f, &f, pt(0.1, 0.0), pt(1.0, 0.3)).expect("valid");
        let split = raster(&cut.paint(&face, &light(), &CTX), "r0019_ac6_split");
        for j in 0..40 {
            let y = -0.5 + f64::from(j) / 40.0;
            for i in 0..60 {
                let x = -0.7 + f64::from(i) / 40.0;
                assert_eq!(px(&split, x, y), px(&whole, x, y), "at ({x}, {y})");
            }
        }
    }
}

// --- AC7: determinism and the goldens ---------------------------------------------------------

mod ac7 {
    use super::*;

    const SIZE: (u32, u32) = (160, 90);

    /// A skin cube with an inner octahedron and a solid prop beside it,
    /// swept halfway, orthographic and trig-free.
    fn frame() -> Vec<Prim2> {
        let mut scene = Scene::new(1.0);
        let around = Motor3::from_unit_quaternion(0.96, 0.0, 0.28, 0.0);
        let down = Motor3::from_unit_quaternion(0.96, -0.28, 0.0, 0.0);
        scene.camera = Camera::orthographic(
            around
                .compose(&down)
                .compose(&Motor3::translator(0.0, 0.0, 5.0)),
        );
        scene.add(Object::mesh(Mesh::cube(0.5, skin()).expect("valid")));
        scene.add(Object::mesh(Mesh::octahedron(0.35, bone()).expect("valid")));
        scene.add(
            Object::mesh(Mesh::cube(0.25, solid()).expect("valid"))
                .at(Motor3::translator(1.0, -0.25, 0.0)),
        );
        let (x, f) = (
            XRay::new(
                faceted(0.02),
                Glass {
                    edge_width: 0.02,
                    edge_alpha: 0.5,
                    ..glass()
                },
                Glow {
                    width: 0.12,
                    alpha: 0.35,
                },
            )
            .expect("valid"),
            faceted(0.02),
        );
        let sweep = Sweep::new(&x, &f, pt(0.25, 0.0), pt(1.0, 0.0)).expect("valid");
        let look = Look {
            finish: &sweep,
            light: light(),
            seed: 0,
        };
        scene.eval_with(0.0, &look)
    }

    fn render(name: &str, svg: bool) -> Vec<u8> {
        let dir = tmp_dir(name);
        let prims = frame();
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
                "{} is missing: MOTOREEL_BLESS=1 cargo test -p motoreel --test r0019_xray",
                path.display()
            )
        });
        assert!(want == got, "{fixture} drifted");
    }

    #[test]
    fn two_renders_and_four_threads_are_byte_identical() {
        for svg in [false, true] {
            assert!(
                render(&format!("r0019_twice_a_{svg}"), svg)
                    == render(&format!("r0019_twice_b_{svg}"), svg)
            );
        }
        let reference = render("r0019_threads_ref", false);
        std::thread::scope(|s| {
            let hs: Vec<_> = (0..4)
                .map(|i| s.spawn(move || render(&format!("r0019_threads_{i}"), false)))
                .collect();
            for h in hs {
                assert!(h.join().expect("thread") == reference);
            }
        });
    }

    #[test]
    fn the_ppm_golden_matches_byte_for_byte() {
        check_golden("r0019_xray.ppm", &render("r0019_ppm_golden", false));
    }

    #[test]
    fn the_svg_golden_matches_byte_for_byte() {
        check_golden("r0019_xray.svg", &render("r0019_svg_golden", true));
    }

    #[test]
    fn the_x_ray_code_is_trig_free() {
        let code = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/xray.rs"))
            .expect("read source");
        for banned in [
            ".sin(", ".cos(", ".tan(", ".atan", ".exp(", ".ln(", ".powf(", "mul_add",
        ] {
            assert!(!code.contains(banned), "xray.rs uses {banned}");
        }
    }

    /// The glow keeps its style when a sweep cuts it: clipping changes
    /// outlines, never paint.
    #[test]
    fn clipping_keeps_styles() {
        let styles: Vec<Style> = frame()
            .iter()
            .filter_map(|p| match p {
                Prim2::Path { style, .. } => Some(*style),
                _ => None,
            })
            .collect();
        assert!(styles.iter().any(|s| s.fill.is_none() && s.width == 0.12));
    }
}
