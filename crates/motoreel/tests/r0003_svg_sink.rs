//! R-0003 — `SvgSink` and the first rendered animation: the QA-owned
//! acceptance suite (e2e).
//!
//! Loop step 3, TDD red: authored before the implementation exists, derived
//! from the acceptance criteria of `requirements/0003-svg-sink.md`
//! (AC1–AC5) and SPEC-0003 §6's test plan. Each test's header comment names
//! the criteria it verifies. SPEC-0003 §2.1 sketches four test files; this
//! suite keeps the repo's one-file-per-requirement convention (r0001,
//! r0002) while carrying §6's per-AC content unchanged.
//!
//! Criterion → test map:
//!
//! - AC1  `ac1_frame_count_indices_and_zero_duration`,
//!   `ac1_timing_law_frame_times_are_derived_not_accumulated`,
//!   `ac1_sink_error_stops_the_walk`,
//!   `ac1_bad_fps_and_bad_duration_are_invalid_input_before_any_sink_call`,
//!   `ac1_render_accepts_a_dyn_sink`
//! - AC2  `ac2_filenames_are_zero_padded_five_digits`,
//!   `ac2_new_creates_nested_output_directory`,
//!   `ac2_empty_frame_is_the_well_formed_empty_document`,
//!   `ac2_custom_view_header_and_viewbox`,
//!   `ac2_primitive_templates_are_pinned`,
//!   `ac2_frame_overwrites_existing_file`,
//!   `ac2_invalid_size_or_view_is_invalid_input` (SPEC-0003 §2.4/§2.9)
//! - AC3  `ac3_golden_frame_matches_the_checked_in_fixture_byte_for_byte`
//! - AC4  `ac4_first_light_240_frames_rendered_twice_bit_identical`
//! - AC5  `ac5_zero_new_dependencies_and_recorded_ffmpeg_invocation`
//!
//! Exactness levels follow SPEC-0003 §2.5–§2.6: the byte grammar is a fixed
//! template over shortest-roundtrip `f64` `Display`, so every SVG assertion
//! here is full-document byte equality — no XML parser, no tolerance. The
//! AC1 timing law asserts `x.to_bits()` equality because duration 4.0 is
//! dyadic (F11) and garust's translator `log`/`exp` path is exact for a
//! unit-rate translator (architect-verified; calibrated against garust
//! ground truth across all 240 frames, QA probe run 2026-08-20). The AC3
//! fixture was hand-derived from the §2.5 grammar and confirmed against a
//! spec-faithful probe implementation before freezing; the one token where
//! probe and hand derivation disagreed is the triangle apex, whose
//! authored x = 0 reads back from garust's transform as -0.0 and therefore
//! prints `-0` — possible and legal SVG per §2.5, frozen as such (the
//! spec's illustrative block shows `0,0.25`; the pinned grammar over the
//! real bits gives `-0,0.25`). AC4 renders in-tree product code shared via
//! `#[path]` (adjudicated), twice, same process — same-environment
//! bit-identity per §2.6.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use garust::{pga, Motor3};
use motoreel::{FrameSink, Object, Prim2, Pt2, Rgb, Scene, Style, SvgSink, Track};

/// The shipped demo scene, included from the example source itself so the
/// demo and this determinism test cannot drift apart (SPEC-0003 §2.7; the
/// shared file names engine items through `motoreel::` paths only).
/// `rustfmt::skip` keeps the format gate green while the product file does
/// not exist yet (TDD red); once it lands, rustfmt still reaches it
/// through the example target's own `mod scene;`.
#[rustfmt::skip]
#[path = "../examples/first_light/scene.rs"]
mod scene;

// --- Shared helpers ------------------------------------------------------

/// A clean per-test scratch directory under `CARGO_TARGET_TMPDIR`
/// (SPEC-0003's test-infra decision): stale frames from a previous run are
/// removed so file counts are exact. The directory itself is left for the
/// sink under test to create.
fn tmp_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clean the stale test directory");
    }
    dir
}

/// Sorted file names in a directory — zero-padded frame names make the
/// lexicographic order the frame order.
fn dir_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .expect("read the test output directory")
        .map(|entry| {
            entry
                .expect("directory entry")
                .file_name()
                .into_string()
                .expect("UTF-8 file name")
        })
        .collect();
    names.sort();
    names
}

/// One rendered frame as UTF-8 text.
fn read_frame(dir: &Path, index: usize) -> String {
    fs::read_to_string(dir.join(format!("frame_{index:05}.svg"))).expect("read a rendered frame")
}

/// A scene holding one static origin point under the default camera (view
/// depth 5 — always visible): every frame evaluates to exactly one
/// primitive.
fn one_point_scene(duration: f64) -> Scene {
    let mut scene = Scene::new(duration);
    scene.add(Object::point(pga::Point::new(0.0, 0.0, 0.0)));
    scene
}

/// In-memory sink recording every call — file I/O is not the point of AC1.
#[derive(Default)]
struct RecordingSink {
    calls: Vec<(usize, Vec<Prim2>)>,
}

impl FrameSink for RecordingSink {
    fn frame(&mut self, index: usize, prims: &[Prim2]) -> io::Result<()> {
        self.calls.push((index, prims.to_vec()));
        Ok(())
    }
}

/// In-memory sink failing at one frame index with its own error kind.
struct FailingSink {
    fail_at: usize,
    calls: usize,
}

impl FrameSink for FailingSink {
    fn frame(&mut self, index: usize, _prims: &[Prim2]) -> io::Result<()> {
        self.calls += 1;
        if index == self.fail_at {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "deliberate sink failure",
            ));
        }
        Ok(())
    }
}

// --- AC1: the frame walk --------------------------------------------------

// AC1 — frame count is ceil(duration · fps): the integral product 4.0 × 60
// is exactly 240 (not 241), a fractional product gets its partial second a
// frame, and a zero-duration scene walks zero frames returning Ok(())
// without touching the sink. Indices arrive as 0..n in order, exactly one
// call each.
#[test]
fn ac1_frame_count_indices_and_zero_duration() {
    let mut sink = RecordingSink::default();
    one_point_scene(4.0)
        .render(60.0, &mut sink)
        .expect("the walk succeeds");
    assert_eq!(sink.calls.len(), 240, "4.0 s × 60 fps → exactly 240 frames");
    for (i, (index, prims)) in sink.calls.iter().enumerate() {
        assert_eq!(*index, i, "indices are 0..n in order, one call each");
        assert_eq!(prims.len(), 1, "the static point is visible every frame");
    }

    let mut sink = RecordingSink::default();
    one_point_scene(1.01)
        .render(60.0, &mut sink)
        .expect("the walk succeeds");
    assert_eq!(
        sink.calls.len(),
        61,
        "ceil(1.01 × 60) = 61 — the partial second gets a frame"
    );

    let mut sink = RecordingSink::default();
    one_point_scene(0.0)
        .render(60.0, &mut sink)
        .expect("zero frames is Ok(())");
    assert!(sink.calls.is_empty(), "the sink is never called");
}

// AC1 (R-0003 §4, F11) — frame times are derived (t_i = i / fps), never
// accumulated. A unit-rate translator track over the dyadic duration 4.0
// poses the origin point at x = t exactly: the span parameter t / 4.0 is an
// exact power-of-two division and garust's translator log/exp path is exact
// for it (the parabolic log branch scales by 1/c with c = 1, the null exp
// is 1 + self — architect-verified, stub-calibrated over all 240 frames).
// Under the default orthographic camera the emitted x-coordinate must
// therefore equal i as f64 / fps bit-for-bit — a running `t += dt` sum
// would drift off these bits.
#[test]
fn ac1_timing_law_frame_times_are_derived_not_accumulated() {
    let mut scene = Scene::new(4.0);
    scene.add(
        Object::point(pga::Point::new(0.0, 0.0, 0.0)).with_track(
            Track::keys([
                (0.0, Motor3::identity()),
                (4.0, Motor3::translator(4.0, 0.0, 0.0)),
            ])
            .expect("timing-law keys are strictly increasing"),
        ),
    );
    let mut sink = RecordingSink::default();
    scene.render(60.0, &mut sink).expect("the walk succeeds");
    assert_eq!(sink.calls.len(), 240);
    for (index, prims) in &sink.calls {
        let want = *index as f64 / 60.0;
        match prims.as_slice() {
            [Prim2::Point { at, .. }] => {
                if *index == 0 {
                    // t = 0 clamps to the identity key, and garust's
                    // transform reads the origin's x back as -0.0
                    // (calibrated): value-equal to 0/fps. The sign of
                    // zero at the byte level is the golden test's
                    // business, not the timing law's.
                    assert_eq!(at.x, 0.0, "frame 0 sits at the origin");
                } else {
                    assert_eq!(
                        at.x.to_bits(),
                        want.to_bits(),
                        "frame {index}: x must equal i/fps bit-exactly, got {} want {want}",
                        at.x
                    );
                }
                assert_eq!(at.y, 0.0, "frame {index}: the point stays on the x-axis");
            }
            other => panic!("frame {index}: expected exactly one point, got {other:?}"),
        }
    }
}

// AC1 — a sink error at frame k stops the walk with that error, verbatim,
// after exactly k + 1 calls; frames k+1.. are never attempted.
#[test]
fn ac1_sink_error_stops_the_walk() {
    let mut sink = FailingSink {
        fail_at: 3,
        calls: 0,
    };
    let err = one_point_scene(4.0)
        .render(60.0, &mut sink)
        .expect_err("the sink error must propagate");
    assert_eq!(
        err.kind(),
        io::ErrorKind::BrokenPipe,
        "the sink's own error comes back, not a wrapper"
    );
    assert_eq!(
        sink.calls, 4,
        "frames 0..=3 ran; frame 4 was never attempted"
    );
}

// AC1 — non-finite or non-positive fps and a duration failing
// `is_finite() && >= 0` are rejected as `InvalidInput` before any sink
// call (SPEC-0003 §2.3: duration is validated here because SPEC-0002
// deliberately validates nothing).
#[test]
fn ac1_bad_fps_and_bad_duration_are_invalid_input_before_any_sink_call() {
    for fps in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut sink = RecordingSink::default();
        let err = one_point_scene(1.0)
            .render(fps, &mut sink)
            .expect_err("a bad fps must be rejected");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "fps {fps}");
        assert!(sink.calls.is_empty(), "fps {fps}: the sink stays untouched");
    }
    for duration in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut sink = RecordingSink::default();
        let err = one_point_scene(duration)
            .render(60.0, &mut sink)
            .expect_err("a bad duration must be rejected");
        assert_eq!(
            err.kind(),
            io::ErrorKind::InvalidInput,
            "duration {duration}"
        );
        assert!(
            sink.calls.is_empty(),
            "duration {duration}: the sink stays untouched"
        );
    }
}

// AC1 (SPEC-0003 §2.3) — the walk's bound is `S: FrameSink + ?Sized`, so
// the RFC §3.4 call shape also works through a trait object.
#[test]
fn ac1_render_accepts_a_dyn_sink() {
    let mut recording = RecordingSink::default();
    let sink: &mut dyn FrameSink = &mut recording;
    one_point_scene(1.0)
        .render(2.0, sink)
        .expect("rendering through &mut dyn FrameSink works");
    assert_eq!(recording.calls.len(), 2);
}

// --- AC2: the SVG writer, driven through the trait directly ---------------

/// SPEC-0003 §2.5's exact empty document over the default 1920×1080 raster
/// and 3.2 × 1.8 centred view window: `\n` after every line including the
/// last, no indentation, f64s via shortest-roundtrip `Display`.
const EMPTY_DEFAULT_DOC: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8"?>"#,
    "\n",
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="1920" height="1080" viewBox="-1.6 -0.9 3.2 1.8">"#,
    "\n",
    r#"<g transform="scale(1 -1)">"#,
    "\n",
    "</g>\n</svg>\n",
);

/// The same empty document over a custom raster and view window: integral
/// f64s print bare ("4", never "4.0"), and the viewBox is the centred
/// window (−w/2, −h/2, w, h).
const EMPTY_CUSTOM_DOC: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8"?>"#,
    "\n",
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="640" height="480" viewBox="-2 -1 4 2">"#,
    "\n",
    r#"<g transform="scale(1 -1)">"#,
    "\n",
    "</g>\n</svg>\n",
);

/// §2.5's pinned per-primitive templates, one line per primitive in slice
/// order inside the default header/footer: lowercase six-digit hex, style
/// attributes always emitted (`stroke-opacity="1"` included), point =
/// filled circle of radius width/2, polyline pairs "x,y" space-separated
/// with `fill="none"`, round caps and joins.
const TEMPLATE_DOC: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8"?>"#,
    "\n",
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="1920" height="1080" viewBox="-1.6 -0.9 3.2 1.8">"#,
    "\n",
    r#"<g transform="scale(1 -1)">"#,
    "\n",
    r##"<line x1="-1.5" y1="0.75" x2="0.5" y2="-0.25" stroke="#abcdef" stroke-width="0.25" stroke-opacity="1" stroke-linecap="round"/>"##,
    "\n",
    r##"<circle cx="0.3" cy="0.5" r="0.1" fill="#ff0011" fill-opacity="0.25"/>"##,
    "\n",
    r##"<polyline points="0,0 1,0.5 -1,-0.5" fill="none" stroke="#000000" stroke-width="0.03125" stroke-opacity="0.5" stroke-linecap="round" stroke-linejoin="round"/>"##,
    "\n",
    "</g>\n</svg>\n",
);

// AC2 — filenames are `frame_%05d.svg`: zero-padded to five digits, in the
// sink's own directory, indices 0 and 123 exercising the padding.
#[test]
fn ac2_filenames_are_zero_padded_five_digits() {
    let dir = tmp_dir("r0003_names");
    let mut sink = SvgSink::new(&dir).expect("sink construction");
    sink.frame(0, &[]).expect("frame 0");
    sink.frame(123, &[]).expect("frame 123");
    assert_eq!(dir_names(&dir), ["frame_00000.svg", "frame_00123.svg"]);
}

// AC2 (SPEC-0003 §2.4) — `new` creates the output directory at
// construction, nested parents included, so path failures surface before a
// long render.
#[test]
fn ac2_new_creates_nested_output_directory() {
    let dir = tmp_dir("r0003_nested").join("a/b/out");
    assert!(!dir.exists(), "the nested path starts absent");
    let mut sink = SvgSink::new(&dir).expect("new creates parents");
    assert!(dir.is_dir(), "the directory exists after construction");
    sink.frame(0, &[]).expect("a frame lands inside");
    assert_eq!(dir_names(&dir), ["frame_00000.svg"]);
}

// AC2 — an empty primitive slice yields the well-formed empty document:
// pinned header, default viewBox mapped from image space, y-flip group,
// nothing else — byte-for-byte.
#[test]
fn ac2_empty_frame_is_the_well_formed_empty_document() {
    let dir = tmp_dir("r0003_empty");
    let mut sink = SvgSink::new(&dir).expect("sink construction");
    sink.frame(0, &[]).expect("write the empty frame");
    assert_eq!(read_frame(&dir, 0), EMPTY_DEFAULT_DOC);
}

// AC2 — `with_view` puts the raster hint in width/height and the centred
// view window in the viewBox, exactly per §2.5; it also creates its
// (nested) directory like `new` does.
#[test]
fn ac2_custom_view_header_and_viewbox() {
    let dir = tmp_dir("r0003_custom_view").join("nested/out");
    let mut sink = SvgSink::with_view(&dir, (640, 480), (4.0, 2.0)).expect("with_view constructs");
    assert!(dir.is_dir(), "with_view creates the directory");
    sink.frame(7, &[]).expect("write a frame");
    assert_eq!(read_frame(&dir, 7), EMPTY_CUSTOM_DOC);
}

// AC2 — one stroke element per primitive honouring stroke/width/alpha,
// each kind emitting its pinned template, elements in slice order, style
// attributes always present including `stroke-opacity="1"`, non-dyadic
// values (0.3, 0.2/2 → "0.1") through shortest-roundtrip Display.
#[test]
fn ac2_primitive_templates_are_pinned() {
    let dir = tmp_dir("r0003_templates");
    let mut sink = SvgSink::new(&dir).expect("sink construction");
    let prims = [
        Prim2::Segment {
            a: Pt2 { x: -1.5, y: 0.75 },
            b: Pt2 { x: 0.5, y: -0.25 },
            style: Style {
                stroke: Rgb {
                    r: 0xab,
                    g: 0xcd,
                    b: 0xef,
                },
                width: 0.25,
                alpha: 1.0,
            },
        },
        Prim2::Point {
            at: Pt2 { x: 0.3, y: 0.5 },
            style: Style {
                stroke: Rgb {
                    r: 0xff,
                    g: 0x00,
                    b: 0x11,
                },
                width: 0.2,
                alpha: 0.25,
            },
        },
        Prim2::Polyline {
            points: vec![
                Pt2 { x: 0.0, y: 0.0 },
                Pt2 { x: 1.0, y: 0.5 },
                Pt2 { x: -1.0, y: -0.5 },
            ],
            style: Style {
                stroke: Rgb { r: 0, g: 0, b: 0 },
                width: 0.03125,
                alpha: 0.5,
            },
        },
    ];
    sink.frame(0, &prims).expect("write the template frame");
    assert_eq!(read_frame(&dir, 0), TEMPLATE_DOC);
}

// AC2 (SPEC-0003 §2.4) — `frame` overwrites an existing file wholesale
// (`fs::write` truncates): a re-render over a stale directory replaces
// frames, it never appends and never deletes.
#[test]
fn ac2_frame_overwrites_existing_file() {
    let dir = tmp_dir("r0003_overwrite");
    let mut sink = SvgSink::new(&dir).expect("sink construction");
    let segment = Prim2::Segment {
        a: Pt2 { x: 0.0, y: 0.0 },
        b: Pt2 { x: 1.0, y: 1.0 },
        style: Style::default(),
    };
    sink.frame(0, &[segment]).expect("first write");
    assert_ne!(read_frame(&dir, 0), EMPTY_DEFAULT_DOC);
    sink.frame(0, &[]).expect("second write");
    assert_eq!(
        read_frame(&dir, 0),
        EMPTY_DEFAULT_DOC,
        "the file is replaced, not appended to"
    );
}

// AC2 (SPEC-0003 §2.4/§2.9) — zero raster dimensions and non-finite or
// non-positive view windows are rejected at construction as
// `InvalidInput`, the same policy as the walk's validation.
#[test]
fn ac2_invalid_size_or_view_is_invalid_input() {
    let dir = tmp_dir("r0003_invalid");
    let cases: [((u32, u32), (f64, f64)); 6] = [
        ((0, 1080), (3.2, 1.8)),
        ((1920, 0), (3.2, 1.8)),
        ((64, 64), (0.0, 1.8)),
        ((64, 64), (-3.2, 1.8)),
        ((64, 64), (f64::NAN, 1.8)),
        ((64, 64), (3.2, f64::INFINITY)),
    ];
    for (size, view) in cases {
        match SvgSink::with_view(&dir, size, view) {
            Err(err) => assert_eq!(
                err.kind(),
                io::ErrorKind::InvalidInput,
                "size {size:?} view {view:?}"
            ),
            Ok(_) => panic!("size {size:?} view {view:?} must be rejected"),
        }
    }
}

// --- AC3: the golden frame ------------------------------------------------

/// The checked-in golden fixture: hand-derived from §2.5's pinned grammar,
/// probe-confirmed, frozen — including the `-0,0.25` apex token (the
/// authored x = 0 reads back as -0.0; see the module doc). Regenerate only
/// via `MOTOREEL_BLESS=1 cargo test -p motoreel --test r0003_svg_sink`,
/// then review the diff like code (SPEC-0003 §6 AC3).
const GOLDEN: &[u8] = include_bytes!("golden/frame_00000.svg");

/// SPEC-0003 §3's trig-free golden scene: one segment, one point dot, one
/// closed triangle polyline, every vertex authored in the z = 0 plane,
/// viewed by the `Camera::default()` that `Scene::new` installs — the
/// orthographic camera at translator(0, 0, 5), translator-only and dyadic,
/// so every vertex sits at view depth 5 and the platform libm is never
/// touched: the fixture's bytes are portable (§2.6). Orthographic
/// projection ignores depth, so the image coordinates are the authored
/// x/y unchanged.
fn golden_scene() -> Scene {
    let grey = Style {
        stroke: Rgb {
            r: 0xe0,
            g: 0xe0,
            b: 0xe0,
        },
        width: 0.02,
        alpha: 1.0,
    };
    let orange = Style {
        stroke: Rgb {
            r: 0xff,
            g: 0x4d,
            b: 0x00,
        },
        width: 0.06, // the dot renders as a filled circle of radius 0.03
        alpha: 1.0,
    };
    let teal = Style {
        stroke: Rgb {
            r: 0x00,
            g: 0xb4,
            b: 0xd8,
        },
        width: 0.02,
        alpha: 0.5,
    };
    let mut scene = Scene::new(1.0);
    scene.add(
        Object::segment(
            pga::Point::new(-1.0, 0.0, 0.0),
            pga::Point::new(1.0, 0.0, 0.0),
        )
        .with_style(grey),
    );
    scene.add(Object::point(pga::Point::new(0.5, 0.5, 0.0)).with_style(orange));
    scene.add(
        Object::polyline(vec![
            pga::Point::new(-0.5, -0.5, 0.0),
            pga::Point::new(0.5, -0.5, 0.0),
            pga::Point::new(0.0, 0.25, 0.0),
            pga::Point::new(-0.5, -0.5, 0.0),
        ])
        .with_style(teal),
    );
    scene
}

// AC3 — one frame of the known scene, rendered at 1 fps over duration 1.0
// (exactly one frame, t = 0), equals the checked-in fixture byte-for-byte.
#[test]
fn ac3_golden_frame_matches_the_checked_in_fixture_byte_for_byte() {
    let dir = tmp_dir("r0003_golden");
    let mut sink = SvgSink::new(&dir).expect("sink construction");
    golden_scene()
        .render(1.0, &mut sink)
        .expect("golden render");
    assert_eq!(
        dir_names(&dir),
        ["frame_00000.svg"],
        "duration 1.0 at 1 fps is exactly one frame"
    );
    let got = fs::read(dir.join("frame_00000.svg")).expect("read the rendered frame");
    if std::env::var_os("MOTOREEL_BLESS").is_some_and(|v| v == "1") {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/frame_00000.svg");
        fs::write(&fixture, &got).expect("bless the golden fixture");
        eprintln!("blessed {} — review the diff like code", fixture.display());
        return;
    }
    assert!(
        got == GOLDEN,
        "the golden frame drifted from the checked-in fixture\n\
         --- rendered ---\n{}--- fixture ---\n{}",
        String::from_utf8_lossy(&got),
        String::from_utf8_lossy(GOLDEN)
    );
}

// --- AC4: first light -----------------------------------------------------

// AC4 — the shipped demo scene (RFC-012 §3.5 screw demo, join-line
// omitted), rendered at 60 fps for its 4 s, produces exactly 240 frames —
// frame_00000.svg through frame_00239.svg, nothing else — and rendering it
// twice in the same process is bit-identical on every corresponding pair
// (SPEC-0003 §2.6: same-environment determinism; trig is fine here).
#[test]
fn ac4_first_light_240_frames_rendered_twice_bit_identical() {
    let base = tmp_dir("r0003_first_light");
    let (a, b) = (base.join("a"), base.join("b"));
    let mut sink = SvgSink::new(&a).expect("sink a");
    scene::scene().render(60.0, &mut sink).expect("render a");
    let mut sink = SvgSink::new(&b).expect("sink b");
    scene::scene().render(60.0, &mut sink).expect("render b");

    let want_names: Vec<String> = (0..240).map(|i| format!("frame_{i:05}.svg")).collect();
    assert_eq!(dir_names(&a), want_names, "exactly 240 frames in a/");
    assert_eq!(dir_names(&b), want_names, "exactly 240 frames in b/");
    for name in &want_names {
        let frame_a = fs::read(a.join(name)).expect("frame in a/");
        let frame_b = fs::read(b.join(name)).expect("frame in b/");
        assert!(!frame_a.is_empty(), "{name} must not be empty");
        assert!(frame_a == frame_b, "{name} differs between the two renders");
    }
}

// --- AC5: zero new dependencies, documented encode line -------------------

/// The dependency names under one `[section]` of the crate's small
/// manifest — just enough TOML to audit AC5 without adding a parser
/// dependency.
fn section_keys(manifest: &str, section: &str) -> Vec<String> {
    let header = format!("[{section}]");
    let mut keys = Vec::new();
    let mut inside = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            inside = line == header;
        } else if inside && !line.is_empty() && !line.starts_with('#') {
            if let Some((key, _)) = line.split_once('=') {
                keys.push(key.trim().to_string());
            }
        }
    }
    keys
}

// AC5 — zero new dependencies: the normal graph stays garust alone, the
// dev graph proptest alone, and no build- or target-specific dependency
// table exists. The documented ffmpeg invocation is recorded with the
// demo. Nothing here (or anywhere in this suite) runs ffmpeg — encoding
// stays outside the crate and outside CI.
#[test]
fn ac5_zero_new_dependencies_and_recorded_ffmpeg_invocation() {
    let manifest = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .expect("read the crate manifest");
    assert_eq!(section_keys(&manifest, "dependencies"), ["garust"]);
    assert_eq!(section_keys(&manifest, "dev-dependencies"), ["proptest"]);
    assert!(
        !manifest.contains("[build-dependencies]"),
        "no build dependencies"
    );
    assert!(
        !manifest.contains(".dependencies]"),
        "no target-specific dependency tables"
    );

    let main_rs = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/first_light/main.rs"),
    )
    .expect("examples/first_light/main.rs exists");
    assert!(
        main_rs.contains("ffmpeg -framerate 60 -i out/frame_%05d.svg"),
        "the ffmpeg invocation must be documented with the demo"
    );
}
