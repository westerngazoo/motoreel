//! R-0016 — gradient fills: the acceptance suite (e2e).
//!
//! Written from `requirements/0016-gradient-fills.md` (AC1–AC10) and
//! SPEC-0016 §6 before the implementation existed, so it went red at
//! compile time, as R-0012's suite did. One `mod` per criterion; each
//! test's header comment says what it pins.
//!
//! **Canvases with a power-of-two scale.** Most raster tests use a
//! 256 × 128 canvas over a 4 × 2 view, 64 px per unit, so every pixel
//! centre has a binary-exact image coordinate. That is what lets AC3 and
//! AC5 assert exact equality of colours rather than a tolerance.

use std::fs;
use std::path::{Path, PathBuf};

use garust::{pga, Motor3};
use motoreel::shapes::{circle, polygon, rounded_rect};
use motoreel::{
    Camera, Fill, FrameSink, Gradient, GradientError, GradientKind, Object, Paint, PpmSink, Prim2,
    Pt2, Rgb, Scene, Stop, Style, Subpath, SvgSink, Track,
};

// --- Shared helpers ---------------------------------------------------------

/// A clean per-test scratch directory under `CARGO_TARGET_TMPDIR`.
fn tmp_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clean the stale test directory");
    }
    dir
}

fn p(x: f64, y: f64) -> Pt2 {
    Pt2 { x, y }
}

const BLACK: Rgb = Rgb::BLACK;
const RED: Rgb = Rgb {
    r: 0xff,
    g: 0x00,
    b: 0x00,
};
const BLUE: Rgb = Rgb {
    r: 0x10,
    g: 0x40,
    b: 0xf0,
};
/// The R-0012 golden palette (SPEC-0012 §2.10), reused by AC6.
const GOLD: Rgb = Rgb {
    r: 0xe9,
    g: 0xb2,
    b: 0x3f,
};
const INK: Rgb = Rgb {
    r: 0x14,
    g: 0x14,
    b: 0x17,
};
const STEEL: Rgb = Rgb {
    r: 0xce,
    g: 0xd1,
    b: 0xd9,
};
const SIGNAL: Rgb = Rgb {
    r: 0xe0,
    g: 0x32,
    b: 0x2a,
};

fn stop(offset: f64, colour: Rgb, alpha: f64) -> Stop {
    Stop {
        offset,
        colour,
        alpha,
    }
}

/// Fill only, no stroke: SPEC-0012 §2.7's spelling.
fn fill_style(fill: Fill) -> Style {
    Style {
        stroke: BLACK,
        width: 0.0,
        alpha: 1.0,
        fill: Some(fill),
    }
}

fn path_prim(subpaths: Vec<Subpath<Pt2>>, fill: Fill) -> Prim2 {
    Prim2::Path {
        subpaths,
        style: fill_style(fill),
    }
}

/// The axis-aligned rectangle from `(x0, y0)` to `(x1, y1)`, closed.
fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Subpath<Pt2>> {
    polygon(&[p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)])
}

/// The power-of-two canvas: 256 × 128 px over 4 × 2 units (64 px/unit).
const EXACT: (u32, u32) = (256, 128);
const EXACT_VIEW: (f64, f64) = (4.0, 2.0);

/// The image coordinate of pixel column `px`'s centre on [`EXACT`].
fn col_x(px: u32) -> f64 {
    (f64::from(px) + 0.5 - 128.0) / 64.0
}

/// The image coordinate of pixel row `py`'s centre on [`EXACT`].
fn row_y(py: u32) -> f64 {
    (64.0 - (f64::from(py) + 0.5)) / 64.0
}

/// One PPM frame of `prims` on a black background, header stripped.
fn render_prims(name: &str, size: (u32, u32), view: (f64, f64), prims: &[Prim2]) -> Vec<u8> {
    let dir = tmp_dir(name);
    let mut sink = PpmSink::with_view(&dir, size, view).expect("ppm sink");
    sink.frame(0, prims).expect("frame");
    let bytes = fs::read(dir.join("frame_00000.ppm")).expect("read frame");
    let header = format!("P6\n{} {}\n255\n", size.0, size.1).len();
    bytes[header..].to_vec()
}

fn pixel(frame: &[u8], size: (u32, u32), x: u32, y: u32) -> [u8; 3] {
    let i = (y as usize * size.0 as usize + x as usize) * 3;
    [frame[i], frame[i + 1], frame[i + 2]]
}

fn rgb(c: Rgb) -> [u8; 3] {
    [c.r, c.g, c.b]
}

/// A red → blue linear gradient from pixel column `c0`'s centre to column
/// `c1`'s, along a horizontal axis on [`EXACT`].
fn red_to_blue(c0: u32, c1: u32) -> Gradient {
    Gradient::linear(
        p(col_x(c0), 0.0),
        p(col_x(c1), 0.0),
        &[stop(0.0, RED, 1.0), stop(1.0, BLUE, 1.0)],
    )
    .expect("valid gradient")
}

/// The whole [`EXACT`] view as one rectangle: every pixel at coverage 1.
fn whole_view() -> Vec<Subpath<Pt2>> {
    rect(-2.0, -1.0, 2.0, 1.0)
}

/// Bless or compare one golden fixture (SPEC-0003's `MOTOREEL_BLESS=1`).
fn check_golden(fixture: &str, got: &[u8]) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(fixture);
    if std::env::var_os("MOTOREEL_BLESS").is_some_and(|v| v == "1") {
        fs::write(&path, got).expect("bless the golden fixture");
        eprintln!("blessed {} — review it like code", path.display());
        return;
    }
    let want = fs::read(&path).unwrap_or_else(|_| {
        panic!(
            "{} is missing. Bless it once the implementation is reviewed: \
             MOTOREEL_BLESS=1 cargo test -p motoreel --test r0016_gradients",
            path.display()
        )
    });
    assert_eq!(want.len(), got.len(), "{fixture}: wrong length");
    if let Some(at) = (0..got.len()).find(|&i| got[i] != want[i]) {
        panic!(
            "{fixture} drifted at byte {at}: rendered {}, fixture {}",
            got[at], want[at]
        );
    }
}

// --- AC1: the paint model ----------------------------------------------------

mod ac1 {
    use super::*;

    // `Fill::solid` keeps its `const` signature and means a flat paint;
    // `Fill`, `Paint`, `Gradient` and `Style` are `Copy`.
    #[test]
    fn solid_is_flat_paint_and_everything_stays_copy() {
        const HALF_GOLD: Fill = Fill::solid(GOLD, 0.5);
        assert_eq!(HALF_GOLD.paint, Paint::Flat(GOLD));
        assert_eq!(HALF_GOLD.alpha, 0.5);
        fn copy<T: Copy>(t: T) -> (T, T) {
            (t, t)
        }
        let g = red_to_blue(0, 10);
        let _ = copy(g);
        let _ = copy(Paint::Gradient(g));
        let _ = copy(Fill::gradient(g, 1.0));
        let _ = copy(fill_style(Fill::gradient(g, 1.0)));
    }

    // The constructors keep the kind, the stops verbatim and the frame of
    // SPEC-0016 §2.2.
    #[test]
    fn linear_and_radial_keep_their_frame_and_stops() {
        let stops = [
            stop(0.0, RED, 1.0),
            stop(0.25, GOLD, 0.5),
            stop(1.0, BLUE, 0.0),
        ];
        let lin = Gradient::linear(p(1.0, 2.0), p(4.0, 6.0), &stops).expect("valid");
        assert_eq!(lin.kind(), GradientKind::Linear);
        assert_eq!(lin.stops(), &stops);
        assert_eq!(lin.frame(), (p(1.0, 2.0), p(4.0, 6.0), p(-3.0, 5.0)));

        let rad = Gradient::radial(p(1.0, -1.0), 0.5, &stops[..2]).expect("valid");
        assert_eq!(rad.kind(), GradientKind::Radial);
        assert_eq!(rad.stops(), &stops[..2]);
        assert_eq!(rad.frame(), (p(1.0, -1.0), p(1.5, -1.0), p(1.0, -0.5)));
    }

    // Eight stops is the most, and equal gradients compare equal however
    // they were built (unused slots carry no leftovers).
    #[test]
    fn eight_stops_are_accepted_and_equality_is_by_meaning() {
        let eight: Vec<Stop> = (0..8)
            .map(|i| stop(f64::from(i) / 7.0, GOLD, 1.0))
            .collect();
        let g = Gradient::linear(p(0.0, 0.0), p(1.0, 0.0), &eight).expect("eight stops");
        assert_eq!(g.stops().len(), 8);
        let a = Gradient::linear(p(0.0, 0.0), p(1.0, 0.0), &eight[..2]).expect("valid");
        let b = Gradient::linear(p(0.0, 0.0), p(1.0, 0.0), &eight[..2]).expect("valid");
        assert_eq!(a, b);
    }
}

// --- AC2: invalid input is a typed error -------------------------------------

mod ac2 {
    use super::*;

    fn lin(stops: &[Stop]) -> Result<Gradient, GradientError> {
        Gradient::linear(p(0.0, 0.0), p(1.0, 0.0), stops)
    }

    #[test]
    fn stop_count_outside_two_to_eight_is_rejected() {
        let s = stop(0.0, RED, 1.0);
        for n in [0, 1, 9, 12] {
            let stops = vec![s; n];
            assert_eq!(lin(&stops), Err(GradientError::StopCount { count: n }));
            assert_eq!(
                Gradient::radial(p(0.0, 0.0), 1.0, &stops),
                Err(GradientError::StopCount { count: n })
            );
        }
    }

    #[test]
    fn bad_offsets_are_rejected_at_their_index() {
        for bad in [f64::NAN, f64::INFINITY, -0.01, 1.01] {
            let stops = [
                stop(0.0, RED, 1.0),
                stop(0.5, RED, 1.0),
                stop(bad, RED, 1.0),
            ];
            assert_eq!(lin(&stops), Err(GradientError::StopOffset { index: 2 }));
        }
    }

    #[test]
    fn unsorted_offsets_are_rejected_at_their_index() {
        let stops = [
            stop(0.0, RED, 1.0),
            stop(0.6, RED, 1.0),
            stop(0.4, RED, 1.0),
        ];
        assert_eq!(lin(&stops), Err(GradientError::StopsUnsorted { index: 2 }));
        // Equal offsets are a hard edge, not an error.
        let hard = [
            stop(0.0, RED, 1.0),
            stop(0.5, RED, 1.0),
            stop(0.5, BLUE, 1.0),
        ];
        assert!(lin(&hard).is_ok());
    }

    #[test]
    fn bad_stop_alpha_is_rejected_at_their_index() {
        for bad in [f64::NAN, f64::NEG_INFINITY, -0.5, 1.5] {
            let stops = [stop(0.0, RED, bad), stop(1.0, RED, 1.0)];
            assert_eq!(lin(&stops), Err(GradientError::StopAlpha { index: 0 }));
        }
    }

    #[test]
    fn non_finite_geometry_is_rejected() {
        let s = [stop(0.0, RED, 1.0), stop(1.0, BLUE, 1.0)];
        for bad in [f64::NAN, f64::INFINITY] {
            assert_eq!(
                Gradient::linear(p(bad, 0.0), p(1.0, 0.0), &s),
                Err(GradientError::NonFinite)
            );
            assert_eq!(
                Gradient::linear(p(0.0, 0.0), p(1.0, bad), &s),
                Err(GradientError::NonFinite)
            );
            assert_eq!(
                Gradient::radial(p(0.0, bad), 1.0, &s),
                Err(GradientError::NonFinite)
            );
            assert_eq!(
                Gradient::radial(p(0.0, 0.0), bad, &s),
                Err(GradientError::NonFinite)
            );
        }
        // Finite input whose frame overflows is rejected too.
        assert_eq!(
            Gradient::linear(p(-f64::MAX, 0.0), p(f64::MAX, 0.0), &s),
            Err(GradientError::NonFinite)
        );
    }

    #[test]
    fn degenerate_axis_and_non_positive_radius_are_rejected() {
        let s = [stop(0.0, RED, 1.0), stop(1.0, BLUE, 1.0)];
        assert_eq!(
            Gradient::linear(p(0.5, 0.5), p(0.5, 0.5), &s),
            Err(GradientError::DegenerateAxis)
        );
        for r in [0.0, -0.0, -1.0] {
            assert_eq!(
                Gradient::radial(p(0.0, 0.0), r, &s),
                Err(GradientError::NonPositiveRadius)
            );
        }
    }

    #[test]
    fn errors_display_and_implement_error() {
        let e: Box<dyn std::error::Error> = Box::new(GradientError::StopsUnsorted { index: 3 });
        let text = e.to_string();
        assert!(text.contains('3'), "the message names the stop: {text}");
        assert!(!GradientError::NonPositiveRadius.to_string().is_empty());
    }
}

// --- AC3: interpolation is pinned --------------------------------------------

mod ac3 {
    use super::*;

    fn three() -> Gradient {
        Gradient::linear(
            p(0.0, 0.0),
            p(1.0, 0.0),
            &[
                stop(0.0, RED, 1.0),
                stop(0.25, GOLD, 0.5),
                stop(1.0, BLUE, 0.2),
            ],
        )
        .expect("valid")
    }

    fn channels(c: Rgb) -> [f64; 3] {
        [f64::from(c.r), f64::from(c.g), f64::from(c.b)]
    }

    #[test]
    fn endpoints_are_the_stops_exactly() {
        let g = three();
        assert_eq!(g.paint_at(0.0), (channels(RED), 1.0));
        assert_eq!(g.paint_at(1.0), (channels(BLUE), 0.2));
        assert_eq!(g.paint_at(0.25), (channels(GOLD), 0.5));
    }

    #[test]
    fn pad_holds_the_end_stops_beyond_the_ends() {
        let g = three();
        for t in [-1e9, -0.5, -0.0] {
            assert_eq!(g.paint_at(t), (channels(RED), 1.0), "t = {t}");
        }
        for t in [1.0 + 1e-12, 3.0, 1e300, f64::INFINITY] {
            assert_eq!(g.paint_at(t), (channels(BLUE), 0.2), "t = {t}");
        }
        // NaN is total: the first stop, never a NaN colour.
        assert_eq!(g.paint_at(f64::NAN), (channels(RED), 1.0));
    }

    // Halfway between GOLD@0.5 (0.25) and BLUE@0.2 (1.0) is the plain
    // average of the encoded values: sRGB, unpremultiplied.
    #[test]
    fn interpolation_is_linear_in_srgb_unpremultiplied() {
        let (c, a) = three().paint_at(0.625);
        let mid = |x: u8, y: u8| f64::from(x) + (f64::from(y) - f64::from(x)) * 0.5;
        assert_eq!(
            c,
            [
                mid(GOLD.r, BLUE.r),
                mid(GOLD.g, BLUE.g),
                mid(GOLD.b, BLUE.b)
            ]
        );
        assert_eq!(a, 0.5 + (0.2 - 0.5) * 0.5);
    }

    #[test]
    fn equal_offsets_make_a_hard_edge_and_the_later_stop_wins() {
        let g = Gradient::linear(
            p(0.0, 0.0),
            p(1.0, 0.0),
            &[
                stop(0.0, RED, 1.0),
                stop(0.5, RED, 1.0),
                stop(0.5, BLUE, 1.0),
                stop(1.0, BLUE, 1.0),
            ],
        )
        .expect("valid");
        assert_eq!(g.paint_at(0.499_999).0, channels(RED));
        assert_eq!(g.paint_at(0.5).0, channels(BLUE));
    }

    #[test]
    fn linear_offset_is_constant_across_the_axis() {
        let g = Gradient::linear(
            p(1.0, 1.0),
            p(3.0, 1.0),
            &[stop(0.0, RED, 1.0), stop(1.0, BLUE, 1.0)],
        )
        .expect("valid");
        assert_eq!(g.offset_at(p(1.0, 1.0)), Some(0.0));
        assert_eq!(g.offset_at(p(3.0, 1.0)), Some(1.0));
        for y in [-5.0, 0.0, 1.0, 7.5] {
            assert_eq!(g.offset_at(p(2.0, y)), Some(0.5), "y = {y}");
        }
        assert_eq!(g.offset_at(p(-1.0, 1.0)), Some(-1.0), "before p0: negative");
    }

    #[test]
    fn radial_offset_is_the_distance_over_the_radius() {
        let g = Gradient::radial(
            p(1.0, 1.0),
            2.0,
            &[stop(0.0, RED, 1.0), stop(1.0, BLUE, 1.0)],
        )
        .expect("valid");
        assert_eq!(g.offset_at(p(1.0, 1.0)), Some(0.0));
        assert_eq!(g.offset_at(p(3.0, 1.0)), Some(1.0));
        assert_eq!(g.offset_at(p(1.0, -1.0)), Some(1.0));
        assert_eq!(g.offset_at(p(1.0, 2.0)), Some(0.5));
        let t = g.offset_at(p(2.2, 2.6)).expect("invertible");
        assert!((t - 1.0).abs() < 1e-15, "3-4-5 point on the circle: {t}");
    }
}

// --- AC4: the gradient rides its object --------------------------------------

mod ac4 {
    use super::*;

    fn square_with(fill: Fill) -> Object {
        Object::planar(rect(-0.25, -0.25, 0.25, 0.25)).with_style(fill_style(fill))
    }

    fn gradient_of(prim: &Prim2) -> Gradient {
        match prim {
            Prim2::Path { style, .. } => match style.fill.expect("a fill").paint {
                Paint::Gradient(g) => g,
                Paint::Flat(_) => panic!("expected a gradient"),
            },
            other => panic!("expected a path, got {other:?}"),
        }
    }

    /// The image of model-plane point `q` under `scene`'s camera and a
    /// pose, by the pipeline's own point mapping: what a control point gets.
    fn image_of(scene: &Scene, pose: &Motor3, q: Pt2) -> Pt2 {
        let to_view = scene.camera.pose.inverse().compose(pose);
        scene
            .camera
            .projection
            .project(&pga::Point::new(q.x, q.y, 0.0).transform(&to_view))
            .expect("projects")
    }

    fn assert_rides(scene: &Scene, pose: Motor3, g: Gradient) {
        let mut scene = scene.clone();
        scene.add(square_with(Fill::gradient(g, 1.0)).at(pose));
        let prims = scene.eval(0.0);
        let got = gradient_of(&prims[0]);
        let (o, x, y) = g.frame();
        assert_eq!(
            got.frame(),
            (
                image_of(&scene, &pose, o),
                image_of(&scene, &pose, x),
                image_of(&scene, &pose, y)
            ),
            "the frame rides exactly like control points"
        );
        assert_eq!(got.stops(), g.stops(), "stops are carried verbatim");
        assert_eq!(got.kind(), g.kind());
    }

    #[test]
    fn a_translated_object_moves_its_gradient() {
        let g = red_to_blue(0, 64);
        let pose = Motor3::translator(1.0, 0.5, 0.0);
        assert_rides(&Scene::new(1.0), pose, g);
        let mut scene = Scene::new(1.0);
        scene.add(square_with(Fill::gradient(g, 1.0)).at(pose));
        let (o, ..) = gradient_of(&scene.eval(0.0)[0]).frame();
        assert_eq!(o, p(col_x(0) + 1.0, 0.5), "binary-exact translation");
    }

    #[test]
    fn a_rotated_object_rotates_its_gradient() {
        let g = Gradient::radial(
            p(0.5, 0.25),
            0.5,
            &[stop(0.0, RED, 1.0), stop(1.0, BLUE, 1.0)],
        )
        .expect("valid");
        // A half turn about z, from an exact unit quaternion.
        let half_turn = Motor3::from_unit_quaternion(0.0, 0.0, 0.0, 1.0);
        assert_rides(&Scene::new(1.0), half_turn, g);
        let mut scene = Scene::new(1.0);
        scene.add(square_with(Fill::gradient(g, 1.0)).at(half_turn));
        let (o, x, _) = gradient_of(&scene.eval(0.0)[0]).frame();
        assert!(
            (o.x + 0.5).abs() < 1e-12 && (o.y + 0.25).abs() < 1e-12,
            "{o:?}"
        );
        assert!(
            (x.x + 1.0).abs() < 1e-12 && (x.y + 0.25).abs() < 1e-12,
            "{x:?}"
        );
    }

    #[test]
    fn pinhole_frame_is_the_projected_frame_points() {
        let mut scene = Scene::new(1.0);
        scene.camera = Camera::pinhole(Motor3::translator(0.0, 0.0, 4.0), 2.0);
        let g = red_to_blue(0, 64);
        assert_rides(&scene, Motor3::identity(), g);
        scene.add(square_with(Fill::gradient(g, 1.0)));
        let (o, x, _) = gradient_of(&scene.eval(0.0)[0]).frame();
        assert_eq!((o, x), (p(col_x(0) / 2.0, 0.0), p(col_x(64) / 2.0, 0.0)));
    }

    // The square is in front of the camera, but the linear axis's far end
    // tilts behind it: the primitive is culled whole (R-0002's rule).
    #[test]
    fn a_frame_point_behind_the_camera_culls_the_primitive() {
        let mut scene = Scene::new(1.0);
        scene.camera = Camera::pinhole(Motor3::translator(0.0, 0.0, 4.0), 2.0);
        // Rotation about x with cos = −0.28, sin = 0.96: y = 10 lands at z = 9.6.
        let tilt = Motor3::from_unit_quaternion(0.6, 0.8, 0.0, 0.0);
        let far = Gradient::linear(
            p(0.0, 0.0),
            p(0.0, 10.0),
            &[stop(0.0, RED, 1.0), stop(1.0, BLUE, 1.0)],
        )
        .expect("valid");
        scene.add(square_with(Fill::solid(RED, 1.0)).at(tilt));
        scene.add(square_with(Fill::gradient(far, 1.0)).at(tilt));
        let prims = scene.eval(0.0);
        assert_eq!(
            prims.len(),
            1,
            "the flat square survives, the gradient one is culled"
        );
    }

    // Only a path's gradient is posed. A flat fill and every other style
    // field are carried verbatim.
    #[test]
    fn flat_fills_and_other_styles_pass_verbatim() {
        let mut scene = Scene::new(1.0);
        let style = Style {
            stroke: GOLD,
            width: 0.02,
            alpha: 0.75,
            fill: Some(Fill::solid(INK, 0.5)),
        };
        scene.add(
            Object::planar(rect(-0.25, -0.25, 0.25, 0.25))
                .with_style(style)
                .at(Motor3::translator(0.5, 0.0, 0.0)),
        );
        match &scene.eval(0.0)[0] {
            Prim2::Path { style: got, .. } => assert_eq!(*got, style),
            other => panic!("expected a path, got {other:?}"),
        }
    }
}

// --- AC5: the raster sink paints the gradient --------------------------------

mod ac5 {
    use super::*;

    #[test]
    fn linear_endpoints_paint_their_stop_colours_exactly() {
        let f = render_prims(
            "r0016_ac5_linear_ends",
            EXACT,
            EXACT_VIEW,
            &[path_prim(
                whole_view(),
                Fill::gradient(red_to_blue(10, 200), 1.0),
            )],
        );
        for y in 0..EXACT.1 {
            assert_eq!(pixel(&f, EXACT, 10, y), rgb(RED), "column 10, row {y}");
            assert_eq!(pixel(&f, EXACT, 200, y), rgb(BLUE), "column 200, row {y}");
        }
        // And in between it varies: column 105 is neither end.
        let mid = pixel(&f, EXACT, 105, 64);
        assert!(mid != rgb(RED) && mid != rgb(BLUE), "{mid:?}");
    }

    #[test]
    fn pad_region_is_exactly_the_end_stop() {
        let f = render_prims(
            "r0016_ac5_pad",
            EXACT,
            EXACT_VIEW,
            &[path_prim(
                whole_view(),
                Fill::gradient(red_to_blue(10, 200), 1.0),
            )],
        );
        for y in 0..EXACT.1 {
            for x in 0..=10 {
                assert_eq!(pixel(&f, EXACT, x, y), rgb(RED), "({x}, {y})");
            }
            for x in 200..EXACT.0 {
                assert_eq!(pixel(&f, EXACT, x, y), rgb(BLUE), "({x}, {y})");
            }
        }
    }

    #[test]
    fn radial_centre_and_edge_paint_their_stops() {
        // Centre on pixel (100, 60), radius 32 px.
        let g = Gradient::radial(
            p(col_x(100), row_y(60)),
            0.5,
            &[stop(0.0, RED, 1.0), stop(1.0, BLUE, 1.0)],
        )
        .expect("valid");
        let f = render_prims(
            "r0016_ac5_radial",
            EXACT,
            EXACT_VIEW,
            &[path_prim(whole_view(), Fill::gradient(g, 1.0))],
        );
        assert_eq!(pixel(&f, EXACT, 100, 60), rgb(RED), "centre");
        for (x, y) in [(132, 60), (68, 60), (100, 28), (100, 92)] {
            assert_eq!(pixel(&f, EXACT, x, y), rgb(BLUE), "edge ({x}, {y})");
        }
        for (x, y) in [(0, 0), (255, 127), (140, 60)] {
            assert_eq!(pixel(&f, EXACT, x, y), rgb(BLUE), "pad ({x}, {y})");
        }
        let inside = pixel(&f, EXACT, 116, 60);
        assert!(inside != rgb(RED) && inside != rgb(BLUE), "{inside:?}");
    }
}

// --- AC6: nothing already rendered moves -------------------------------------

mod ac6 {
    use super::*;

    /// A disc with anti-aliased edges and a fractional opacity: the case
    /// where a dither, if it ran, would change bytes.
    fn disc(fill: Fill) -> Vec<Prim2> {
        vec![path_prim(circle(p(0.1, -0.05), 0.7), fill)]
    }

    #[test]
    fn equal_stops_render_byte_identical_to_solid() {
        let cases = [
            (0.6, 1.0, 0.6), // fill opacity 0.6, opaque stops
            (1.0, 0.5, 0.5), // opaque fill, half-opaque stops
        ];
        for (k, (fill_alpha, stop_alpha, solid_alpha)) in cases.into_iter().enumerate() {
            let g = Gradient::radial(
                p(0.3, 0.2),
                0.9,
                &[stop(0.0, GOLD, stop_alpha), stop(1.0, GOLD, stop_alpha)],
            )
            .expect("valid");
            let shaded = render_prims(
                &format!("r0016_ac6_equal_{k}"),
                EXACT,
                EXACT_VIEW,
                &disc(Fill::gradient(g, fill_alpha)),
            );
            let solid = render_prims(
                &format!("r0016_ac6_solid_{k}"),
                EXACT,
                EXACT_VIEW,
                &disc(Fill::solid(GOLD, solid_alpha)),
            );
            assert!(
                shaded == solid,
                "case {k}: equal stops must be the flat fill"
            );
        }
    }

    /// SPEC-0012 §2.10's golden scene, restated here on purpose: this test
    /// pins the *same fixture file* through `Fill::solid` after the `Fill`
    /// change, so the copy cannot drift without the byte compare failing.
    fn r0012_golden_scene() -> Scene {
        let filled = |c: Rgb| fill_style(Fill::solid(c, 1.0));
        let square = |cx: f64, h: f64| [p(cx - h, -h), p(cx + h, -h), p(cx + h, h), p(cx - h, h)];
        let mut scene = Scene::new(1.0);
        scene.add(Object::planar(circle(p(-1.0, 0.0), 0.5)).with_style(Style {
            stroke: GOLD,
            ..filled(GOLD)
        }));
        scene.add(
            Object::planar(rounded_rect(p(0.2, 0.0), 1.0, 0.8, 0.2)).with_style(Style {
                stroke: STEEL,
                width: 0.02,
                alpha: 1.0,
                fill: Some(Fill::solid(INK, 1.0)),
            }),
        );
        let mut hole = polygon(&square(1.15, 0.35));
        hole.extend(polygon(&square(1.15, 0.15)).iter().map(Subpath::reversed));
        scene.add(Object::planar(hole).with_style(Style {
            stroke: SIGNAL,
            ..filled(SIGNAL)
        }));
        scene
    }

    #[test]
    fn the_r0012_golden_is_unchanged() {
        let dir = tmp_dir("r0016_ac6_r0012_golden");
        let mut sink = PpmSink::with_view(&dir, (320, 180), (3.2, 1.8)).expect("sink");
        r0012_golden_scene().render(1.0, &mut sink).expect("render");
        let got = fs::read(dir.join("frame_00000.ppm")).expect("frame");
        let want = include_bytes!("golden/r0012_paths.ppm");
        assert!(got == want, "R-0012's PPM golden moved");
    }
}

// --- AC7: the raster sink does not band --------------------------------------

mod ac7 {
    use super::*;

    /// A 1024 × 64 canvas over 16 × 1 units (64 px/unit), filled by a
    /// horizontal ramp of 8 code values from column 0 to column 1023: 128 px
    /// per code value when quantised plainly.
    const WIDE: (u32, u32) = (1024, 64);
    const WIDE_VIEW: (f64, f64) = (16.0, 1.0);
    const DARK: Rgb = Rgb {
        r: 40,
        g: 24,
        b: 16,
    };
    const LESS_DARK: Rgb = Rgb {
        r: 48,
        g: 32,
        b: 24,
    };

    fn wide_x(px: u32) -> f64 {
        (f64::from(px) + 0.5 - 512.0) / 64.0
    }

    fn ramp(name: &str) -> Vec<u8> {
        let g = Gradient::linear(
            p(wide_x(0), 0.0),
            p(wide_x(1023), 0.0),
            &[stop(0.0, DARK, 1.0), stop(1.0, LESS_DARK, 1.0)],
        )
        .expect("valid");
        render_prims(
            name,
            WIDE,
            WIDE_VIEW,
            &[path_prim(
                rect(-8.0, -0.5, 8.0, 0.5),
                Fill::gradient(g, 1.0),
            )],
        )
    }

    /// The exact red channel at column `x`: the composite before rounding.
    fn ideal_red(x: u32) -> f64 {
        let t = f64::from(x) / 1023.0;
        40.0 + 8.0 * t
    }

    /// Column `x`'s 8-pixel-tall stack of rows 0..8.
    fn column(f: &[u8], x: u32) -> Vec<[u8; 3]> {
        (0..8).map(|y| pixel(f, WIDE, x, y)).collect()
    }

    #[test]
    fn a_long_ramp_has_no_column_run_longer_than_the_dither_period() {
        // The premise: plain rounding would hold each value ≥ 100 px.
        let plain: Vec<i64> = (0..WIDE.0).map(|x| ideal_red(x).round() as i64).collect();
        let longest_plain = plain
            .chunk_by(|a, b| a == b)
            .map(<[i64]>::len)
            .max()
            .expect("non-empty");
        assert!(longest_plain >= 100, "premise: steps of {longest_plain} px");

        let f = ramp("r0016_ac7_ramp_runs");
        let columns: Vec<Vec<[u8; 3]>> = (0..WIDE.0).map(|x| column(&f, x)).collect();
        let longest = columns
            .chunk_by(|a, b| a == b)
            .map(<[Vec<[u8; 3]>]>::len)
            .max()
            .expect("non-empty");
        // One Bayer period: a run can span the columns whose eight
        // thresholds all fall on one side of the value, never a whole step.
        assert!(
            longest <= 8,
            "a run of {longest} identical columns: banding"
        );
    }

    #[test]
    fn dither_is_unbiased_per_8x8_block() {
        let f = ramp("r0016_ac7_ramp_bias");
        for bx in 0..WIDE.0 / 8 {
            for by in 0..WIDE.1 / 8 {
                let (mut got, mut want) = (0.0, 0.0);
                for x in bx * 8..bx * 8 + 8 {
                    for y in by * 8..by * 8 + 8 {
                        got += f64::from(pixel(&f, WIDE, x, y)[0]);
                        want += ideal_red(x);
                    }
                }
                let bias = (got - want) / 64.0;
                assert!(bias.abs() <= 0.5, "block ({bx}, {by}) is off by {bias}");
            }
        }
    }

    // A flat fill at a fractional opacity rounds every pixel the same way:
    // half-white over black is 128 everywhere, with no pattern.
    #[test]
    fn flat_fills_are_not_dithered() {
        let f = render_prims(
            "r0016_ac7_flat",
            EXACT,
            EXACT_VIEW,
            &[path_prim(whole_view(), Fill::solid(Rgb::WHITE, 0.5))],
        );
        assert!(f.iter().all(|&b| b == 128));
    }
}

// --- AC8: determinism and the goldens ----------------------------------------

/// The trig-free golden scene, 160 × 90 over the default view:
///
/// 1. a rounded panel with a three-stop linear gradient, top to bottom;
/// 2. a disc with a radial gradient fading gold to transparent gold;
/// 3. a hard-stop linear band over both, at fill opacity 0.75, stroked.
fn golden_scene() -> Scene {
    let mut scene = Scene::new(1.0);
    let panel = Gradient::linear(
        p(0.0, 0.8),
        p(0.0, -0.8),
        &[
            stop(0.0, STEEL, 1.0),
            stop(0.5, BLUE, 1.0),
            stop(1.0, INK, 1.0),
        ],
    )
    .expect("valid");
    scene.add(
        Object::planar(rounded_rect(p(-0.6, 0.0), 1.6, 1.6, 0.25))
            .with_style(fill_style(Fill::gradient(panel, 1.0))),
    );
    let glow = Gradient::radial(
        p(0.75, 0.25),
        0.75,
        &[stop(0.0, GOLD, 1.0), stop(1.0, GOLD, 0.0)],
    )
    .expect("valid");
    scene.add(
        Object::planar(circle(p(0.75, 0.25), 0.75))
            .with_style(fill_style(Fill::gradient(glow, 1.0))),
    );
    let band = Gradient::linear(
        p(-1.5, 0.0),
        p(1.5, 0.0),
        &[
            stop(0.0, SIGNAL, 1.0),
            stop(0.5, SIGNAL, 1.0),
            stop(0.5, GOLD, 1.0),
            stop(1.0, GOLD, 0.5),
        ],
    )
    .expect("valid");
    scene.add(
        Object::planar(rect(-1.5, -0.6, 1.5, -0.4)).with_style(Style {
            stroke: Rgb::WHITE,
            width: 0.02,
            alpha: 1.0,
            fill: Some(Fill::gradient(band, 0.75)),
        }),
    );
    scene
}

const GOLDEN_SIZE: (u32, u32) = (160, 90);

fn render_golden(name: &str, svg: bool) -> Vec<u8> {
    let dir = tmp_dir(name);
    let file = if svg {
        let mut sink = SvgSink::with_view(&dir, GOLDEN_SIZE, (3.2, 1.8)).expect("svg sink");
        golden_scene().render(1.0, &mut sink).expect("render");
        "frame_00000.svg"
    } else {
        let mut sink = PpmSink::with_view(&dir, GOLDEN_SIZE, (3.2, 1.8)).expect("ppm sink");
        golden_scene().render(1.0, &mut sink).expect("render");
        "frame_00000.ppm"
    };
    fs::read(dir.join(file)).expect("read frame")
}

mod ac8 {
    use super::*;

    #[test]
    fn two_renders_are_byte_identical() {
        for svg in [false, true] {
            let a = render_golden(&format!("r0016_ac8_twice_a_{svg}"), svg);
            let b = render_golden(&format!("r0016_ac8_twice_b_{svg}"), svg);
            assert!(a == b, "svg = {svg}");
        }
    }

    #[test]
    fn renders_on_several_threads_are_byte_identical() {
        let reference = render_golden("r0016_ac8_threads_ref", false);
        let frames: Vec<Vec<u8>> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..4)
                .map(|i| s.spawn(move || render_golden(&format!("r0016_ac8_threads_{i}"), false)))
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().expect("render thread"))
                .collect()
        });
        for (i, f) in frames.iter().enumerate() {
            assert!(*f == reference, "thread {i} differs");
        }
    }

    #[test]
    fn the_ppm_golden_matches_byte_for_byte() {
        check_golden(
            "r0016_gradients.ppm",
            &render_golden("r0016_ac8_ppm_golden", false),
        );
    }
}

// --- AC9: the SVG sink emits native gradients --------------------------------

mod ac9 {
    use super::*;

    #[test]
    fn the_svg_golden_matches_byte_for_byte() {
        let got = render_golden("r0016_ac9_svg_golden", true);
        let text = String::from_utf8(got.clone()).expect("UTF-8");
        assert_eq!(text.matches("<linearGradient ").count(), 2);
        assert_eq!(text.matches("<radialGradient ").count(), 1);
        assert_eq!(text.matches("<stop ").count(), 3 + 2 + 4);
        check_golden("r0016_gradients.svg", &got);
    }

    // The grammar of one element, line by line (SPEC-0016 §2.8).
    #[test]
    fn a_linear_gradient_is_written_as_pinned() {
        let dir = tmp_dir("r0016_ac9_grammar");
        let mut sink = SvgSink::with_view(&dir, (100, 100), (2.0, 2.0)).expect("sink");
        let g = Gradient::linear(
            p(-0.5, 0.25),
            p(0.5, 0.25),
            &[stop(0.0, RED, 1.0), stop(1.0, BLUE, 0.5)],
        )
        .expect("valid");
        sink.frame(
            0,
            &[path_prim(
                rect(-1.0, -1.0, 1.0, 1.0),
                Fill::gradient(g, 0.75),
            )],
        )
        .expect("frame");
        let text = fs::read_to_string(dir.join("frame_00000.svg")).expect("read");
        let lines: Vec<&str> = text.lines().collect();
        let at = lines
            .iter()
            .position(|l| l.starts_with("<linearGradient"))
            .expect("a gradient element");
        assert_eq!(
            &lines[at..at + 5],
            &[
                "<linearGradient id=\"g0\" gradientUnits=\"userSpaceOnUse\" \
                 gradientTransform=\"matrix(1 0 0 1 -0.5 0.25)\" x1=\"0\" y1=\"0\" \
                 x2=\"1\" y2=\"0\" spreadMethod=\"pad\">",
                "<stop offset=\"0\" stop-color=\"#ff0000\" stop-opacity=\"1\"/>",
                "<stop offset=\"1\" stop-color=\"#1040f0\" stop-opacity=\"0.5\"/>",
                "</linearGradient>",
                "<path d=\"M -1,-1 L 1,-1 L 1,1 L -1,1 Z\" fill=\"url(#g0)\" \
                 fill-opacity=\"0.75\" fill-rule=\"nonzero\" stroke=\"none\"/>",
            ]
        );
    }

    #[test]
    fn ids_count_from_zero_in_draw_order_each_frame() {
        let dir = tmp_dir("r0016_ac9_ids");
        let mut sink = SvgSink::with_view(&dir, (100, 100), (2.0, 2.0)).expect("sink");
        let g = red_to_blue(0, 64);
        let prims = [
            path_prim(rect(-1.0, -1.0, 0.0, 0.0), Fill::gradient(g, 1.0)),
            path_prim(rect(0.0, 0.0, 0.5, 0.5), Fill::solid(GOLD, 1.0)),
            path_prim(rect(0.0, -1.0, 1.0, 0.0), Fill::gradient(g, 1.0)),
        ];
        for index in 0..2 {
            sink.frame(index, &prims).expect("frame");
            let text = fs::read_to_string(dir.join(format!("frame_{index:05}.svg"))).expect("read");
            let ids: Vec<&str> = text
                .match_indices(" id=\"")
                .map(|(i, _)| &text[i + 5..i + 7])
                .collect();
            assert_eq!(ids, ["g0", "g1"], "frame {index}");
            assert_eq!(text.matches("fill=\"url(#g0)\"").count(), 1);
            assert_eq!(text.matches("fill=\"url(#g1)\"").count(), 1);
        }
    }

    #[test]
    fn a_radial_gradient_names_its_unit_circle() {
        let dir = tmp_dir("r0016_ac9_radial");
        let mut sink = SvgSink::with_view(&dir, (100, 100), (2.0, 2.0)).expect("sink");
        let g = Gradient::radial(
            p(0.25, -0.5),
            0.5,
            &[stop(0.0, RED, 1.0), stop(1.0, BLUE, 1.0)],
        )
        .expect("valid");
        sink.frame(
            0,
            &[path_prim(
                rect(-1.0, -1.0, 1.0, 1.0),
                Fill::gradient(g, 1.0),
            )],
        )
        .expect("frame");
        let text = fs::read_to_string(dir.join("frame_00000.svg")).expect("read");
        assert!(text.contains(
            "<radialGradient id=\"g0\" gradientUnits=\"userSpaceOnUse\" \
             gradientTransform=\"matrix(0.5 0 0 0.5 0.25 -0.5)\" cx=\"0\" cy=\"0\" \
             r=\"1\" spreadMethod=\"pad\">"
        ));
    }
}

// --- AC10: the kernel stays thin ---------------------------------------------

mod ac10 {
    use super::*;

    // Gradients are core API: this suite uses no feature-gated item, and
    // CI builds it under `--no-default-features`.
    #[test]
    fn gradients_need_no_feature() {
        let g = red_to_blue(0, 1);
        let mut scene = Scene::new(0.0);
        scene.add(
            Object::planar(rect(-1.0, -1.0, 1.0, 1.0))
                .with_style(fill_style(Fill::gradient(g, 1.0)))
                .with_track(Track::hold(Motor3::identity())),
        );
        assert_eq!(scene.eval(0.0).len(), 1);
    }
}
