//! R-0001 — `Track` + `Ease`: the QA-owned acceptance suite (e2e).
//!
//! Loop step 3, TDD red: authored before the implementation exists, derived
//! from the acceptance criteria of `requirements/0001-track-ease.md`
//! (AC1–AC8) and the `spin` criteria of `specs/0001-track-ease.md` §6. Each
//! test's header comment names the criteria it verifies.
//!
//! Exactness levels are calibrated against garust ground truth (QA probe run,
//! 2026-08-20): constructor-built motors renormalize bit-identically,
//! `Motor::slerp(_, _, 0.0)` reproduces its first argument bit-for-bit, and
//! the dyadic translator midpoint is bit-exact — so AC1 and AC2's translation
//! half assert with motor equality, while trig-bearing paths assert
//! coefficient-wise at 1e-12 (the tolerance garust's own tests use).

use garust::{Motor3, Pga3};
use motoreel::{Ease, Track, TrackError};
use proptest::prelude::*;
use std::f64::consts::TAU;

// --- Shared helpers ------------------------------------------------------

/// One of the three Euclidean basis bivectors of PGA — a valid rotation
/// plane (squares to −1): `e1e2`, `e1e3`, `e2e3`.
fn pga_axis(choice: usize) -> Pga3 {
    Pga3::basis([0b0011, 0b0101, 0b0110][choice % 3])
}

/// Coefficient-wise comparison of two motors at an absolute tolerance.
fn assert_motor_approx(got: &Motor3, want: &Motor3, tol: f64, ctx: &str) {
    let g = got.versor().coeffs;
    let w = want.versor().coeffs;
    for (i, (x, y)) in g.iter().zip(w.iter()).enumerate() {
        assert!(
            (x - y).abs() < tol,
            "{ctx}: coefficient {i}: {x} vs {y} (tol {tol})"
        );
    }
}

/// The motor's 16 coefficient bit patterns — AC7's determinism currency.
fn motor_bits(m: &Motor3) -> Vec<u64> {
    m.versor().coeffs.iter().map(|c| c.to_bits()).collect()
}

/// Count a track's spans through the public surface: `ease_span` accepts
/// exactly the indices `0..span_count` (AC6) and rejects the rest (AC5).
fn span_count(track: &mut Track) -> usize {
    let mut n = 0;
    while n < 10_000 {
        if track.ease_span(n, Ease::Linear).is_err() {
            return n;
        }
        n += 1;
    }
    panic!("more than 10_000 spans — ease_span never errored");
}

// --- Anchored custom curves (SPEC-0001 F1: f(0) = 0 and f(1) = 1) --------

/// Anchored: √0 = 0 and √1 = 1, both exact in IEEE 754.
fn anchored_sqrt(u: f64) -> f64 {
    u.sqrt()
}

/// Anchored: 0² = 0 and 1² = 1.
fn anchored_square(u: f64) -> f64 {
    u * u
}

/// Anchored (the factor `u·(2u−1)·(u−1)` vanishes exactly at 0 and 1) but
/// misbehaved in between: reaches 1.75 at u = 0.25 and −0.75 at u = 0.75,
/// so `Ease::apply` must clamp its output to keep the path inside the span.
fn anchored_overshoot(u: f64) -> f64 {
    16.0 * u * (2.0 * u - 1.0) * (u - 1.0) + u
}

// --- AC1: eval at a key's exact time returns that key's motor exactly ----

// AC1 — deterministic unit case. Translator keys are exactly unit-norm, so
// construction-time renormalization is the identity and "exactly" means
// motor equality on the clamp boundaries (first/last) and on an interior
// key (reached as u = 0 of its span).
#[test]
fn ac1_eval_at_key_times_returns_key_motors_exactly() {
    let k0 = Motor3::translator(1.0, 2.0, 3.0);
    let k1 = Motor3::translator(4.0, -2.0, 6.0);
    let k2 = Motor3::translator(-1.0, 0.0, 2.0);
    let track = Track::keys([(0.0, k0), (1.0, k1), (2.5, k2)]).unwrap();
    assert_eq!(track.eval(0.0), k0, "first key (clamp boundary)");
    assert_eq!(track.eval(1.0), k1, "interior key (span start, u = 0)");
    assert_eq!(track.eval(2.5), k2, "last key (clamp boundary)");
}

// --- AC2: midpoints are the half translation / half rotation -------------

// AC2 — pure translation. Dyadic key times and dyadic translations make the
// half translation exactly representable; the constant-speed screw midpoint
// must hit it exactly (verified bit-exact against garust ground truth).
#[test]
fn ac2_translation_midpoint_is_the_exact_half_translation() {
    let track = Track::keys([
        (0.0, Motor3::translator(1.0, 2.0, 3.0)),
        (1.0, Motor3::translator(5.0, -4.0, 7.0)),
    ])
    .unwrap();
    assert_eq!(track.eval(0.5), Motor3::translator(3.0, -1.0, 5.0));

    let track = Track::keys([
        (0.25, Motor3::identity()),
        (0.75, Motor3::translator(4.0, -2.0, 6.0)),
    ])
    .unwrap();
    assert_eq!(track.eval(0.5), Motor3::translator(2.0, -1.0, 3.0));
}

// AC2 — pure rotation (≤ quarter turn): the midpoint is the half rotation,
// at ~1e-12 because sin/cos are involved (garust's own test tolerance).
#[test]
fn ac2_rotation_midpoint_is_the_half_rotation() {
    let e23 = pga_axis(2);
    let track = Track::keys([
        (0.0, Motor3::identity()),
        (1.0, Motor3::rotor(TAU / 4.0, e23)),
    ])
    .unwrap();
    assert_motor_approx(
        &track.eval(0.5),
        &Motor3::rotor(TAU / 8.0, e23),
        1e-12,
        "identity to quarter turn",
    );

    let track = Track::keys([
        (0.0, Motor3::rotor(TAU / 8.0, e23)),
        (1.0, Motor3::rotor(3.0 * TAU / 8.0, e23)),
    ])
    .unwrap();
    assert_motor_approx(
        &track.eval(0.5),
        &Motor3::rotor(TAU / 4.0, e23),
        1e-12,
        "offset-base rotation pair",
    );
}

// --- AC3: easing changes the schedule, never the path --------------------

// AC3 + AC7 (Custom) — a misbehaved-but-anchored Custom curve overshoots
// past [0, 1]; `Ease::apply` clamps its output, so the eased pose pins to
// the span's end keys instead of extrapolating past them.
#[test]
fn ac3_misbehaved_anchored_custom_is_clamped_to_span_endpoints() {
    let k0 = Motor3::translator(1.0, 1.0, 0.0);
    let k1 = Motor3::translator(3.0, -1.0, 2.0) * Motor3::rotor(0.6, pga_axis(1));
    let track = Track::keys([(0.0, k0), (1.0, k1)])
        .unwrap()
        .ease(Ease::Custom(anchored_overshoot));
    // u = 0.25 → curve says 1.75 → clamped to 1 → the span's end pose.
    assert_motor_approx(&track.eval(0.25), &k1, 1e-12, "clamped to span end");
    // u = 0.75 → curve says −0.75 → clamped to 0 → the span's start pose.
    assert_eq!(track.eval(0.75), k0, "clamped to span start");
}

// --- AC4: evaluation is total and clamps ---------------------------------

// AC4 — t at or before the first key returns the first motor; at or after
// the last key, the last motor — including far-out and infinite times.
#[test]
fn ac4_eval_clamps_at_and_beyond_the_end_keys() {
    let first = Motor3::translator(1.0, 0.0, -1.0);
    let last = Motor3::translator(-2.0, 5.0, 0.5);
    let track = Track::keys([(1.0, first), (3.0, last)]).unwrap();
    assert_eq!(track.eval(1.0), first, "exactly at the first key time");
    assert_eq!(track.eval(3.0), last, "exactly at the last key time");
    assert_eq!(track.eval(0.999_999_999), first, "just before");
    assert_eq!(track.eval(-1.0e9), first, "far before");
    assert_eq!(track.eval(f64::NEG_INFINITY), first, "-inf");
    assert_eq!(track.eval(3.000_000_001), last, "just after");
    assert_eq!(track.eval(1.0e9), last, "far after");
    assert_eq!(track.eval(f64::INFINITY), last, "+inf");
}

// AC4 + AC5 + AC6 (edge) — a single-key track is valid, has zero spans, and
// holds its pose at every t; `hold` is the infallible spelling of the same.
#[test]
fn ac4_single_key_tracks_hold_their_pose_everywhere() {
    let pose = Motor3::translator(0.5, -0.5, 2.0);
    let track = Track::keys([(2.0, pose)]).unwrap();
    for t in [-1.0e6, 0.0, 2.0, 7.5, 1.0e6] {
        assert_eq!(track.eval(t), pose, "keys-built hold at t={t}");
    }
    let held = Track::hold(pose);
    for t in [-3.0, 0.0, 42.0] {
        assert_eq!(held.eval(t), pose, "hold at t={t}");
    }
    // Zero spans: whole-track easing is a no-op, not a panic (AC6 edge)...
    let eased = Track::keys([(2.0, pose)]).unwrap().ease(Ease::SmoothStep);
    assert_eq!(eased.eval(9.0), pose);
    // ...and there is no span 0 to ease individually (AC5).
    assert!(matches!(
        Track::hold(pose).ease_span(0, Ease::Linear),
        Err(TrackError::NoSuchSpan { index: 0 })
    ));
}

// --- AC5: construction validates with typed errors -----------------------

// AC5 — empty keys are a typed error, not a panic.
#[test]
fn ac5_empty_keys_are_a_typed_error() {
    assert!(matches!(
        Track::keys(Vec::<(f64, Motor3)>::new()),
        Err(TrackError::Empty)
    ));
}

// AC5 — equal key times violate strict increase; the second key of the
// pair is reported (decision log 2026-08-20).
#[test]
fn ac5_equal_key_times_are_non_increasing() {
    let m = Motor3::identity();
    assert!(matches!(
        Track::keys([(1.0, m), (1.0, m)]),
        Err(TrackError::NonIncreasing { index: 1 })
    ));
}

// AC5 — decreasing key times violate strict increase (and are not silently
// reordered: construction refuses them outright).
#[test]
fn ac5_decreasing_key_times_are_non_increasing() {
    let m = Motor3::identity();
    assert!(matches!(
        Track::keys([(2.0, m), (1.0, m)]),
        Err(TrackError::NonIncreasing { index: 1 })
    ));
}

// AC5 — the NonIncreasing index names the first chain-breaking key
// (decided 2026-08-20 after the qa run flagged the convention as
// unpinned): the second key of a non-increasing pair — here key 2.
#[test]
fn ac5_non_increasing_index_localizes_the_violation() {
    let m = Motor3::identity();
    let err = Track::keys([(0.0, m), (1.0, m), (1.0, m), (3.0, m)]).unwrap_err();
    assert!(
        matches!(err, TrackError::NonIncreasing { index: 2 }),
        "index must name the first chain-breaking key (2), got {err:?}"
    );
}

// AC5 — key times must be finite (decided 2026-08-20 after the qa run
// flagged the single-NaN hole): a non-finite time is NonIncreasing,
// reported at its OWN index — even a lone key with no pair to compare.
#[test]
fn ac5_non_finite_key_times_error_at_their_own_index() {
    let m = Motor3::identity();
    let cases = [
        (vec![(f64::NAN, m)], 0),
        (vec![(f64::NAN, m), (1.0, m)], 0),
        (vec![(0.0, m), (f64::NAN, m), (2.0, m)], 1),
        (vec![(0.0, m), (1.0, m), (f64::NAN, m)], 2),
        (vec![(0.0, m), (f64::INFINITY, m)], 1),
        (vec![(f64::NEG_INFINITY, m), (1.0, m)], 0),
    ];
    for (case, (keys, want)) in cases.into_iter().enumerate() {
        let err = Track::keys(keys).unwrap_err();
        assert!(
            matches!(err, TrackError::NonIncreasing { index } if index == want),
            "non-finite case {case}: want index {want}, got {err:?}"
        );
    }
}

// AC5 — `ease_span` past the last span is a typed error carrying the
// requested index.
#[test]
fn ac5_ease_span_out_of_range_is_no_such_span() {
    let m = Motor3::identity();
    let mut track =
        Track::keys([(0.0, m), (1.0, Motor3::translator(1.0, 0.0, 0.0)), (2.0, m)]).unwrap();
    assert!(track.ease_span(0, Ease::SmoothStep).is_ok());
    assert!(track.ease_span(1, Ease::SmootherStep).is_ok());
    assert!(matches!(
        track.ease_span(2, Ease::Linear),
        Err(TrackError::NoSuchSpan { index: 2 })
    ));
    assert!(matches!(
        track.ease_span(usize::MAX, Ease::Linear),
        Err(TrackError::NoSuchSpan { index: usize::MAX })
    ));
}

// AC5 — TrackError is a typed, first-class error: it implements Display and
// std::error::Error (SPEC-0001 §2), for every reachable variant.
#[test]
fn ac5_track_error_implements_display_and_error() {
    let errors = [
        Track::keys(Vec::<(f64, Motor3)>::new()).unwrap_err(),
        Track::keys([(1.0, Motor3::identity()), (1.0, Motor3::identity())]).unwrap_err(),
        Track::hold(Motor3::identity())
            .ease_span(0, Ease::Linear)
            .unwrap_err(),
        Track::spin(f64::NAN, pga_axis(2), 1.0).unwrap_err(),
    ];
    for err in &errors {
        assert!(!format!("{err}").is_empty(), "empty Display for {err:?}");
        let _: &dyn std::error::Error = err;
    }
}

// --- AC6: easing is per span, with a whole-track convenience -------------

// AC6 — each span carries its own ease: easing span 0 must not change span
// 1's schedule, and vice versa. Reference poses are direct garust slerps at
// the pinned remap values (SmoothStep(0.25) = 0.15625 and
// SmootherStep(0.25) = 0.103515625, both exact dyadics).
#[test]
fn ac6_ease_span_applies_to_exactly_one_span() {
    let k0 = Motor3::identity();
    let k1 = Motor3::translator(2.0, 0.0, 0.0);
    let k2 = Motor3::translator(2.0, 4.0, 0.0);
    let mut track = Track::keys([(0.0, k0), (1.0, k1), (2.0, k2)]).unwrap();

    track.ease_span(0, Ease::SmoothStep).unwrap();
    assert_motor_approx(
        &track.eval(0.25),
        &k0.slerp(&k1, 0.15625),
        1e-12,
        "span 0 eased with SmoothStep",
    );
    assert_motor_approx(
        &track.eval(1.25),
        &k1.slerp(&k2, 0.25),
        1e-12,
        "span 1 still Linear",
    );

    track.ease_span(1, Ease::SmootherStep).unwrap();
    assert_motor_approx(
        &track.eval(1.25),
        &k1.slerp(&k2, 0.103515625),
        1e-12,
        "span 1 eased with SmootherStep",
    );
    assert_motor_approx(
        &track.eval(0.25),
        &k0.slerp(&k1, 0.15625),
        1e-12,
        "span 0 unchanged by easing span 1",
    );
}

// AC6 — the consuming whole-track convenience sets every span, including
// overriding earlier per-span choices.
#[test]
fn ac6_whole_track_ease_sets_every_span() {
    let k0 = Motor3::identity();
    let k1 = Motor3::translator(2.0, 0.0, 0.0);
    let k2 = Motor3::translator(2.0, 4.0, 0.0);
    let keys = [(0.0, k0), (1.0, k1), (2.0, k2)];

    let track = Track::keys(keys).unwrap().ease(Ease::SmoothStep);
    assert_motor_approx(
        &track.eval(0.25),
        &k0.slerp(&k1, 0.15625),
        1e-12,
        "span 0 eased by whole-track ease",
    );
    assert_motor_approx(
        &track.eval(1.25),
        &k1.slerp(&k2, 0.15625),
        1e-12,
        "span 1 eased by whole-track ease",
    );

    // ease() overrides per-span setup on every span.
    let mut track = Track::keys(keys).unwrap();
    track.ease_span(0, Ease::SmootherStep).unwrap();
    let track = track.ease(Ease::Linear);
    assert_motor_approx(
        &track.eval(0.25),
        &k0.slerp(&k1, 0.25),
        1e-12,
        "whole-track ease overrides span 0 back to Linear",
    );
}

// --- AC7: ease variants, Clone, bit-deterministic evaluation -------------

// AC7 — the four variants exist and `apply` remaps as specified: dyadic
// inputs give exactly representable outputs, both bounds are clamped, and
// every curve is anchored.
#[test]
fn ac7_ease_variants_and_apply_semantics() {
    assert_eq!(Ease::Linear.apply(0.25), 0.25);
    assert_eq!(Ease::SmoothStep.apply(0.25), 0.15625);
    assert_eq!(Ease::SmoothStep.apply(0.5), 0.5);
    assert_eq!(Ease::SmootherStep.apply(0.25), 0.103515625);
    assert_eq!(Ease::SmootherStep.apply(0.5), 0.5);
    assert_eq!(Ease::Custom(anchored_square).apply(0.5), 0.25);
    for e in [
        Ease::Linear,
        Ease::SmoothStep,
        Ease::SmootherStep,
        Ease::Custom(anchored_sqrt),
        Ease::Custom(anchored_overshoot),
    ] {
        assert_eq!(e.apply(0.0), 0.0, "{e:?} anchored at 0");
        assert_eq!(e.apply(1.0), 1.0, "{e:?} anchored at 1");
        assert_eq!(e.apply(-3.0), 0.0, "{e:?} clamps input from below");
        assert_eq!(e.apply(2.0), 1.0, "{e:?} clamps input from above");
    }
    // Output clamping: a misbehaved-but-anchored Custom cannot escape [0, 1].
    assert_eq!(Ease::Custom(anchored_overshoot).apply(0.25), 1.0);
    assert_eq!(Ease::Custom(anchored_overshoot).apply(0.75), 0.0);
    // NaN input clamps low (decided 2026-08-20; f64::clamp would propagate).
    assert_eq!(Ease::Linear.apply(f64::NAN), 0.0);
    assert_eq!(Ease::SmootherStep.apply(f64::NAN), 0.0);
}

// AC7 — Track is Clone, and evaluation is deterministic: re-evaluation, a
// clone, and a track rebuilt from identical inputs all produce bit-identical
// motor coefficients (compared as bits, not approximately).
#[test]
fn ac7_clone_and_rebuild_evaluate_bit_identically() {
    let keys = vec![
        (0.0, Motor3::identity()),
        (
            1.0,
            Motor3::translator(1.5, -2.0, 0.5) * Motor3::rotor(0.9, pga_axis(2)),
        ),
        (2.0, Motor3::rotor(-0.7, pga_axis(0))),
        (
            3.0,
            Motor3::translator(-3.0, 1.0, 2.0) * Motor3::rotor(0.4, pga_axis(1)),
        ),
    ];
    let build = || {
        let mut track = Track::keys(keys.clone()).unwrap();
        track.ease_span(0, Ease::SmootherStep).unwrap();
        track
            .ease_span(2, Ease::Custom(anchored_overshoot))
            .unwrap();
        track
    };
    let track = build();
    let clone = track.clone();
    let rebuilt = build();
    for t in [-0.5, 0.0, 0.31, 1.0, 1.77, 2.5, 2.9, 3.0, 9.0] {
        let reference = motor_bits(&track.eval(t));
        assert_eq!(reference, motor_bits(&track.eval(t)), "re-eval at t={t}");
        assert_eq!(reference, motor_bits(&clone.eval(t)), "clone at t={t}");
        assert_eq!(reference, motor_bits(&rebuilt.eval(t)), "rebuild at t={t}");
    }
}

// --- AC8: dense sampled tracks are first-class ---------------------------

// AC8 — a 240-key uniformly spaced track (60 fps × 4 s, the recorded-rollout
// shape) built through the *same* `Track::keys` constructor as an authored
// track: exact at every key time (AC1's contract, shared path) and
// interpolating between keys. The samples come from the constant screw
// M(t) = rotor(w·t, e23) · translator(v·t, 0, 0) — commuting generators, a
// one-parameter subgroup — so between-key evaluation must land back on the
// sampled motion itself.
#[test]
fn ac8_dense_rollout_track_is_first_class() {
    let (w, v) = (TAU / 4.0, 0.5);
    let e23 = pga_axis(2);
    let pose = |t: f64| Motor3::rotor(w * t, e23) * Motor3::translator(v * t, 0.0, 0.0);
    let keys: Vec<(f64, Motor3)> = (0..240)
        .map(|i| {
            let t = f64::from(i) / 60.0;
            (t, pose(t))
        })
        .collect();
    let track = Track::keys(keys.iter().copied()).unwrap();

    // Keys are renormalized on ingest (SPEC-0001; same convention as the
    // AC1 property above) — a rotor·translator product's norm² is 1 ± ulp,
    // so the stored key is the renormalized one. Reconciled after the
    // implementation surfaced the inconsistency with the raw-key compare.
    for &(t, m) in &keys {
        assert_eq!(track.eval(t), m.renormalize(), "exact at key time t={t}");
    }
    for i in [0_u32, 1, 58, 119, 177, 238] {
        for frac in [0.25, 0.618_033_988_7] {
            let t = (f64::from(i) + frac) / 60.0;
            assert_motor_approx(
                &track.eval(t),
                &pose(t),
                1e-9,
                &format!("between keys at t={t}"),
            );
        }
    }
}

// --- SPEC-0001 §6: spin ---------------------------------------------------

// spin — the pose at t equals rotor(rate·t, plane) across a full TAU sweep
// (key times and interior times), beyond-duration times clamp (AC4 applies),
// and a negative rate spins the other way.
#[test]
fn spin_pose_matches_rotor_of_rate_times_t_through_a_full_turn() {
    let e23 = pga_axis(2);
    let track = Track::spin(TAU, e23, 1.0).unwrap();
    let sweep = [
        0.0,
        0.0625,
        0.125,
        0.2,
        0.25,
        1.0 / 3.0,
        0.4,
        0.5,
        0.618_033_988_7,
        0.75,
        0.8,
        0.875,
        0.9,
        0.96,
        1.0,
    ];
    for t in sweep {
        assert_motor_approx(
            &track.eval(t),
            &Motor3::rotor(TAU * t, e23),
            1e-12,
            &format!("full-turn sweep at t={t}"),
        );
    }
    assert_eq!(
        motor_bits(&track.eval(2.0)),
        motor_bits(&track.eval(1.0)),
        "past the duration the spin clamps to its final pose"
    );

    let back = Track::spin(-TAU / 2.0, e23, 1.0).unwrap();
    assert_motor_approx(
        &back.eval(0.5),
        &Motor3::rotor(-TAU / 4.0, e23),
        1e-12,
        "negative rate, halfway",
    );
    assert_motor_approx(
        &back.eval(1.0),
        &Motor3::rotor(-TAU / 2.0, e23),
        1e-12,
        "negative rate, full duration",
    );
}

// spin — the emitted subdivision is ceil(|rate|·duration / (TAU/4)) spans
// (at most a quarter turn per span), observed through the public surface:
// ease_span accepts exactly that many indices.
#[test]
fn spin_subdivides_into_ceil_quarter_turn_spans() {
    let e23 = pga_axis(2);
    let cases = [
        (TAU, 1.0, 4),        // one full turn → four quarter-turn spans
        (TAU / 4.0, 1.0, 1),  // exactly a quarter turn → one span
        (1.0, 1.0, 1),        // 1 rad < TAU/4
        (1.0, 2.0, 2),        // 2 rad → ceil(1.27…)
        (-TAU, 0.5, 2),       // |rate|·duration = π → exactly two
        (3.0 * TAU, 1.0, 12), // three turns
    ];
    for (rate, duration, expected) in cases {
        let mut track = Track::spin(rate, e23, duration).unwrap();
        assert_eq!(
            span_count(&mut track),
            expected,
            "span count of spin({rate}, e23, {duration})"
        );
    }
}

// spin — the plane bivector is normalized by spin: any non-zero multiple of
// a unit plane produces the same rotation.
#[test]
fn spin_normalizes_the_plane_bivector() {
    let e12 = pga_axis(0);
    let track = Track::spin(TAU / 2.0, e12 * 2.5, 2.0).unwrap();
    for t in [0.0, 0.31, 1.0, 1.5, 2.0] {
        assert_motor_approx(
            &track.eval(t),
            &Motor3::rotor(TAU / 2.0 * t, e12),
            1e-12,
            &format!("scaled plane at t={t}"),
        );
    }
}

// spin — F5 validation: non-finite rate, and non-finite or non-positive
// duration, are the typed InvalidSpin error.
#[test]
fn spin_rejects_non_finite_rate_and_invalid_duration() {
    let e23 = pga_axis(2);
    let bad = [
        (f64::NAN, 1.0),
        (f64::INFINITY, 1.0),
        (f64::NEG_INFINITY, 1.0),
        (1.0, f64::NAN),
        (1.0, f64::INFINITY),
        (1.0, f64::NEG_INFINITY),
        (1.0, 0.0),
        (1.0, -1.0),
    ];
    for (rate, duration) in bad {
        assert!(
            matches!(
                Track::spin(rate, e23, duration),
                Err(TrackError::InvalidSpin)
            ),
            "spin({rate}, e23, {duration}) must be InvalidSpin"
        );
    }
}

// --- Property tests (AC1, AC3, AC4) --------------------------------------

prop_compose! {
    /// A random screw motion with rotation ≤ 1 rad and translation ≤ 5 per
    /// key — bounded so no adjacent key pair approaches slerp's antipodal
    /// fold, where the short-way screw is ill-conditioned.
    fn arb_motor()(
        angle in -1.0f64..1.0,
        axis in 0usize..3,
        dx in -5.0f64..5.0,
        dy in -5.0f64..5.0,
        dz in -5.0f64..5.0,
    ) -> Motor3 {
        Motor3::translator(dx, dy, dz) * Motor3::rotor(angle, pga_axis(axis))
    }
}

prop_compose! {
    /// A valid key vector: 2..=7 keys at strictly increasing times.
    fn arb_keys()(
        t0 in -10.0f64..10.0,
        first in arb_motor(),
        rest in prop::collection::vec((0.1f64..2.0, arb_motor()), 1..7),
    ) -> Vec<(f64, Motor3)> {
        let mut keys = vec![(t0, first)];
        let mut t = t0;
        for (dt, m) in rest {
            t += dt;
            keys.push((t, m));
        }
        keys
    }
}

/// Every ease the API offers — drawing only ANCHORED custom curves, per the
/// SPEC-0001 F1 `Custom` contract (an unanchored curve is outside the API's
/// domain and would rightly break AC1 at interior keys).
fn arb_ease() -> impl Strategy<Value = Ease> {
    prop_oneof![
        Just(Ease::Linear),
        Just(Ease::SmoothStep),
        Just(Ease::SmootherStep),
        Just(Ease::Custom(anchored_sqrt)),
        Just(Ease::Custom(anchored_square)),
        Just(Ease::Custom(anchored_overshoot)),
    ]
}

proptest! {
    // AC1 (property) — over random valid tracks with random anchored eases
    // on every span, eval at each key's exact time returns that key's motor
    // exactly. The expected value is the construction contract itself: the
    // key as renormalized once by `Motor::renormalize` (SPEC-0001 §2), which
    // is bit-identical to the raw key for these constructor-built motors.
    #[test]
    fn ac1_eval_at_key_times_property(
        keys in arb_keys(),
        eases in prop::collection::vec(arb_ease(), 7),
    ) {
        let mut track = Track::keys(keys.iter().copied()).unwrap();
        for span in 0..keys.len() - 1 {
            track.ease_span(span, eases[span % eases.len()]).unwrap();
        }
        for (i, (t, m)) in keys.iter().enumerate() {
            prop_assert_eq!(track.eval(*t), m.renormalize(), "key {} at t={}", i, t);
        }
    }

    // AC3 (property) — easing changes the schedule, never the path: for any
    // ease e and any time t inside a span with local parameter u, the eased
    // evaluation equals the linear evaluation at e(u). The oracle is a
    // direct garust slerp of the (renormalized) span keys at the public
    // remap `e.apply(u)` — garust-only, independent of Track's interior.
    #[test]
    fn ac3_eased_eval_is_linear_eval_at_remapped_parameter(
        keys in arb_keys(),
        e in arb_ease(),
        span_pick in 0usize..64,
        u in 0.01f64..0.99,
    ) {
        let span = span_pick % (keys.len() - 1);
        let (t0, m0) = keys[span];
        let (t1, m1) = keys[span + 1];
        let t = t0 + u * (t1 - t0);

        let eased = Track::keys(keys.iter().copied()).unwrap().ease(e);
        let s = e.apply((t - t0) / (t1 - t0));
        let want = m0.renormalize().slerp(&m1.renormalize(), s);
        let got = eased.eval(t);
        let g = got.versor().coeffs;
        let w = want.versor().coeffs;
        for (i, (x, y)) in g.iter().zip(w.iter()).enumerate() {
            prop_assert!(
                (x - y).abs() < 1e-9,
                "coefficient {}: {} vs {} (span {}, u {}, ease {:?})",
                i, x, y, span, u, e
            );
        }
    }

    // AC4 (property) — any t outside the key range clamps: it evaluates to
    // the end key's motor, bit-identical to evaluating at the end key time.
    #[test]
    fn ac4_out_of_range_eval_clamps_to_end_poses(
        keys in arb_keys(),
        below in 0.001f64..1.0e6,
        above in 0.001f64..1.0e6,
    ) {
        let track = Track::keys(keys.iter().copied()).unwrap();
        let (t_first, first) = keys[0];
        let (t_last, last) = keys[keys.len() - 1];
        prop_assert_eq!(track.eval(t_first - below), first.renormalize());
        prop_assert_eq!(track.eval(t_last + above), last.renormalize());
        prop_assert_eq!(
            motor_bits(&track.eval(t_first - below)),
            motor_bits(&track.eval(t_first))
        );
        prop_assert_eq!(
            motor_bits(&track.eval(t_last + above)),
            motor_bits(&track.eval(t_last))
        );
    }
}

// --- qa-gap closures (SPEC-0001 decision log, 2026-08-20) -----------------
// The qa run flagged these as ambiguous and left them untested; the spec
// now decides them, and these tests pin the decisions.

// eval(NaN) clamps low to the first pose — evaluation stays total (AC4).
#[test]
fn gap_eval_nan_clamps_to_the_first_pose() {
    let a = Motor3::translator(1.0, 0.0, 0.0);
    let b = Motor3::translator(2.0, 0.0, 0.0);
    let track = Track::keys([(0.0, a), (1.0, b)]).unwrap();
    assert_eq!(track.eval(f64::NAN), track.eval(-1.0));
}

// spin(rate = 0) is an identity hold, not a degenerate zero-span track.
#[test]
fn gap_spin_zero_rate_is_an_identity_hold() {
    let track = Track::spin(0.0, pga_axis(2), 4.0).unwrap();
    for t in [-1.0, 0.0, 2.0, 4.0, 100.0] {
        assert_eq!(track.eval(t), Motor3::identity());
    }
}

// A plane with no Euclidean part (an ideal plane, e.g. e01) has no
// rotation rate about it: InvalidSpin.
#[test]
fn gap_spin_ideal_plane_is_invalid() {
    let ideal = Pga3::basis(0b1001); // e0∧e1: zero Euclidean part
    assert!(matches!(
        Track::spin(1.0, ideal, 1.0),
        Err(TrackError::InvalidSpin)
    ));
}
