//! R-0002 — Scene, objects, motor-posed camera, projection: the QA-owned
//! acceptance suite (e2e).
//!
//! Loop step 3, TDD red: authored before the implementation exists, derived
//! from the acceptance criteria of `requirements/0002-scene-camera.md`
//! (AC1–AC6), the R-0002 §4 cull constraint, and SPEC-0002 §6's test plan.
//! Each test's header comment names the criteria it verifies.
//!
//! Criterion → test map:
//!
//! - AC1  `ac1_golden_frame_evaluates_exactly` (also witnesses AC4 and §4)
//! - AC2  `ac2_moving_camera_by_m_matches_moving_world_by_m_inverse`
//! - AC3  `ac3_projection_models_hand_computed_and_culling`,
//!   `ac3_both_models_available_on_one_camera_type`
//! - AC4  `ac4_style_passthrough_is_bit_exact_under_both_projections`
//!   (plus riders in AC1 and AC2)
//! - AC5  `ac5_single_key_object_renders_at_its_fixed_pose_for_all_t`,
//!   `ac5_two_key_object_exact_at_keys_interpolated_between`
//! - AC6  `ac6_same_scene_same_t_is_bit_identical` (plus the AC2 rider)
//! - §4 cull / never-NaN  `cull_one_bad_vertex_drops_the_whole_primitive`,
//!   `cull_survivors_keep_insertion_draw_order`,
//!   `cull_applies_uniformly_to_orthographic`,
//!   `never_nonfinite_even_for_garbage_focal`,
//!   `never_nonfinite_under_hostile_scenes`
//! - SPEC-0002 pinned decisions  `spec_defaults_are_the_adjudicated_ones`
//!   (§2.3/§2.4), `spec_degenerate_polylines_pass_through_unvalidated`
//!   (§2.3), `spec_scene_is_plain_data_and_eval_ignores_duration` (§2.6)
//!
//! Exactness levels are calibrated to the R-0001 ground-truth probes and
//! SPEC-0002 §6: the AC1 golden uses only dyadic inputs with power-of-two
//! view depths and pins its animated object to a key time, so the whole
//! translator/compose/sandwich/divide path is rounding-free and `assert_eq!`
//! is legitimate; slerp-bearing mid-span values assert at 1e-12; the AC2
//! duality compares at 1e-6 because the two sides compose on different ends
//! and round differently. AC2's generator keeps every view depth O(1)-far
//! from `Projection::NEAR` (the spec's own generator guarantee), so cull
//! decisions can never straddle the threshold between the two sides.

use garust::{pga, Motor3, Pga3};
use motoreel::{Camera, Object, ObjectId, Prim2, Projection, Pt2, Rgb, Scene, Shape, Style, Track};
use proptest::prelude::*;
use std::f64::consts::TAU;

// --- Shared helpers ------------------------------------------------------

/// One of the three Euclidean basis bivectors of PGA — a valid rotation
/// plane (squares to −1): `e1e2`, `e1e3`, `e2e3`.
fn pga_axis(choice: usize) -> Pga3 {
    Pga3::basis([0b0011, 0b0101, 0b0110][choice % 3])
}

/// An ideal point (a direction): the difference of two unit-weight points
/// has weight 0, so its `to_euclidean` weight-divide is non-finite — the
/// §2.5 guard's job to cull.
fn ideal_point(x: f64, y: f64, z: f64) -> pga::Point {
    pga::Point::from_multivector(Pga3::point(x, y, z) - Pga3::point(0.0, 0.0, 0.0))
}

/// A primitive flattened to (variant tag, vertices, style).
fn prim_parts(prim: &Prim2) -> (u8, Vec<Pt2>, Style) {
    match prim {
        Prim2::Point { at, style } => (0, vec![*at], *style),
        Prim2::Segment { a, b, style } => (1, vec![*a, *b], *style),
        Prim2::Polyline { points, style } => (2, points.clone(), *style),
        // R-0004 amended the vocabulary; flattened here only to keep this
        // helper exhaustive. `Edges` behaviour is R-0004's to verify.
        Prim2::Edges { segments, style } => (
            3,
            segments.iter().flat_map(|(a, b)| [*a, *b]).collect(),
            *style,
        ),
    }
}

type StyleBits = (u8, u8, u8, u64, u64);
type PrimBits = (u8, Vec<(u64, u64)>, StyleBits);

/// A style's exact content — u8 channels plus f64 bit patterns (AC4's
/// "unchanged" is bit-level, stricter than `==`).
fn style_bits(style: Style) -> StyleBits {
    (
        style.stroke.r,
        style.stroke.g,
        style.stroke.b,
        style.width.to_bits(),
        style.alpha.to_bits(),
    )
}

/// A primitive list's exact content, every f64 as bits — AC6's determinism
/// currency (stricter than `==`, which admits ±0 crossings).
fn prim_bits(prims: &[Prim2]) -> Vec<PrimBits> {
    prims
        .iter()
        .map(|prim| {
            let (tag, points, style) = prim_parts(prim);
            (
                tag,
                points
                    .iter()
                    .map(|p| (p.x.to_bits(), p.y.to_bits()))
                    .collect(),
                style_bits(style),
            )
        })
        .collect()
}

/// Every f64 an emitted primitive carries (coordinates and style scalars) —
/// the §2.2 never-non-finite invariant's audit surface.
fn every_f64(prims: &[Prim2]) -> Vec<f64> {
    let mut out = Vec::new();
    for prim in prims {
        let (_, points, style) = prim_parts(prim);
        for p in points {
            out.push(p.x);
            out.push(p.y);
        }
        out.push(style.width);
        out.push(style.alpha);
    }
    out
}

// --- The golden scene (AC1, AC6) -----------------------------------------

/// Three distinct all-dyadic styles for the golden frame.
fn golden_styles() -> [Style; 3] {
    [
        Style {
            stroke: Rgb {
                r: 255,
                g: 64,
                b: 0,
            },
            width: 0.5,
            alpha: 1.0,
        },
        Style {
            stroke: Rgb {
                r: 0,
                g: 255,
                b: 64,
            },
            width: 0.25,
            alpha: 0.5,
        },
        Style {
            stroke: Rgb {
                r: 64,
                g: 0,
                b: 255,
            },
            width: 2.0,
            alpha: 0.75,
        },
    ]
}

/// SPEC-0002 §6 AC1's scene: pinhole camera at `translator(0,0,4)`, focal
/// 2. Every input is dyadic, every surviving view depth a power of two,
/// and the animated object is evaluated at its last key time — so the
/// whole evaluation is rounding-free and the golden compare is exact.
fn golden_scene() -> Scene {
    let [s1, s2, s3] = golden_styles();
    let mut scene = Scene::new(8.0);
    scene.camera = Camera::pinhole(Motor3::translator(0.0, 0.0, 4.0), 2.0);
    // 1. Static segment, identity pose: view depth 4.
    scene.add(
        Object::segment(
            pga::Point::new(1.0, 0.0, 0.0),
            pga::Point::new(0.0, 1.0, 0.0),
        )
        .with_style(s1),
    );
    // 2. Unit square held at translator(1,0,0): view depth 4.
    scene.add(
        Object::polyline(vec![
            pga::Point::new(0.0, 0.0, 0.0),
            pga::Point::new(1.0, 0.0, 0.0),
            pga::Point::new(1.0, 1.0, 0.0),
            pga::Point::new(0.0, 1.0, 0.0),
        ])
        .with_style(s2)
        .at(Motor3::translator(1.0, 0.0, 0.0)),
    );
    // 3. Animated point: keys (0, identity), (4, translator(0,0,2)) — at
    //    t = 4 it sits at (0,0,2), view depth 2 (exact by R-0001 AC1/AC4,
    //    no slerp bits on the path).
    scene.add(
        Object::point(pga::Point::new(0.0, 0.0, 0.0))
            .with_style(s3)
            .with_track(
                Track::keys([
                    (0.0, Motor3::identity()),
                    (4.0, Motor3::translator(0.0, 0.0, 2.0)),
                ])
                .expect("golden track keys are valid"),
            ),
    );
    // 4. Behind the camera (view depth −2): culled.
    scene.add(Object::point(pga::Point::new(0.0, 0.0, 6.0)));
    // 5. Exactly on the camera plane (view depth 0): culled.
    scene.add(Object::point(pga::Point::new(0.0, 0.0, 4.0)));
    scene
}

// AC1 (+ AC4, §4 cull) — one frame of the known scene at t = 4 equals the
// hand-constructed primitive list exactly: `assert_eq!`, no tolerance. The
// behind-camera and at-plane objects must be absent, the styles verbatim.
#[test]
fn ac1_golden_frame_evaluates_exactly() {
    let [s1, s2, s3] = golden_styles();
    let want = vec![
        Prim2::Segment {
            a: Pt2 { x: 0.5, y: 0.0 },
            b: Pt2 { x: 0.0, y: 0.5 },
            style: s1,
        },
        Prim2::Polyline {
            points: vec![
                Pt2 { x: 0.5, y: 0.0 },
                Pt2 { x: 1.0, y: 0.0 },
                Pt2 { x: 1.0, y: 0.5 },
                Pt2 { x: 0.5, y: 0.5 },
            ],
            style: s2,
        },
        Prim2::Point {
            at: Pt2 { x: 0.0, y: 0.0 },
            style: s3,
        },
    ];
    assert_eq!(golden_scene().eval(4.0), want);
}

// --- AC3: projection models ----------------------------------------------

// AC3 — pinhole divides by view depth with the configured focal length,
// orthographic omits the divide; behind, at-plane, below-NEAR, and ideal
// points cull to `None` under both models; depth exactly NEAR passes
// (the rule is `d >= NEAR`).
#[test]
fn ac3_projection_models_hand_computed_and_culling() {
    assert_eq!(Projection::NEAR, 1e-9);

    let pin = Projection::Pinhole { focal: 2.0 };
    let ortho = Projection::Orthographic;

    // View-space point (1, 2, −4): depth d = 4.
    let front = pga::Point::new(1.0, 2.0, -4.0);
    assert_eq!(pin.project(&front), Some(Pt2 { x: 0.5, y: 1.0 }));
    assert_eq!(ortho.project(&front), Some(Pt2 { x: 1.0, y: 2.0 }));

    let culled = [
        ("behind", pga::Point::new(0.0, 0.0, 1.0)),
        ("at-plane", pga::Point::new(0.0, 0.0, 0.0)),
        ("below-NEAR", pga::Point::new(0.0, 0.0, -1.0e-12)),
        ("ideal", ideal_point(2.0, -3.0, 5.0)),
    ];
    for (name, p) in culled {
        assert_eq!(pin.project(&p), None, "pinhole must cull the {name} point");
        assert_eq!(
            ortho.project(&p),
            None,
            "orthographic must cull the {name} point"
        );
    }

    // The boundary is inclusive: depth exactly NEAR is in front.
    let on_near = pga::Point::new(0.0, 0.0, -1.0e-9);
    assert_eq!(pin.project(&on_near), Some(Pt2 { x: 0.0, y: 0.0 }));
    assert_eq!(ortho.project(&on_near), Some(Pt2 { x: 0.0, y: 0.0 }));
}

// AC3 — both models live on the one `Camera` type via its two
// constructors, and a full `Scene::eval` reproduces the hand-computed
// image under each: same geometry, divide vs no divide.
#[test]
fn ac3_both_models_available_on_one_camera_type() {
    let pose = Motor3::translator(0.0, 0.0, 4.0);
    assert_eq!(
        Camera::pinhole(pose, 2.0),
        Camera {
            pose,
            projection: Projection::Pinhole { focal: 2.0 },
        }
    );
    assert_eq!(
        Camera::orthographic(pose),
        Camera {
            pose,
            projection: Projection::Orthographic,
        }
    );

    // World point (1, 2, 0) → view (1, 2, −4), depth 4.
    let subject = || {
        let mut scene = Scene::new(1.0);
        scene.add(Object::point(pga::Point::new(1.0, 2.0, 0.0)));
        scene
    };
    let mut pin_scene = subject();
    pin_scene.camera = Camera::pinhole(pose, 2.0);
    assert_eq!(
        pin_scene.eval(0.0),
        vec![Prim2::Point {
            at: Pt2 { x: 0.5, y: 1.0 },
            style: Style::default(),
        }],
        "pinhole: focal · x / d = 2·1/4"
    );
    let mut ortho_scene = subject();
    ortho_scene.camera = Camera::orthographic(pose);
    assert_eq!(
        ortho_scene.eval(0.0),
        vec![Prim2::Point {
            at: Pt2 { x: 1.0, y: 2.0 },
            style: Style::default(),
        }],
        "orthographic: image = view (x, y), no divide"
    );
}

// --- AC4: style passthrough ----------------------------------------------

// AC4 — distinct styles (u8 channels, width 3.5, alpha 0.25) reach the
// emitted primitives bit-for-bit unchanged, under both projections, for
// all three shape kinds.
#[test]
fn ac4_style_passthrough_is_bit_exact_under_both_projections() {
    let sa = Style {
        stroke: Rgb {
            r: 7,
            g: 200,
            b: 33,
        },
        width: 3.5,
        alpha: 0.25,
    };
    let sb = Style {
        stroke: Rgb { r: 255, g: 0, b: 1 },
        width: 0.125,
        alpha: 1.0,
    };
    let sc = Style {
        stroke: Rgb { r: 0, g: 0, b: 0 },
        width: 0.0,
        alpha: 0.5,
    };
    let cameras = [
        Camera::pinhole(Motor3::translator(0.0, 0.0, 4.0), 2.0),
        Camera::orthographic(Motor3::translator(0.0, 0.0, 4.0)),
    ];
    for camera in cameras {
        let mut scene = Scene::new(1.0);
        scene.camera = camera;
        scene.add(Object::point(pga::Point::new(0.5, -0.5, 0.0)).with_style(sa));
        scene.add(
            Object::segment(
                pga::Point::new(0.0, 0.0, 0.0),
                pga::Point::new(1.0, 1.0, 1.0),
            )
            .with_style(sb),
        );
        scene.add(
            Object::polyline(vec![
                pga::Point::new(-1.0, 0.0, 0.0),
                pga::Point::new(0.0, 1.0, 0.0),
                pga::Point::new(1.0, 0.0, -1.0),
            ])
            .with_style(sc),
        );
        let prims = scene.eval(0.0);
        assert_eq!(prims.len(), 3, "all three objects face {camera:?}");
        let got: Vec<StyleBits> = prims.iter().map(|p| style_bits(prim_parts(p).2)).collect();
        assert_eq!(
            got,
            vec![style_bits(sa), style_bits(sb), style_bits(sc)],
            "styles must pass through unchanged under {camera:?}"
        );
    }
}

// --- AC5: objects evaluate their tracks at scene time --------------------

// AC5 — a single-key (static) object renders at its fixed pose: the same
// bit-identical primitive list at every t (R-0001 AC4 clamp semantics),
// and that list is the hand-computed image of the posed geometry.
#[test]
fn ac5_single_key_object_renders_at_its_fixed_pose_for_all_t() {
    let pose = Motor3::translator(0.5, 0.25, 1.0);
    let mut scene = Scene::new(2.0);
    scene.camera = Camera::pinhole(Motor3::translator(0.0, 0.0, 5.0), 2.0);
    scene.add(
        Object::segment(
            pga::Point::new(0.0, 0.0, 0.0),
            pga::Point::new(1.0, 0.0, 0.0),
        )
        .at(pose),
    );
    // World segment (0.5, 0.25, 1)–(1.5, 0.25, 1): view depth 4 (dyadic).
    let want = vec![Prim2::Segment {
        a: Pt2 { x: 0.25, y: 0.125 },
        b: Pt2 { x: 0.75, y: 0.125 },
        style: Style::default(),
    }];
    let reference = scene.eval(0.0);
    assert_eq!(reference, want, "the fixed pose's hand-computed image");
    let reference = prim_bits(&reference);
    for t in [-1.0, 0.0, 0.5, 7.0] {
        assert_eq!(
            prim_bits(&scene.eval(t)),
            reference,
            "a static object must render bit-identically at t = {t}"
        );
    }
}

// AC5 — a two-key object is exact at both key times (dyadic discipline:
// no slerp bits on the path) and lands on the quarter translation at
// t = 1 of the (0, id) → (4, translator(0,0,2)) track, within 1e-12 —
// tolerance decouples this spec from slerp bit-behavior.
#[test]
fn ac5_two_key_object_exact_at_keys_interpolated_between() {
    let style = golden_styles()[2];
    let mut scene = Scene::new(4.0);
    scene.camera = Camera::pinhole(Motor3::translator(0.0, 0.0, 4.0), 2.0);
    scene.add(
        Object::point(pga::Point::new(1.0, 1.0, 0.0))
            .with_style(style)
            .with_track(
                Track::keys([
                    (0.0, Motor3::identity()),
                    (4.0, Motor3::translator(0.0, 0.0, 2.0)),
                ])
                .expect("two-key track is valid"),
            ),
    );
    // t = 0: world (1,1,0), depth 4 → (0.5, 0.5). Exact.
    assert_eq!(
        scene.eval(0.0),
        vec![Prim2::Point {
            at: Pt2 { x: 0.5, y: 0.5 },
            style,
        }]
    );
    // t = 4: world (1,1,2), depth 2 → (1, 1). Exact.
    assert_eq!(
        scene.eval(4.0),
        vec![Prim2::Point {
            at: Pt2 { x: 1.0, y: 1.0 },
            style,
        }]
    );
    // t = 1 (u = 0.25): pose ≈ translator(0,0,0.5), world (1,1,0.5),
    // depth 3.5 → focal·x/d = 2/3.5 on both axes.
    let prims = scene.eval(1.0);
    assert_eq!(prims.len(), 1, "the point stays visible mid-span");
    match &prims[0] {
        Prim2::Point { at, style: got } => {
            let want = 4.0 / 7.0;
            assert!(
                (at.x - want).abs() < 1e-12 && (at.y - want).abs() < 1e-12,
                "quarter translation must image at ({want}, {want}), got ({}, {})",
                at.x,
                at.y
            );
            assert_eq!(style_bits(*got), style_bits(style));
        }
        other => panic!("expected a point at t = 1, got {other:?}"),
    }
}

// --- AC6: determinism -----------------------------------------------------

// AC6 — same scene, same t → the same primitive list down to every f64's
// bit pattern: across re-evaluation, across a clone, at a key time and at
// a slerp-bearing interior time.
#[test]
fn ac6_same_scene_same_t_is_bit_identical() {
    let scene = golden_scene();
    let first = scene.eval(4.0);
    assert_eq!(
        prim_bits(&scene.eval(4.0)),
        prim_bits(&first),
        "re-evaluation must be bit-identical"
    );
    assert_eq!(
        prim_bits(&scene.clone().eval(4.0)),
        prim_bits(&first),
        "a cloned scene must evaluate bit-identically"
    );
    let t = 1.372;
    assert_eq!(
        prim_bits(&scene.eval(t)),
        prim_bits(&scene.eval(t)),
        "bit-identical at a slerp-bearing interior time too"
    );
}

// --- R-0002 §4: the cull policy (SPEC-0002 §2.5) -------------------------

// §4 cull — whole-primitive drop: one culled vertex removes the entire
// primitive; there is no clipping, no partial polyline, no substituted
// vertex. The all-visible control keeps every vertex.
#[test]
fn cull_one_bad_vertex_drops_the_whole_primitive() {
    let camera = Camera::pinhole(Motor3::translator(0.0, 0.0, 4.0), 2.0);
    let visible = [
        pga::Point::new(0.0, 0.0, 0.0),
        pga::Point::new(1.0, 0.0, 0.0),
        pga::Point::new(1.0, 1.0, 0.0),
    ];

    let mut control = Scene::new(1.0);
    control.camera = camera;
    control.add(Object::polyline(visible.to_vec()));
    assert_eq!(
        control.eval(0.0),
        vec![Prim2::Polyline {
            points: vec![
                Pt2 { x: 0.0, y: 0.0 },
                Pt2 { x: 0.5, y: 0.0 },
                Pt2 { x: 0.5, y: 0.5 },
            ],
            style: Style::default(),
        }],
        "the all-visible control survives with every vertex"
    );

    let bad_vertices = [
        pga::Point::new(0.0, 0.0, 6.0), // behind the camera (depth −2)
        pga::Point::new(0.0, 0.0, 4.0), // exactly on the camera plane
    ];
    for bad in bad_vertices {
        let mut scene = Scene::new(1.0);
        scene.camera = camera;
        let mut points = visible.to_vec();
        points.push(bad);
        scene.add(Object::polyline(points));
        assert_eq!(
            scene.eval(0.0),
            Vec::new(),
            "a polyline with one culled vertex must vanish entirely"
        );

        let mut scene = Scene::new(1.0);
        scene.camera = camera;
        scene.add(Object::segment(pga::Point::new(0.0, 0.0, 0.0), bad));
        assert_eq!(
            scene.eval(0.0),
            Vec::new(),
            "a segment with one culled endpoint must vanish entirely"
        );
    }
}

// §4 cull (+ AC1 draw order) — culled primitives are simply absent, and
// the survivors keep insertion order.
#[test]
fn cull_survivors_keep_insertion_draw_order() {
    let [s1, _, s3] = golden_styles();
    let mut scene = Scene::new(1.0);
    scene.camera = Camera::pinhole(Motor3::translator(0.0, 0.0, 4.0), 2.0);
    scene.add(Object::point(pga::Point::new(0.0, 0.0, 0.0)).with_style(s1));
    scene.add(Object::point(pga::Point::new(0.0, 0.0, 6.0))); // culled
    scene.add(Object::point(pga::Point::new(1.0, 0.0, 0.0)).with_style(s3));
    assert_eq!(
        scene.eval(0.0),
        vec![
            Prim2::Point {
                at: Pt2 { x: 0.0, y: 0.0 },
                style: s1,
            },
            Prim2::Point {
                at: Pt2 { x: 0.5, y: 0.0 },
                style: s3,
            },
        ],
        "absent, not substituted; survivors in insertion order"
    );
}

// §4 cull — the rule is uniform across models (spec decision 2026-08-20):
// an orthographic view culls behind-camera and at-plane vertices too.
#[test]
fn cull_applies_uniformly_to_orthographic() {
    let mut scene = Scene::new(1.0);
    scene.camera = Camera::orthographic(Motor3::translator(0.0, 0.0, 5.0));
    scene.add(Object::point(pga::Point::new(0.25, 0.5, 0.0))); // depth 5
    scene.add(Object::point(pga::Point::new(0.0, 0.0, 6.0))); // depth −1
    scene.add(Object::point(pga::Point::new(0.0, 0.0, 5.0))); // depth 0
    assert_eq!(
        scene.eval(0.0),
        vec![Prim2::Point {
            at: Pt2 { x: 0.25, y: 0.5 },
            style: Style::default(),
        }],
        "orthographic views cull behind/at-plane vertices too"
    );
}

// §4 never-NaN — garbage focal values (out of the documented contract)
// still cannot leak a non-finite number: a non-finite projected
// coordinate culls, so the primitive is absent — never NaN output.
#[test]
fn never_nonfinite_even_for_garbage_focal() {
    let front = pga::Point::new(1.0, 2.0, -4.0);
    let on_axis = pga::Point::new(0.0, 0.0, -4.0);
    for focal in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let pin = Projection::Pinhole { focal };
        assert_eq!(pin.project(&front), None, "focal {focal} must cull");
        assert_eq!(
            pin.project(&on_axis),
            None,
            "focal {focal} must cull on the optical axis"
        );
    }
    // Finite out-of-contract focal is GIGO: whatever comes out is finite.
    for focal in [0.0, -2.0, 1.0e300] {
        if let Some(p) = (Projection::Pinhole { focal }).project(&front) {
            assert!(
                p.x.is_finite() && p.y.is_finite(),
                "focal {focal} leaked a non-finite coordinate: {p:?}"
            );
        }
    }
    // Scene-level: absence, not NaN, reaches the sink.
    let mut scene = Scene::new(1.0);
    scene.camera = Camera::pinhole(Motor3::translator(0.0, 0.0, 4.0), f64::NAN);
    scene.add(Object::point(pga::Point::new(0.0, 0.0, 0.0)));
    assert_eq!(
        scene.eval(0.0),
        Vec::new(),
        "a NaN focal must cull the primitive, not emit NaN"
    );
}

// --- SPEC-0002 pinned decisions ------------------------------------------

// SPEC-0002 §2.3/§2.4 (supports AC3–AC5) — the adjudicated defaults:
// `Style::default()` white / width 0.01 / alpha 1, `Camera::default()`
// orthographic at translator(0,0,5), and `Object` constructors giving the
// default style with an identity hold.
#[test]
fn spec_defaults_are_the_adjudicated_ones() {
    assert_eq!(
        Rgb::WHITE,
        Rgb {
            r: 255,
            g: 255,
            b: 255
        }
    );
    assert_eq!(Rgb::BLACK, Rgb { r: 0, g: 0, b: 0 });
    let style = Style::default();
    assert_eq!(
        style,
        Style {
            stroke: Rgb::WHITE,
            width: 0.01,
            alpha: 1.0,
        }
    );
    assert_eq!(
        (style.width.to_bits(), style.alpha.to_bits()),
        (0.01f64.to_bits(), 1.0f64.to_bits())
    );

    assert_eq!(
        Camera::default(),
        Camera::orthographic(Motor3::translator(0.0, 0.0, 5.0))
    );
    assert_eq!(Camera::default().projection, Projection::Orthographic);

    // Constructor defaults, observed end to end: under `Scene::new`'s
    // default camera a constructor-built point renders at its own (x, y)
    // in the default style at any t (identity hold).
    let object = Object::point(pga::Point::new(0.25, -0.5, 0.0));
    assert_eq!(object.style, Style::default());
    let mut scene = Scene::new(1.0);
    scene.add(object);
    for t in [0.0, 137.0] {
        assert_eq!(
            scene.eval(t),
            vec![Prim2::Point {
                at: Pt2 { x: 0.25, y: -0.5 },
                style: Style::default(),
            }],
            "identity hold under the default camera at t = {t}"
        );
    }
}

// SPEC-0002 §2.3 (AC1 evaluation semantics edge) — polylines with fewer
// than two points pass through unvalidated: the pipeline is total, and
// rendering nothing (or a dot) is the sink's call.
#[test]
fn spec_degenerate_polylines_pass_through_unvalidated() {
    let mut scene = Scene::new(1.0);
    scene.camera = Camera::pinhole(Motor3::translator(0.0, 0.0, 4.0), 2.0);
    scene.add(Object::polyline(Vec::new()));
    scene.add(Object::polyline(vec![pga::Point::new(1.0, 1.0, 0.0)]));
    assert_eq!(
        scene.eval(0.0),
        vec![
            Prim2::Polyline {
                points: vec![],
                style: Style::default(),
            },
            Prim2::Polyline {
                points: vec![Pt2 { x: 0.5, y: 0.5 }],
                style: Style::default(),
            },
        ],
        "degenerate polylines are emitted as-is"
    );
}

// SPEC-0002 §2.6 (supports AC1/AC6) — `Scene` is plain public data:
// `new` starts empty with the default camera; `duration` is stored
// verbatim, unvalidated, and never consumed by `eval` (it is SPEC-0003's
// input); `add` returns distinct stable ids.
#[test]
fn spec_scene_is_plain_data_and_eval_ignores_duration() {
    let scene = Scene::new(6.5);
    assert!(scene.objects.is_empty(), "a new scene has no objects");
    assert_eq!(scene.camera, Camera::default());
    assert_eq!(scene.duration.to_bits(), 6.5f64.to_bits());
    assert_eq!(scene.eval(0.0), Vec::new());

    let build = |duration: f64| {
        let mut scene = Scene::new(duration);
        scene.camera = Camera::pinhole(Motor3::translator(0.0, 0.0, 4.0), 2.0);
        scene.add(Object::segment(
            pga::Point::new(0.0, 0.0, 0.0),
            pga::Point::new(1.0, 0.0, 0.0),
        ));
        scene
    };
    let reference = prim_bits(&build(1.0).eval(0.5));
    for duration in [1.0e6, 0.0, -3.0, f64::NAN] {
        let scene = build(duration);
        if duration.is_nan() {
            assert!(scene.duration.is_nan(), "duration stored verbatim");
        } else {
            assert_eq!(scene.duration.to_bits(), duration.to_bits());
        }
        assert_eq!(
            prim_bits(&scene.eval(0.5)),
            reference,
            "duration {duration} must not affect eval"
        );
    }

    let mut scene = Scene::new(1.0);
    let first: ObjectId = scene.add(Object::point(pga::Point::new(0.0, 0.0, 0.0)));
    let second = scene.add(Object::point(pga::Point::new(1.0, 0.0, 0.0)));
    let first_copy = first; // ObjectId is Copy
    assert_eq!(first, first_copy);
    assert_ne!(first, second, "insertion ids are distinct");
    assert_eq!(scene.objects.len(), 2);
    assert_eq!(
        scene.objects[1].shape,
        Shape::Point(pga::Point::new(1.0, 0.0, 0.0)),
        "objects are public data in insertion order"
    );
    scene.duration = 9.0; // public plain data stays writable
    assert_eq!(scene.duration.to_bits(), 9.0f64.to_bits());
}

// --- Property generators (AC2, §4 invariant) ------------------------------

prop_compose! {
    /// SPEC-0002 §6's motor generator: `translator ∘ rotor(θ, basis
    /// plane)`. `reach` bounds each translation component and `turn` the
    /// angle. The duality tests pass reach 0.8 (not the spec's prose ±2):
    /// with the camera fixed 8 back, that keeps every generated view depth
    /// ≥ ~0.9 — the spec's governing generator guarantee ("depths
    /// O(1)-far from NEAR, image coordinates O(10)"), which a ±2 box on
    /// both M and the object poses would break by letting geometry reach
    /// the camera plane, where the pinhole divide amplifies rounding past
    /// any fixed tolerance.
    fn arb_motor(reach: f64, turn: f64)(
        dx in -reach..reach,
        dy in -reach..reach,
        dz in -reach..reach,
        angle in -turn..turn,
        axis in 0usize..3,
    ) -> Motor3 {
        Motor3::translator(dx, dy, dz) * Motor3::rotor(angle, pga_axis(axis))
    }
}

prop_compose! {
    /// A model-space vertex in the spec's [−2, 2]³ box.
    fn arb_vertex()(
        x in -2.0f64..2.0,
        y in -2.0f64..2.0,
        z in -2.0f64..2.0,
    ) -> pga::Point {
        pga::Point::new(x, y, z)
    }
}

prop_compose! {
    /// A contract-respecting style with arbitrary channels.
    fn arb_style()(
        r in any::<u8>(),
        g in any::<u8>(),
        b in any::<u8>(),
        width in 0.0f64..4.0,
        alpha in 0.0f64..=1.0,
    ) -> Style {
        Style { stroke: Rgb { r, g, b }, width, alpha }
    }
}

/// Any shape kind over in-box vertices (polylines up to the spec's 8).
fn arb_shape() -> impl Strategy<Value = Shape> {
    prop_oneof![
        arb_vertex().prop_map(Shape::Point).boxed(),
        (arb_vertex(), arb_vertex())
            .prop_map(|(a, b)| Shape::Segment(a, b))
            .boxed(),
        prop::collection::vec(arb_vertex(), 2..=8)
            .prop_map(Shape::Polyline)
            .boxed(),
    ]
}

/// Both projection variants (AC2 runs under each).
fn arb_projection() -> impl Strategy<Value = Projection> {
    prop_oneof![
        Just(Projection::Pinhole { focal: 2.0 }),
        Just(Projection::Orthographic),
    ]
}

/// A hostile coordinate: near the camera or out to ±1e9.
fn arb_hostile_coord() -> impl Strategy<Value = f64> {
    prop_oneof![-8.0f64..8.0, -1.0e9f64..1.0e9]
}

/// A hostile vertex: a finite point anywhere, or an ideal point (weight
/// 0, non-finite Euclidean readback).
fn arb_hostile_vertex() -> impl Strategy<Value = pga::Point> {
    prop_oneof![
        (
            arb_hostile_coord(),
            arb_hostile_coord(),
            arb_hostile_coord()
        )
            .prop_map(|(x, y, z)| pga::Point::new(x, y, z))
            .boxed(),
        (
            arb_hostile_coord(),
            arb_hostile_coord(),
            arb_hostile_coord()
        )
            .prop_map(|(x, y, z)| ideal_point(x, y, z))
            .boxed(),
    ]
}

/// Any shape over hostile vertices, degenerate polylines included.
fn arb_hostile_shape() -> impl Strategy<Value = Shape> {
    prop_oneof![
        arb_hostile_vertex().prop_map(Shape::Point).boxed(),
        (arb_hostile_vertex(), arb_hostile_vertex())
            .prop_map(|(a, b)| Shape::Segment(a, b))
            .boxed(),
        prop::collection::vec(arb_hostile_vertex(), 0..=6)
            .prop_map(Shape::Polyline)
            .boxed(),
    ]
}

/// Hostile projections: focal across (1e-6, 1e3], or orthographic.
fn arb_hostile_projection() -> impl Strategy<Value = Projection> {
    prop_oneof![
        (1.0e-6f64..1.0e3)
            .prop_map(|focal| Projection::Pinhole { focal })
            .boxed(),
        Just(Projection::Orthographic).boxed(),
    ]
}

proptest! {
    // AC2 (+ AC4/AC6 riders) — camera/world duality: moving the camera by
    // M images identically to moving the world by M⁻¹. Static keys lose no
    // generality (slerp is left-invariant, SPEC-0002 §2.7); the two sides
    // compose on different ends and round differently, hence 1e-6 —
    // mathematically the property is exact.
    #[test]
    fn ac2_moving_camera_by_m_matches_moving_world_by_m_inverse(
        m in arb_motor(0.8, TAU / 4.0),
        camera_nudge in arb_motor(0.25, TAU / 16.0),
        projection in arb_projection(),
        objects in prop::collection::vec(
            (arb_shape(), arb_motor(0.8, TAU / 4.0), arb_style()),
            1..=4,
        ),
    ) {
        let c = Motor3::translator(0.0, 0.0, 8.0) * camera_nudge;
        let m_inv = m.inverse();

        let mut moved_camera = Scene::new(1.0);
        moved_camera.camera = Camera { pose: m * c, projection };
        let mut moved_world = Scene::new(1.0);
        moved_world.camera = Camera { pose: c, projection };
        for (shape, pose, style) in &objects {
            moved_camera.add(Object {
                shape: shape.clone(),
                style: *style,
                track: Track::hold(*pose),
            });
            moved_world.add(Object {
                shape: shape.clone(),
                style: *style,
                track: Track::hold(m_inv.compose(pose)),
            });
        }

        let left = moved_camera.eval(0.0);
        let right = moved_world.eval(0.0);

        // AC6 rider: re-evaluating an arbitrary generated scene is
        // bit-identical.
        prop_assert_eq!(prim_bits(&moved_camera.eval(0.0)), prim_bits(&left));

        prop_assert_eq!(left.len(), right.len(), "primitive counts must agree");
        for (i, (l, r)) in left.iter().zip(&right).enumerate() {
            let (l_tag, l_points, l_style) = prim_parts(l);
            let (r_tag, r_points, r_style) = prim_parts(r);
            prop_assert_eq!(l_tag, r_tag, "variant of primitive {}", i);
            // AC4 rider: the style rides through bit-equal on both sides.
            prop_assert_eq!(
                style_bits(l_style),
                style_bits(r_style),
                "style of primitive {}",
                i
            );
            prop_assert_eq!(l_points.len(), r_points.len(), "vertex count of primitive {}", i);
            for (j, (lp, rp)) in l_points.iter().zip(&r_points).enumerate() {
                prop_assert!(
                    (lp.x - rp.x).abs() < 1e-6 && (lp.y - rp.y).abs() < 1e-6,
                    "primitive {} vertex {}: ({}, {}) vs ({}, {})",
                    i, j, lp.x, lp.y, rp.x, rp.y
                );
            }
        }
    }

    // §4 never-NaN (supports AC1/AC3) — under hostile scenes (coordinates
    // to ±1e9, at/behind-plane points, ideal points, focal across nine
    // decades, both models, arbitrary t) no emitted primitive ever carries
    // a non-finite f64.
    #[test]
    fn never_nonfinite_under_hostile_scenes(
        camera_pose in arb_motor(4.0, TAU / 4.0),
        projection in arb_hostile_projection(),
        objects in prop::collection::vec(
            (arb_hostile_shape(), arb_motor(2.0, TAU / 4.0), arb_style()),
            1..=3,
        ),
        t in -1.0e3f64..1.0e3,
    ) {
        let mut scene = Scene::new(1.0);
        scene.camera = Camera { pose: camera_pose, projection };
        for (shape, pose, style) in objects {
            scene.add(Object { shape, style, track: Track::hold(pose) });
        }
        for value in every_f64(&scene.eval(t)) {
            prop_assert!(
                value.is_finite(),
                "a non-finite f64 left Scene::eval: {}",
                value
            );
        }
    }
}
