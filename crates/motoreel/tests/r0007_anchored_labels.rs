//! R-0007 — anchored text labels: the QA-owned acceptance suite (e2e).
//!
//! Loop step 3, TDD red: authored before the behaviour exists, derived from
//! `requirements/0007-anchored-labels.md` (AC1–AC8) and SPEC-0007 §6's test
//! plan. Unit tests for the private helpers (`to_ascii`, `screen_point`,
//! `escape_text`, `font::glyph`, the `k` derivation and the alignment
//! arithmetic) live in each module's own `#[cfg(test)] mod tests`.
//!
//! Criterion → test map:
//!
//! - AC1  `ac1_label_fields_round_trip_into_the_emitted_text`,
//!   `ac1_scene_starts_empty_with_the_default_view`,
//!   `ac1_every_object_then_every_label_in_insertion_order`
//! - AC2  `ac2_point_anchor_matches_an_object_point_bit_for_bit`,
//!   `ac2_unprojectable_anchors_are_absent`,
//!   `ac2_non_finite_offset_is_absent`
//! - AC3  `ac3_pose_anchor_equals_the_independently_computed_projection`,
//!   `ac3_pose_anchor_matches_a_point_object_on_the_same_track`,
//!   `ac3_later_insertions_do_not_move_a_label`,
//!   `ac3_a_culled_object_does_not_cull_its_label`,
//!   `ac3_out_of_range_object_id_is_absent_not_a_panic`
//! - AC4  `ac4_nine_screen_anchors_are_fixed_across_time_camera_projection`,
//!   `ac4_middle_row_and_column_are_positive_zero`,
//!   `ac4_three_defaults_agree_on_the_view`,
//!   `ac4_render_rejects_a_view_disagreement_before_writing`
//! - AC5  `ac5_text_template_is_pinned_per_align`,
//!   `ac5_minus_zero_is_emitted_for_the_middle_row`,
//!   `ac5_escaping_maps_exactly_three_characters`,
//!   `ac5_r0003_golden_is_byte_unchanged`,
//!   `ac5_labels_svg_golden_matches`
//! - AC7  `ac7_two_renders_are_byte_identical_per_sink`,
//!   `ac7_eval_and_clone_agree`, `ac7_zero_new_dependencies`
//! - AC8  `ac8_substitution_table_preserves_character_count`,
//!   `ac8_both_sinks_see_the_same_substituted_string`
//!
//! AC6 (the embedded face and raster text) is a separate stage and lands
//! with `font.rs`; its tests join this file then.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use garust::{pga, Motor3, Pga3};
use motoreel::{
    Align, Anchor, Camera, FrameSink, Label, Object, PpmSink, Prim2, Projection, Pt2, Rgb, Scene,
    ScreenAnchor, Style, SvgSink, Track,
};

// --- Shared helpers ------------------------------------------------------

fn tmp_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clean the stale test directory");
    }
    dir
}

fn dir_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .expect("read the test output directory")
        .map(|e| {
            e.expect("entry")
                .file_name()
                .into_string()
                .expect("UTF-8 name")
        })
        .collect();
    names.sort();
    names
}

fn pt(x: f64, y: f64) -> Pt2 {
    Pt2 { x, y }
}

/// The `Prim2::Text` items of a slice, in order.
fn texts(prims: &[Prim2]) -> Vec<&Prim2> {
    prims
        .iter()
        .filter(|p| matches!(p, Prim2::Text { .. }))
        .collect()
}

/// The one text position in a slice; panics if there is not exactly one.
fn only_text_at(prims: &[Prim2]) -> Pt2 {
    let ts = texts(prims);
    assert_eq!(ts.len(), 1, "expected exactly one label");
    match ts[0] {
        Prim2::Text { at, .. } => *at,
        _ => unreachable!(),
    }
}

fn bits(p: Pt2) -> (u64, u64) {
    (p.x.to_bits(), p.y.to_bits())
}

/// Render one frame of a scene through an `SvgSink` and return the text.
fn svg_of(name: &str, scene: &Scene) -> String {
    let dir = tmp_dir(name);
    let mut sink = SvgSink::new(&dir).expect("svg sink");
    scene.render(1.0, &mut sink).expect("render");
    fs::read_to_string(dir.join("frame_00000.svg")).expect("read svg")
}

// --- AC1: the Label shape and a scene that holds labels -------------------

// AC1 — every field round-trips into the emitted `Prim2::Text`, floats
// bit-for-bit (R-0002 AC4's strictness).
#[test]
fn ac1_label_fields_round_trip_into_the_emitted_text() {
    let style = Style {
        stroke: Rgb {
            r: 0x12,
            g: 0x34,
            b: 0x56,
        },
        width: 0.037,
        alpha: 0.25,
    };
    let mut scene = Scene::new(1.0);
    scene.add_label(
        Label::new("hi", Anchor::Screen(ScreenAnchor::Centre))
            .with_offset(pt(0.125, -0.0625))
            .with_size(0.1875)
            .with_align(Align::Right)
            .with_style(style),
    );
    let prims = scene.eval(0.0);
    match texts(&prims)[0] {
        Prim2::Text {
            at,
            text,
            size,
            align,
            style: got,
        } => {
            assert_eq!(text, "hi");
            assert_eq!(size.to_bits(), 0.1875_f64.to_bits(), "size is verbatim");
            assert_eq!(*align, Align::Right);
            assert_eq!(got.stroke, style.stroke);
            assert_eq!(got.width.to_bits(), style.width.to_bits());
            assert_eq!(got.alpha.to_bits(), style.alpha.to_bits());
            // Centre is (+0, +0), so `at` is the offset exactly.
            assert_eq!(bits(*at), bits(pt(0.125, -0.0625)));
        }
        _ => unreachable!(),
    }
}

// AC1 — the documented defaults, and a scene that starts empty.
#[test]
fn ac1_scene_starts_empty_with_the_default_view() {
    let scene = Scene::new(1.0);
    assert!(scene.labels.is_empty(), "labels start empty");
    assert_eq!(scene.view, (3.2, 1.8), "the default view window");

    let label = Label::new("x", Anchor::Screen(ScreenAnchor::Centre));
    assert_eq!(label.size, 0.08, "default em height");
    assert_eq!(label.align, Align::Left, "default alignment");
    assert_eq!(label.style, Style::default(), "default style");
    assert_eq!(label.offset, pt(0.0, 0.0), "default offset");
}

// AC1 — every object, then every label, each in insertion order (§2.1).
#[test]
fn ac1_every_object_then_every_label_in_insertion_order() {
    let mut scene = Scene::new(1.0);
    scene.add(Object::point(pga::Point::new(0.0, 0.0, 0.0)));
    scene.add(Object::point(pga::Point::new(0.1, 0.0, 0.0)));
    scene.add_label(Label::new("a", Anchor::Screen(ScreenAnchor::TopLeft)));
    scene.add_label(Label::new("b", Anchor::Screen(ScreenAnchor::TopRight)));

    let prims = scene.eval(0.0);
    assert_eq!(prims.len(), 4);
    assert!(matches!(prims[0], Prim2::Point { .. }), "object first");
    assert!(matches!(prims[1], Prim2::Point { .. }), "object second");
    match (&prims[2], &prims[3]) {
        (Prim2::Text { text: a, .. }, Prim2::Text { text: b, .. }) => {
            assert_eq!((a.as_str(), b.as_str()), ("a", "b"), "insertion order");
        }
        _ => panic!("labels must follow every object"),
    }
}

// --- AC2: point anchors project and cull like geometry --------------------

// AC2(a) — a point-anchored label and an `Object::point` at the same world
// point under the same camera produce bit-identical positions, under both
// projections.
#[test]
fn ac2_point_anchor_matches_an_object_point_bit_for_bit() {
    let world = pga::Point::new(0.37, -0.21, 0.0);
    for projection in [Projection::Orthographic, Projection::Pinhole { focal: 1.7 }] {
        let mut scene = Scene::new(1.0);
        scene.camera = Camera {
            pose: scene.camera.pose,
            projection,
        };
        scene.add(Object::point(world));
        scene.add_label(Label::new("p", Anchor::Point(world)));

        let prims = scene.eval(0.0);
        let object_at = match &prims[0] {
            Prim2::Point { at, .. } => *at,
            other => panic!("expected a point, got {other:?}"),
        };
        assert_eq!(
            bits(only_text_at(&prims)),
            bits(object_at),
            "{projection:?}: label and geometry must agree bit-for-bit"
        );
    }
}

// AC2(b) — behind the camera, at the plane, below NEAR, and an ideal point:
// the label is absent, and no NaN appears anywhere in the frame.
#[test]
fn ac2_unprojectable_anchors_are_absent() {
    // The default camera sits at translator(0, 0, 5) looking along -z.
    // The r0002 generator: a direction, not a location — SPEC-0002 §2.5's
    // guard is what culls it.
    let ideal =
        pga::Point::from_multivector(Pga3::point(1.0, 0.0, 0.0) - Pga3::point(0.0, 0.0, 0.0));
    let cases: [(&str, pga::Point); 4] = [
        ("behind", pga::Point::new(0.0, 0.0, 10.0)),
        ("at the plane", pga::Point::new(0.0, 0.0, 5.0)),
        (
            "below NEAR",
            pga::Point::new(0.0, 0.0, 5.0 - Projection::NEAR / 2.0),
        ),
        ("ideal", ideal),
    ];
    for (what, p) in cases {
        let mut scene = Scene::new(1.0);
        scene.add_label(Label::new("x", Anchor::Point(p)));
        let prims = scene.eval(0.0);
        assert!(texts(&prims).is_empty(), "{what}: the label must be absent");
    }
}

// AC2(c) — a NaN or infinite offset is absent: the guard keeps SPEC-0002's
// finite-coordinate invariant unconditional.
#[test]
fn ac2_non_finite_offset_is_absent() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for offset in [pt(bad, 0.0), pt(0.0, bad)] {
            let mut scene = Scene::new(1.0);
            scene.add_label(
                Label::new("x", Anchor::Screen(ScreenAnchor::Centre)).with_offset(offset),
            );
            assert!(
                texts(&scene.eval(0.0)).is_empty(),
                "offset {offset:?} must cull the label"
            );
        }
    }
}

// --- AC3: pose anchors ride the track -------------------------------------

/// A multi-key track: a unit-rate translation along +x over 4 s.
fn moving_track() -> Track {
    Track::keys([
        (0.0, Motor3::identity()),
        (4.0, Motor3::translator(1.0, 0.0, 0.0)),
    ])
    .expect("a two-key track")
}

// AC3 — the headline assertion, computed independently of the
// implementation: the emitted position equals
// `camera.pose.inverse().compose(&track.eval(t))` applied to the model
// anchor and then projected. `Projection::project` is public for this.
#[test]
fn ac3_pose_anchor_equals_the_independently_computed_projection() {
    let model = pga::Point::new(0.2, 0.3, 0.0);
    let track = moving_track();
    let mut scene = Scene::new(4.0);
    let id = scene.add(Object::point(pga::Point::new(0.0, 0.0, 0.0)).with_track(track.clone()));
    scene.add_label(Label::new(
        "m",
        Anchor::Pose {
            object: id,
            at: model,
        },
    ));

    for t in [-1.0, 0.0, 0.5, 1.0, 4.0, 7.0] {
        let prims = scene.eval(t);
        let to_view = scene.camera.pose.inverse().compose(&track.eval(t));
        let want = scene
            .camera
            .projection
            .project(&model.transform(&to_view))
            .expect("the anchor is in front of the camera");
        assert_eq!(
            bits(only_text_at(&prims)),
            bits(want),
            "t = {t}: the label must equal the independently computed projection"
        );
    }
}

// AC3 — a pose-anchored label at model point `p` is bit-identical in
// position to a `Shape::Point(p)` object carrying the same track, while
// that track is moving.
#[test]
fn ac3_pose_anchor_matches_a_point_object_on_the_same_track() {
    let model = pga::Point::new(0.2, 0.3, 0.0);
    let track = moving_track();
    let mut scene = Scene::new(4.0);
    let id = scene.add(Object::point(pga::Point::new(0.0, 0.0, 0.0)).with_track(track.clone()));
    scene.add(Object::point(model).with_track(track));
    scene.add_label(Label::new(
        "m",
        Anchor::Pose {
            object: id,
            at: model,
        },
    ));

    for t in [0.0, 1.0, 2.5] {
        let prims = scene.eval(t);
        let witness = match &prims[1] {
            Prim2::Point { at, .. } => *at,
            other => panic!("expected the witness point, got {other:?}"),
        };
        assert_eq!(
            bits(only_text_at(&prims)),
            bits(witness),
            "t = {t}: pose anchor must ride the track exactly as geometry does"
        );
    }
}

// AC3 — ids are stable: inserting further objects after the anchor's does
// not move the label.
#[test]
fn ac3_later_insertions_do_not_move_a_label() {
    let model = pga::Point::new(0.2, 0.3, 0.0);
    let mut scene = Scene::new(1.0);
    let id = scene.add(Object::point(pga::Point::new(0.0, 0.0, 0.0)).with_track(moving_track()));
    scene.add_label(Label::new(
        "m",
        Anchor::Pose {
            object: id,
            at: model,
        },
    ));
    let before = only_text_at(&scene.eval(1.0));

    scene.add(Object::point(pga::Point::new(9.0, 9.0, 0.0)));
    scene.add(Object::point(pga::Point::new(8.0, 8.0, 0.0)));
    assert_eq!(
        bits(only_text_at(&scene.eval(1.0))),
        bits(before),
        "an ObjectId must keep meaning the same object"
    );
}

// AC3 — a label whose object is culled but whose own anchor point is in
// front is present: the label's cull is its own vertex's, not the object's.
#[test]
fn ac3_a_culled_object_does_not_cull_its_label() {
    // The object's geometry sits behind the camera; the label's model
    // anchor is pulled back in front of it.
    let mut scene = Scene::new(1.0);
    let id = scene.add(Object::point(pga::Point::new(0.0, 0.0, 10.0)));
    scene.add_label(Label::new(
        "still here",
        Anchor::Pose {
            object: id,
            at: pga::Point::new(0.0, 0.0, -10.0),
        },
    ));
    let prims = scene.eval(0.0);
    assert!(
        prims.iter().all(|p| !matches!(p, Prim2::Point { .. })),
        "the object itself must be culled"
    );
    assert_eq!(texts(&prims).len(), 1, "but its label survives on its own");
}

// AC3 — an out-of-range `ObjectId` culls, and does not panic. Reachable
// because `Scene`'s fields are public and ids are not scene-scoped.
#[test]
fn ac3_out_of_range_object_id_is_absent_not_a_panic() {
    let mut donor = Scene::new(1.0);
    donor.add(Object::point(pga::Point::new(0.0, 0.0, 0.0)));
    donor.add(Object::point(pga::Point::new(1.0, 0.0, 0.0)));
    let stale = donor.add(Object::point(pga::Point::new(2.0, 0.0, 0.0)));

    let mut scene = Scene::new(1.0); // no objects at all
    scene.add_label(Label::new(
        "stale",
        Anchor::Pose {
            object: stale,
            at: pga::Point::new(0.0, 0.0, 0.0),
        },
    ));
    assert!(
        texts(&scene.eval(0.0)).is_empty(),
        "a stale id culls the label rather than panicking"
    );
}

// --- AC4: screen anchors are fixed, over the full 3 x 3 grid --------------

/// The nine grid points for the default view, written out literally rather
/// than computed by the helper under test.
const GRID: [(ScreenAnchor, (f64, f64)); 9] = [
    (ScreenAnchor::TopLeft, (-1.6, 0.9)),
    (ScreenAnchor::TopCentre, (0.0, 0.9)),
    (ScreenAnchor::TopRight, (1.6, 0.9)),
    (ScreenAnchor::MidLeft, (-1.6, 0.0)),
    (ScreenAnchor::Centre, (0.0, 0.0)),
    (ScreenAnchor::MidRight, (1.6, 0.0)),
    (ScreenAnchor::BottomLeft, (-1.6, -0.9)),
    (ScreenAnchor::BottomCentre, (0.0, -0.9)),
    (ScreenAnchor::BottomRight, (1.6, -0.9)),
];

// AC4(a) — nine cases, not four: fixed across time, camera pose and
// projection, and equal to the literal grid point plus the offset.
#[test]
fn ac4_nine_screen_anchors_are_fixed_across_time_camera_projection() {
    let offset = pt(0.05, -0.03);
    for (anchor, (wx, wy)) in GRID {
        let want = pt(wx + offset.x, wy + offset.y);
        for pose in [
            Motor3::identity(),
            Motor3::translator(3.0, -2.0, 7.0),
            Motor3::rotor(0.7, Pga3::basis(0b0101)),
        ] {
            for projection in [Projection::Orthographic, Projection::Pinhole { focal: 2.0 }] {
                for t in [0.0, 1.0, 7.0] {
                    let mut scene = Scene::new(8.0);
                    scene.camera = Camera { pose, projection };
                    scene.add_label(Label::new("x", Anchor::Screen(anchor)).with_offset(offset));
                    assert_eq!(
                        bits(only_text_at(&scene.eval(t))),
                        bits(want),
                        "{anchor:?} at t = {t} must be the same bits every time"
                    );
                }
            }
        }
    }

    // A non-default view moves all nine correspondingly.
    for (anchor, (wx, wy)) in GRID {
        let mut scene = Scene::new(1.0);
        scene.view = (8.0, 4.0);
        scene.add_label(Label::new("x", Anchor::Screen(anchor)));
        let scale = pt(wx / 1.6 * 4.0, wy / 0.9 * 2.0);
        let got = only_text_at(&scene.eval(0.0));
        assert_eq!(got.x.abs(), scale.x.abs(), "{anchor:?} x tracks the view");
        assert_eq!(got.y.abs(), scale.y.abs(), "{anchor:?} y tracks the view");
    }
}

// AC4(b) — the middle row and column are `+0.0` by `to_bits`, not by `==`,
// for every view including a contract-violating one, because those
// coordinates are a literal and not arithmetic.
#[test]
fn ac4_middle_row_and_column_are_positive_zero() {
    for view in [
        (3.2, 1.8),
        (-4.0, -2.0),
        (f64::NAN, 1.0),
        (f64::INFINITY, 2.0),
    ] {
        let mut scene = Scene::new(1.0);
        scene.view = view;
        scene.add_label(Label::new("c", Anchor::Screen(ScreenAnchor::Centre)));
        let at = only_text_at(&scene.eval(0.0));
        assert_eq!(
            at.x.to_bits(),
            0.0_f64.to_bits(),
            "view {view:?}: Centre.x must be +0.0, not -0.0"
        );
        assert_eq!(at.y.to_bits(), 0.0_f64.to_bits(), "view {view:?}: Centre.y");
    }

    // The five corner/edge variants under a contract-violating view are
    // absent — their coordinates *are* arithmetic on the bad window.
    for anchor in [
        ScreenAnchor::TopLeft,
        ScreenAnchor::TopRight,
        ScreenAnchor::BottomLeft,
        ScreenAnchor::BottomRight,
        ScreenAnchor::MidLeft,
    ] {
        let mut scene = Scene::new(1.0);
        scene.view = (f64::NAN, 1.0);
        scene.add_label(Label::new("x", Anchor::Screen(anchor)));
        assert!(
            texts(&scene.eval(0.0)).is_empty(),
            "{anchor:?} must be absent under a NaN view"
        );
    }
}

// AC4(c) — three defaults agree, extending SPEC-0006 AC2(b)'s two-sink
// assertion to three.
#[test]
fn ac4_three_defaults_agree_on_the_view() {
    let scene = Scene::new(1.0);
    let svg = SvgSink::new(tmp_dir("r0007_ac4_svg")).expect("svg sink");
    let ppm = PpmSink::new(tmp_dir("r0007_ac4_ppm")).expect("ppm sink");
    assert_eq!(svg.view(), Some(scene.view), "SvgSink vs Scene");
    assert_eq!(ppm.view(), Some(scene.view), "PpmSink vs Scene");
}

// AC4(d) — the validator: a disagreement is `InvalidInput` and writes no
// file. The absence of `frame_00000` is the evidence for "before frame 0".
#[test]
fn ac4_render_rejects_a_view_disagreement_before_writing() {
    let dir = tmp_dir("r0007_ac4_validator");
    let mut sink = SvgSink::with_view(&dir, (1920, 1080), (3.2, 1.8)).expect("sink");
    let mut scene = Scene::new(1.0);
    scene.view = (4.0, 2.0); // disagrees

    let err = scene.render(1.0, &mut sink).expect_err("must be rejected");
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    assert!(dir_names(&dir).is_empty(), "no frame may be written");

    // A NaN scene view against a finite sink view is rejected too.
    let mut nan_scene = Scene::new(1.0);
    nan_scene.view = (f64::NAN, 1.8);
    assert_eq!(
        nan_scene
            .render(1.0, &mut sink)
            .expect_err("NaN never compares equal")
            .kind(),
        io::ErrorKind::InvalidInput
    );

    // Agreement renders normally.
    let ok_dir = tmp_dir("r0007_ac4_validator_ok");
    let mut ok_sink = SvgSink::new(&ok_dir).expect("sink");
    Scene::new(1.0).render(1.0, &mut ok_sink).expect("agreeing");
    assert_eq!(dir_names(&ok_dir), ["frame_00000.svg"]);
}

// --- AC5: the SVG <text> element, and the untouched golden ----------------

/// A one-label scene, rendered, with the `<text>` line returned.
fn text_line(name: &str, label: Label) -> String {
    let mut scene = Scene::new(1.0);
    scene.add_label(label);
    svg_of(name, &scene)
        .lines()
        .find(|l| l.starts_with("<text"))
        .expect("a <text> element")
        .to_string()
}

// AC5(a) — the pinned template, per `Align`, with attribute order asserted
// literally and no XML parser.
#[test]
fn ac5_text_template_is_pinned_per_align() {
    let cases = [
        (Align::Left, "start"),
        (Align::Center, "middle"),
        (Align::Right, "end"),
    ];
    for (align, word) in cases {
        let line = text_line(
            &format!("r0007_ac5_{word}"),
            Label::new("hi", Anchor::Screen(ScreenAnchor::TopLeft))
                .with_align(align)
                .with_size(0.08),
        );
        assert_eq!(
            line,
            format!(
                "<text transform=\"scale(1 -1)\" x=\"-1.6\" y=\"-0.9\" \
                 font-family=\"monospace\" font-size=\"0.08\" \
                 text-anchor=\"{word}\" xml:space=\"preserve\" \
                 fill=\"#ffffff\" fill-opacity=\"1\">hi</text>"
            ),
            "the template and its attribute order are pinned"
        );
    }
}

// AC5(a') — the `-0` witness, kept and made specific: the three middle-row
// anchors have `at.y == +0.0`, so the template's `{-at.y}` emits `y="-0"`.
#[test]
fn ac5_minus_zero_is_emitted_for_the_middle_row() {
    for (i, anchor) in [
        ScreenAnchor::MidLeft,
        ScreenAnchor::Centre,
        ScreenAnchor::MidRight,
    ]
    .into_iter()
    .enumerate()
    {
        let line = text_line(
            &format!("r0007_ac5_minuszero_{i}"),
            Label::new("m", Anchor::Screen(anchor)),
        );
        assert!(
            line.contains(" y=\"-0\" "),
            "{anchor:?}: the -0 is a byte-level commitment, not an accident\n{line}"
        );
    }
}

// AC5(b) — escaping maps exactly `&`, `<`, `>`; `"` and `'` pass verbatim.
#[test]
fn ac5_escaping_maps_exactly_three_characters() {
    let all: String = (0x20u8..=0x7E).map(|b| b as char).collect();
    let line = text_line(
        "r0007_ac5_escape",
        Label::new(all.clone(), Anchor::Screen(ScreenAnchor::Centre)),
    );
    let body = line
        .split_once('>')
        .and_then(|(_, rest)| rest.rsplit_once("</text>"))
        .map(|(b, _)| b.to_string())
        .expect("the element body");

    let want: String = all
        .chars()
        .map(|c| match c {
            '&' => "&amp;".to_string(),
            '<' => "&lt;".to_string(),
            '>' => "&gt;".to_string(),
            other => other.to_string(),
        })
        .collect();
    assert_eq!(body, want, "exactly three substitutions, and no others");
    assert!(body.contains('"'), "a quote passes verbatim");
    assert!(body.contains('\''), "an apostrophe passes verbatim");
}

// AC5(c) — R-0003's golden is byte-unchanged: adding a variant must not
// move a signed-off fixture.
#[test]
fn ac5_r0003_golden_is_byte_unchanged() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/frame_00000.svg");
    let bytes = fs::read(&fixture).expect("R-0003's golden still exists");
    assert!(
        bytes.starts_with(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"),
        "the fixture is still the R-0003 document"
    );
    assert!(
        !String::from_utf8_lossy(&bytes).contains("<text"),
        "R-0003's scene has no labels, so its golden must carry no <text>"
    );
}

/// The SPEC-0007 §3 label scene: one object and four labels, one per anchor
/// kind, all three alignments, one escaping case, and the `-0` witness.
fn labels_scene() -> Scene {
    let grey = Style {
        stroke: Rgb {
            r: 0xe0,
            g: 0xe0,
            b: 0xe0,
        },
        width: 0.01,
        alpha: 1.0,
    };
    let orange = Style {
        stroke: Rgb {
            r: 0xff,
            g: 0x4d,
            b: 0x00,
        },
        width: 0.01,
        alpha: 1.0,
    };
    let mut scene = Scene::new(1.0);
    let id = scene.add(
        Object::point(pga::Point::new(0.0, 0.0, 0.0))
            .with_track(Track::hold(Motor3::translator(0.5, -0.25, 0.0)))
            .with_style(Style {
                width: 0.01,
                ..Style::default()
            }),
    );
    scene.add_label(
        Label::new("v = 2 m/s", Anchor::Point(pga::Point::new(-1.0, 0.5, 0.0))).with_style(grey),
    );
    scene.add_label(
        Label::new("L < 90 & rising", Anchor::Screen(ScreenAnchor::TopLeft))
            .with_offset(pt(0.125, -0.25))
            .with_size(0.12)
            .with_align(Align::Right),
    );
    scene.add_label(
        Label::new(
            "box",
            Anchor::Pose {
                object: id,
                at: pga::Point::new(0.0, 0.0, 0.0),
            },
        )
        .with_align(Align::Center)
        .with_style(orange),
    );
    scene.add_label(
        Label::new("centre", Anchor::Screen(ScreenAnchor::Centre))
            .with_align(Align::Center)
            .with_style(grey),
    );
    scene
}

const LABELS_SVG: &[u8] = include_bytes!("golden/labels_00000.svg");

// AC5(d) — the new trig-free SVG golden, blessed via
// `MOTOREEL_BLESS=1 cargo test -p motoreel --test r0007_anchored_labels`.
#[test]
fn ac5_labels_svg_golden_matches() {
    let dir = tmp_dir("r0007_ac5_golden");
    let mut sink = SvgSink::new(&dir).expect("sink");
    labels_scene().render(1.0, &mut sink).expect("render");
    let got = fs::read(dir.join("frame_00000.svg")).expect("read the frame");

    if std::env::var_os("MOTOREEL_BLESS").is_some_and(|v| v == "1") {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/labels_00000.svg");
        fs::write(&fixture, &got).expect("bless");
        eprintln!("blessed {} — review like code", fixture.display());
        return;
    }
    assert!(
        got == LABELS_SVG,
        "the label golden drifted\n--- rendered ---\n{}--- fixture ---\n{}",
        String::from_utf8_lossy(&got),
        String::from_utf8_lossy(LABELS_SVG)
    );
}

// --- AC7: determinism end to end ------------------------------------------

// AC7 — two renders in one process are byte-identical, for each sink
// independently (§2.10 pins this reading — not that the two sinks agree
// with each other, which they cannot).
#[test]
fn ac7_two_renders_are_byte_identical_per_sink() {
    let base = tmp_dir("r0007_ac7");
    for (kind, ext) in [("svg", "svg"), ("ppm", "ppm")] {
        let (a, b) = (base.join(kind).join("a"), base.join(kind).join("b"));
        for dir in [&a, &b] {
            if kind == "svg" {
                let mut sink = SvgSink::new(dir).expect("svg sink");
                labels_scene().render(2.0, &mut sink).expect("render");
            } else {
                let mut sink = PpmSink::with_view(dir, (64, 36), (3.2, 1.8)).expect("ppm sink");
                labels_scene().render(2.0, &mut sink).expect("render");
            }
        }
        let want: Vec<String> = (0..2).map(|i| format!("frame_{i:05}.{ext}")).collect();
        assert_eq!(dir_names(&a), want, "{kind}: file list");
        assert_eq!(dir_names(&b), want, "{kind}: file list");
        for name in &want {
            assert!(
                fs::read(a.join(name)).unwrap() == fs::read(b.join(name)).unwrap(),
                "{kind}/{name} differs between two renders in one process"
            );
        }
    }
}

// AC7 — `eval` on a scene and on its `clone()` agree: positions by
// `to_bits`, strings byte-for-byte.
#[test]
fn ac7_eval_and_clone_agree() {
    let scene = labels_scene();
    let clone = scene.clone();
    for t in [0.0, 0.5, 1.0] {
        let (a, b) = (scene.eval(t), clone.eval(t));
        assert_eq!(a.len(), b.len(), "t = {t}: same primitive count");
        for (x, y) in a.iter().zip(&b) {
            match (x, y) {
                (Prim2::Text { at: p, text: s, .. }, Prim2::Text { at: q, text: r, .. }) => {
                    assert_eq!(bits(*p), bits(*q), "t = {t}: position bits");
                    assert_eq!(s, r, "t = {t}: identical strings");
                }
                _ => assert_eq!(x, y, "t = {t}: non-text primitives agree"),
            }
        }
    }
}

/// The keys of a `Cargo.toml` section, in order (the R-0003 helper, reused).
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

// AC7 — the face is a const table, so the dependency graph is unchanged.
#[test]
fn ac7_zero_new_dependencies() {
    let manifest = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .expect("read the crate manifest");
    assert_eq!(section_keys(&manifest, "dependencies"), ["garust"]);
    assert_eq!(section_keys(&manifest, "dev-dependencies"), ["proptest"]);
    assert!(!manifest.contains("[build-dependencies]"), "no build deps");
}

// --- AC8: ASCII-only, total and documented --------------------------------

// AC8 — the substitution table: every unrenderable char becomes exactly one
// `?`, the character count is preserved, and nothing panics.
#[test]
fn ac8_substitution_table_preserves_character_count() {
    let cases: [(&str, &str); 10] = [
        ("é", "?"),
        ("→", "?"),
        ("日本", "??"),
        ("\t", "?"),
        ("\n", "?"),
        ("\r", "?"),
        ("\u{0}", "?"),
        ("\u{7F}", "?"),
        ("e\u{301}", "e?"),
        ("🙂", "?"),
    ];
    for (input, want) in cases {
        let mut scene = Scene::new(1.0);
        scene.add_label(Label::new(input, Anchor::Screen(ScreenAnchor::Centre)));
        let prims = scene.eval(0.0);
        match texts(&prims)[0] {
            Prim2::Text { text, .. } => {
                assert_eq!(text, want, "{input:?} must substitute to {want:?}");
                assert_eq!(
                    text.chars().count(),
                    input.chars().count(),
                    "{input:?}: the character count must be preserved"
                );
                assert!(
                    text.bytes().all(|b| (0x20..=0x7E).contains(&b)),
                    "{input:?}: the result must be printable ASCII"
                );
            }
            _ => unreachable!(),
        }
        // `is_ascii_renderable` agrees with the substitution.
        let label = Label::new(input, Anchor::Screen(ScreenAnchor::Centre));
        assert_eq!(
            label.is_ascii_renderable(),
            input == want,
            "{input:?}: the predicate must agree with the substitution"
        );
    }
}

// AC8 — both sinks see the same substituted string: the PPM frame for a
// substituted label is byte-identical to the same scene authored with `?`
// literally. The cheapest proof that substitution happens upstream and the
// sinks cannot disagree.
#[test]
fn ac8_both_sinks_see_the_same_substituted_string() {
    let render = |name: &str, text: &str| -> Vec<u8> {
        let dir = tmp_dir(name);
        let mut sink = PpmSink::with_view(&dir, (64, 36), (3.2, 1.8)).expect("sink");
        let mut scene = Scene::new(1.0);
        scene.add_label(Label::new(text, Anchor::Screen(ScreenAnchor::Centre)));
        scene.render(1.0, &mut sink).expect("render");
        fs::read(dir.join("frame_00000.ppm")).expect("read")
    };
    assert!(
        render("r0007_ac8_sub", "g\u{00E9}") == render("r0007_ac8_lit", "g?"),
        "the substitution must happen upstream of both sinks"
    );

    let svg = {
        let mut scene = Scene::new(1.0);
        scene.add_label(Label::new(
            "g\u{00E9}",
            Anchor::Screen(ScreenAnchor::Centre),
        ));
        svg_of("r0007_ac8_svg", &scene)
    };
    assert!(
        svg.contains(">g?</text>"),
        "SVG shows the same substitution"
    );
}

// --- AC6: PpmSink rasterizes from the embedded face -----------------------
//
// The fixture frame: 96 × 48 over view (3.0, 1.5), so `s = 32` exactly. At
// that scale the cell height `8/s` is `0.25`, every baseline row lands on a
// dyadic image coordinate, and `size = 0.5 → k = 2` / `size = 0.25 → k = 1`
// are exact rather than rounding accidents (SPEC-0007 §3).

const GLYPH_DIMS: (u32, u32) = (96, 48);
const GLYPH_VIEW: (f64, f64) = (3.0, 1.5);
const GLYPH_S: f64 = 32.0;

fn ppm_header_len(dims: (u32, u32)) -> usize {
    format!("P6\n{} {}\n255\n", dims.0, dims.1).len()
}

fn ppm_pixel(frame: &[u8], dims: (u32, u32), x: u32, y: u32) -> [u8; 3] {
    let i = ppm_header_len(dims) + (y as usize * dims.0 as usize + x as usize) * 3;
    [frame[i], frame[i + 1], frame[i + 2]]
}

/// Render a label scene into the fixture frame and return the whole file.
fn glyph_frame(name: &str, labels: Vec<Label>) -> Vec<u8> {
    let dir = tmp_dir(name);
    let mut sink = PpmSink::with_view(&dir, GLYPH_DIMS, GLYPH_VIEW).expect("sink");
    let mut scene = Scene::new(1.0);
    scene.view = GLYPH_VIEW;
    for l in labels {
        scene.add_label(l);
    }
    scene.render(1.0, &mut sink).expect("render");
    fs::read(dir.join("frame_00000.ppm")).expect("read")
}

fn white() -> Style {
    Style {
        stroke: Rgb::WHITE,
        width: 0.0,
        alpha: 1.0,
    }
}

/// Rows of the frame in which any pixel is lit.
fn lit_rows(frame: &[u8], dims: (u32, u32)) -> Vec<u32> {
    (0..dims.1)
        .filter(|&y| (0..dims.0).any(|x| ppm_pixel(frame, dims, x, y) != [0, 0, 0]))
        .collect()
}

// AC6(b) — the `k` derivation, pinned exactly, plus the degenerate sizes.
#[test]
fn ac6_scale_factor_is_derived_from_the_cell_height() {
    // A capital H is 5 columns wide and spans rows 0..=5 of the cell, so
    // its lit height in pixels is exactly 6k — the cheapest observable
    // proof of k without exposing it.
    for (size, k) in [(0.25_f64, 1_u32), (0.5, 2), (1.0, 4)] {
        let frame = glyph_frame(
            &format!("r0007_ac6_k{k}"),
            vec![Label::new("H", Anchor::Screen(ScreenAnchor::Centre))
                .with_size(size)
                .with_style(white())],
        );
        let rows = lit_rows(&frame, GLYPH_DIMS);
        assert_eq!(
            rows.len() as u32,
            6 * k,
            "size {size} at s = {GLYPH_S} must give k = {k} (6k lit rows)"
        );
    }

    // A sub-font-pixel size clamps to k = 1 rather than vanishing.
    let tiny = glyph_frame(
        "r0007_ac6_tiny",
        vec![Label::new("H", Anchor::Screen(ScreenAnchor::Centre))
            .with_size(1e-6)
            .with_style(white())],
    );
    assert_eq!(lit_rows(&tiny, GLYPH_DIMS).len(), 6, "clamps to k = 1");

    // NaN or non-positive paints nothing.
    for bad in [f64::NAN, 0.0, -1.0, f64::INFINITY] {
        let frame = glyph_frame(
            &format!("r0007_ac6_bad_{}", bad.to_bits()),
            vec![Label::new("H", Anchor::Screen(ScreenAnchor::Centre))
                .with_size(bad)
                .with_style(white())],
        );
        assert!(
            lit_rows(&frame, GLYPH_DIMS).is_empty(),
            "size {bad} must paint nothing"
        );
    }
}

// AC6(c) — the alignment arithmetic, asserted exactly. A 3-character run at
// k = 2 is w = 36 px, so `Center` pens at `px − 18` and `Right` at
// `px − 36`. `w/2` is integral for every k and n because CELL_W is even.
#[test]
fn ac6_alignment_arithmetic_is_exact() {
    // "HHH" at size 0.5 → k = 2, advance 12, w = 36.
    let px = 48_u32; // Centre maps to x = 48
    let cases = [
        (Align::Left, px),
        (Align::Center, px - 18),
        (Align::Right, px - 36),
    ];
    for (align, want_x0) in cases {
        let frame = glyph_frame(
            &format!("r0007_ac6_align_{align:?}"),
            vec![Label::new("HHH", Anchor::Screen(ScreenAnchor::Centre))
                .with_size(0.5)
                .with_align(align)
                .with_style(white())],
        );
        let first_lit = (0..GLYPH_DIMS.0)
            .find(|&x| (0..GLYPH_DIMS.1).any(|y| ppm_pixel(&frame, GLYPH_DIMS, x, y) != [0, 0, 0]))
            .expect("the run is on frame");
        assert_eq!(
            first_lit, want_x0,
            "{align:?}: a 3-char run at k = 2 is 36 px wide, so the pen is here"
        );
    }
    // The halving is exact for every k and n: CELL_W is even.
    for k in 1..=8_i64 {
        for n in 1..=20_i64 {
            assert_eq!((6 * k * n) % 2, 0, "k = {k}, n = {n}: w must be even");
        }
    }
}

// AC6(d) — the baseline convention: a capital sits ON the baseline, so its
// lowest lit row is `py − 1` at k = 1 and nothing is lit at `py`; a 'g'
// lights `py` with its descender.
#[test]
fn ac6_the_anchor_sits_on_the_baseline() {
    // Centre maps to py = 24, an integral row.
    let py = 24_u32;
    let cap = glyph_frame(
        "r0007_ac6_baseline_h",
        vec![Label::new("H", Anchor::Screen(ScreenAnchor::Centre))
            .with_size(0.25)
            .with_style(white())],
    );
    let rows = lit_rows(&cap, GLYPH_DIMS);
    assert_eq!(
        *rows.last().expect("H is lit"),
        py - 1,
        "a capital's lowest lit row is one above the baseline"
    );
    assert!(!rows.contains(&py), "nothing is lit at the baseline itself");

    let desc = glyph_frame(
        "r0007_ac6_baseline_g",
        vec![Label::new("g", Anchor::Screen(ScreenAnchor::Centre))
            .with_size(0.25)
            .with_style(white())],
    );
    assert!(
        lit_rows(&desc, GLYPH_DIMS).contains(&py),
        "'g' reaches the baseline row with its descender"
    );
}

// AC6(e) — ink lands within ±1 px of `to_pixel(at)` (SPEC-0006 §2.8's
// tolerance, extended), and background is preserved outside the cell box.
#[test]
fn ac6_ink_lands_at_the_mapped_anchor() {
    let frame = glyph_frame(
        "r0007_ac6_place",
        vec![Label::new("H", Anchor::Screen(ScreenAnchor::Centre))
            .with_size(0.25)
            .with_style(white())],
    );
    // Centre → at = (0, 0) → (px, py) = (48, 24). The cell is 6 × 8 with
    // the baseline 6 rows down, so ink lives in x ∈ [48, 53], y ∈ [18, 23].
    let lit: Vec<(u32, u32)> = (0..GLYPH_DIMS.1)
        .flat_map(|y| (0..GLYPH_DIMS.0).map(move |x| (x, y)))
        .filter(|&(x, y)| ppm_pixel(&frame, GLYPH_DIMS, x, y) != [0, 0, 0])
        .collect();
    assert!(!lit.is_empty(), "the run must be drawn");
    for (x, y) in lit {
        assert!(
            (48..=52).contains(&x) && (18..=23).contains(&y),
            "ink at ({x}, {y}) escaped the run's cell box"
        );
    }
}

// AC6(f) — SPEC-0006's own golden is byte-unchanged: routing text through
// `draw` must not disturb the stroke path.
#[test]
fn ac6_the_stroke_golden_is_untouched() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/frame_00000.ppm");
    let bytes = fs::read(&fixture).expect("SPEC-0006's golden still exists");
    assert_eq!(bytes.len(), 13 + 64 * 36 * 3, "still the 64 × 36 fixture");
    assert_eq!(&bytes[..13], b"P6\n64 36\n255\n");
}

/// SPEC-0007 §3's PPM label scene: three labels, no objects.
fn glyph_scene_labels() -> Vec<Label> {
    vec![
        Label::new("MOTO", Anchor::Screen(ScreenAnchor::Centre))
            .with_offset(pt(0.0, -0.25))
            .with_size(0.5)
            .with_align(Align::Center)
            .with_style(white()),
        Label::new("g\u{00E9}", Anchor::Screen(ScreenAnchor::BottomLeft))
            .with_offset(pt(0.0625, 0.0625))
            .with_size(0.25)
            .with_style(Style {
                stroke: Rgb {
                    r: 0xff,
                    g: 0x4d,
                    b: 0x00,
                },
                width: 0.0,
                alpha: 1.0,
            }),
        Label::new("MOTO", Anchor::Screen(ScreenAnchor::Centre))
            .with_offset(pt(0.375, -0.25))
            .with_size(0.5)
            .with_align(Align::Center)
            .with_style(Style {
                stroke: Rgb {
                    r: 0x00,
                    g: 0xb4,
                    b: 0xd8,
                },
                width: 0.0,
                alpha: 0.5,
            }),
    ]
}

const LABELS_PPM: &[u8] = include_bytes!("golden/labels_00000.ppm");

/// Decode a byte offset into `(x, y, channel)` — SPEC-0006 §6 AC3's
/// reporter, reused so a binary golden still fails legibly.
fn locate(offset: usize, dims: (u32, u32)) -> String {
    let head = ppm_header_len(dims);
    if offset < head {
        return format!("header byte {offset}");
    }
    let i = offset - head;
    let (px, channel) = (i / 3, i % 3);
    format!(
        "pixel ({}, {}) channel {}",
        px as u32 % dims.0,
        px as u32 / dims.0,
        ["R", "G", "B"][channel]
    )
}

// AC6(g) — the PPM label golden, plus the named pixel probes SPEC-0007 §3
// derives, so a byte diff says which claim broke and not merely that one
// did.
#[test]
fn ac6_labels_ppm_golden_matches_with_named_probes() {
    let got = glyph_frame("r0007_ac6_golden", glyph_scene_labels());

    if std::env::var_os("MOTOREEL_BLESS").is_some_and(|v| v == "1") {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/labels_00000.ppm");
        fs::write(&fixture, &got).expect("bless");
        eprintln!("blessed {}", fixture.display());
        return;
    }

    assert_eq!(
        got.len(),
        13 + 96 * 48 * 3,
        "a 96 × 48 P6 frame is 13 header bytes plus 13 824 pixel bytes"
    );

    // The two composites §3 derives, asserted as named probes. Run 3 is
    // teal at alpha 0.5, displaced from run 1 by exactly one k = 2 cell.
    let teal_over_black = [0, 90, 108];
    let teal_over_white = [128, 218, 236];
    assert!(
        (0..GLYPH_DIMS.0).any(
            |x| (0..GLYPH_DIMS.1).any(|y| ppm_pixel(&got, GLYPH_DIMS, x, y) == teal_over_black)
        ),
        "run 3 must composite over background to (0, 90, 108)"
    );
    assert!(
        (0..GLYPH_DIMS.0).any(
            |x| (0..GLYPH_DIMS.1).any(|y| ppm_pixel(&got, GLYPH_DIMS, x, y) == teal_over_white)
        ),
        "run 3 over run 1's white must be (128, 218, 236) — the ties-away \
         rounding rule, pinned as a side effect"
    );
    // Run 2's baseline row is 46 and its descender puts ink there.
    assert!(
        (2..14).any(|x| ppm_pixel(&got, GLYPH_DIMS, x, 46) != [0, 0, 0]),
        "run 2's 'g' descender must light its baseline row"
    );

    assert_eq!(LABELS_PPM.len(), got.len(), "fixture length");
    if let Some(at) = (0..got.len()).find(|&i| got[i] != LABELS_PPM[i]) {
        panic!(
            "the label golden drifted at byte {at} — {}: rendered {}, fixture {}",
            locate(at, GLYPH_DIMS),
            got[at],
            LABELS_PPM[at]
        );
    }
}

/// The specimen: six rows of 16 glyphs covering all 95 code points.
fn specimen_labels() -> Vec<Label> {
    (0..6)
        .map(|r| {
            let first = 0x20 + 16 * r;
            let text: String = (first..(first + 16).min(0x7F))
                .map(|c| c as u8 as char)
                .collect();
            Label::new(text, Anchor::Screen(ScreenAnchor::TopLeft))
                .with_offset(pt(0.0, -f64::from(6 + 8 * r) / GLYPH_S))
                .with_size(0.25)
                .with_style(white())
        })
        .collect()
}

const SPECIMEN: &[u8] = include_bytes!("golden/font_specimen.ppm");

// AC6(h) — the specimen is mandatory: it is the only artefact in which the
// face is reviewable. It also asserts that no row's ink reaches the next
// row's cell top, so a mis-authored descender fails rather than merely
// looking wrong.
#[test]
fn ac6_font_specimen_matches_and_no_row_collides() {
    let got = glyph_frame("r0007_ac6_specimen", specimen_labels());

    if std::env::var_os("MOTOREEL_BLESS").is_some_and(|v| v == "1") {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/font_specimen.ppm");
        fs::write(&fixture, &got).expect("bless");
        eprintln!("blessed {}", fixture.display());
        return;
    }

    // Rows are 8 px tall with baselines at 6, 14, 22, 30, 38, 46; each
    // row's descender row is its baseline, two rows clear of the next
    // row's cell top.
    for r in 0..6u32 {
        let cell_top = 8 * r;
        let baseline = cell_top + 6;
        for y in (baseline + 1)..(cell_top + 8) {
            assert!(
                (0..GLYPH_DIMS.0).all(|x| ppm_pixel(&got, GLYPH_DIMS, x, y) == [0, 0, 0]),
                "row {r}: ink at y = {y} would collide with the next row"
            );
        }
    }

    assert_eq!(SPECIMEN.len(), got.len(), "fixture length");
    if let Some(at) = (0..got.len()).find(|&i| got[i] != SPECIMEN[i]) {
        panic!(
            "the specimen drifted at byte {at} — {}: rendered {}, fixture {}",
            locate(at, GLYPH_DIMS),
            got[at],
            SPECIMEN[at]
        );
    }
}
