//! R-0004 — Physics playback: recording a garust-physics rollout into
//! motor tracks, the wireframe vocabulary, and the tumbling box: the
//! QA-owned acceptance suite (e2e).
//!
//! Loop step 3, TDD red: authored before the implementation exists, derived
//! from the acceptance criteria of `requirements/0004-physics-playback.md`
//! (AC1–AC7) and SPEC-0004 §6's test plan (which restates them in terms the
//! public API can actually observe). Each test's header comment names the
//! criteria it verifies. One file per requirement, the house convention
//! since SPEC-0003.
//!
//! Criterion → test map:
//!
//! - AC1  `ac1_keys_land_at_derived_times_with_the_stepwise_poses`,
//!   `ac1_key_count_is_steps_plus_one_by_clamping`,
//!   `ac1_tracks_come_back_in_input_order`
//! - AC2  `ac2_a_recorded_track_drives_an_object_through_eval_and_render`,
//!   `ac2_only_record_rs_names_garust_physics`
//! - AC3  `ac3_recording_leaves_the_callers_bodies_untouched`,
//!   `ac3_re_recording_from_the_same_state_reproduces_the_rollout`
//! - AC4  `ac4_two_recordings_are_bit_identical_and_render_identical_frames`
//! - AC5  `ac5_edges_emit_one_styled_primitive_with_every_segment`,
//!   `ac5_one_bad_endpoint_culls_the_whole_wireframe`,
//!   `ac5_edges_never_leak_a_non_finite_coordinate` (proptest)
//! - AC6  `ac6_edge_template_matches_its_pinned_bytes`,
//!   `ac6_r0003_golden_fixture_still_passes_untouched`
//! - AC7  `ac7_tumbling_box_240_frames_twice_bit_identical`,
//!   `ac7_tumbling_box_flips_end_over_end_at_the_measured_times`
//! - SPEC-0004 §6 `RecordError` bullet
//!   `record_error_invalid_timestep_for_zero_negative_nan_and_infinite_dt`,
//!   `record_error_zero_steps_yields_single_key_holds`,
//!   `record_error_empty_bodies_yields_an_empty_vec`,
//!   `record_error_track_arm_is_reachable_without_a_panic`,
//!   `record_error_display_error_source_and_from`
//! - SPEC-0004 §2.1 signature `record_takes_a_joint_slice_it_does_not_need`
//!
//! Exactness levels, calibrated against the real integrator and the real
//! signed-off modules via a spec-faithful stub (QA probe run, 2026-08-22):
//!
//! - `Track` exposes no key iterator, no `len`, and deliberately no
//!   `PartialEq`, so every claim about "the recorded keys" is observed
//!   through `Track::eval` at the derived key times — evaluation at a key
//!   time returns that key's motor bit-exactly (R-0001 AC1, the mechanism
//!   `ac8_dense_rollout_track_is_first_class` already relies on) — and key
//!   *count* is proxied by clamping past `steps as f64 * dt` (SPEC-0004 §6).
//! - Keys are renormalized on ingest (SPEC-0001), so the reference pose a
//!   key is compared against is `Body::pose().renormalize()`, exactly as
//!   R-0001 AC8 reconciled it.
//! - Recording is a pure function of its inputs, so AC1/AC3/AC4 assert
//!   `to_bits` equality over all 16 motor coefficients — no tolerance. The
//!   derived-vs-accumulated distinction is real at that exactness: an
//!   accumulating recorder differs from a deriving one at 13 of 21 key
//!   times for `dt = 0.1, steps = 20` (probe-measured).
//! - AC7's flip signal is read from the recorded poses, not from angular
//!   velocity (state the recorder discards): the body `+y` axis
//!   transformed by each key pose swings between ±1, and the measured sign
//!   reversals land at t = 0.6875 / 2.0541666… / 3.425 s — reproduced
//!   exactly by four different spellings of SPEC-0004 §2.4's moments, and
//!   absent entirely (0 flips) when the perturbation is dropped.
//! - `Motor3::renormalize` is **not** bit-idempotent (it moves 21 of 121
//!   probe poses by up to 4.4e-16), so AC2's authored twin is built from
//!   the raw stepwise poses — the same input the recorder hands
//!   `Track::keys` — never from `eval` output re-ingested.
//! - The SVG byte grammar is a fixed template over shortest-roundtrip `f64`
//!   `Display`, so AC6 asserts full-document byte equality, no XML parser.
//!
//! Note for the implementation (loop step 5): adding the `Prim2::Edges`
//! variant makes `tests/r0002_scene_camera.rs`'s `prim_parts` helper a
//! non-exhaustive `match` and that file stops compiling. It needs one arm,
//! the same three lines this file's own `prim_parts` uses. That is the only
//! landed test file the amendment touches — R-0002's assertions and
//! R-0003's golden bytes are otherwise unaffected, which AC6 re-proves here.

use std::fs;
use std::path::{Path, PathBuf};

use garust::physics::{Body, Inertia, Joint, RigidBody, World};
use garust::{pga, Motor3, Pga3};
use motoreel::{
    record, Camera, FrameSink, Object, Prim2, Projection, Pt2, RecordError, Rgb, Scene, Shape,
    Style, SvgSink, Track, TrackError,
};
use proptest::prelude::*;
use std::f64::consts::TAU;

/// The shipped demo scene, included from the example source itself so the
/// demo and this acceptance suite cannot drift apart (SPEC-0004 §2.0, the
/// convention SPEC-0003 §2.7 established; the shared file names engine
/// items through `motoreel::` paths and garust's through `garust::`).
/// `rustfmt::skip` keeps the format gate green while the product file does
/// not exist yet (TDD red); once it lands, rustfmt still reaches it through
/// the example target's own `mod scene;`.
#[rustfmt::skip]
#[path = "../examples/tumbling_box/scene.rs"]
mod scene;

// --- Shared helpers ------------------------------------------------------

/// A clean per-test scratch directory under `CARGO_TARGET_TMPDIR` (the
/// SPEC-0003 test-infra decision): stale frames from a previous run are
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

/// The motor's 16 coefficient bit patterns — the determinism currency
/// R-0001 AC7 established (stricter than `==`, which admits ±0 crossings).
fn motor_bits(m: &Motor3) -> Vec<u64> {
    m.versor().coeffs.iter().map(|c| c.to_bits()).collect()
}

/// The derived key times of a rollout: `t_i = i as f64 * dt`, never summed.
fn key_times(dt: f64, steps: usize) -> Vec<f64> {
    (0..=steps).map(|i| i as f64 * dt).collect()
}

/// A recorded track's observable content: `eval` at every derived key time,
/// as bits. `Track` has no key iterator and no `PartialEq`, so this is the
/// public-API form of "every key motor" (SPEC-0004 §6 AC1/AC4).
fn track_bits(track: &Track, dt: f64, steps: usize) -> Vec<Vec<u64>> {
    key_times(dt, steps)
        .into_iter()
        .map(|t| motor_bits(&track.eval(t)))
        .collect()
}

/// Every recorded track's observable content, in input order.
fn rollout_bits(tracks: &[Track], dt: f64, steps: usize) -> Vec<Vec<Vec<u64>>> {
    tracks.iter().map(|t| track_bits(t, dt, steps)).collect()
}

/// A body's complete state as bits — AC3's "unmodified" is bit-level,
/// stricter than `Body`'s derived `PartialEq`.
fn body_bits(body: &Body) -> Vec<u64> {
    let r = &body.rigid;
    let mut bits = motor_bits(&r.orientation);
    bits.extend(r.angular_momentum.coeffs.iter().map(|c| c.to_bits()));
    bits.extend(r.position.iter().map(|c| c.to_bits()));
    bits.extend(r.linear_momentum.iter().map(|c| c.to_bits()));
    bits.push(r.mass.to_bits());
    bits.extend(body.inertia.moments().iter().map(|c| c.to_bits()));
    bits.push(body.radius.to_bits());
    bits.push(body.restitution.to_bits());
    bits.push(body.mu.to_bits());
    bits
}

/// Every body's state as bits, in slice order.
fn bodies_bits(bodies: &[Body]) -> Vec<Vec<u64>> {
    bodies.iter().map(body_bits).collect()
}

/// A tumbling test body: three distinct principal moments, a spin with a
/// component about every principal plane, an offset centre of mass and a
/// drift — so its rollout is non-trivial in all six degrees of freedom and
/// two bodies built with different seeds can never be confused.
fn spinner(seed: f64) -> Body {
    let inertia = Inertia::principal([1.0 + seed, 2.0 + seed, 3.0 + seed]);
    let planes = Inertia::principal_planes();
    let omega = planes[0] * (0.7 + seed) + planes[1] * (1.3 - seed) + planes[2] * 0.31;
    let mut body = Body::ball(1.0, 0.25);
    body.rigid = RigidBody::spinning(Motor3::identity(), &inertia, omega);
    body.inertia = inertia;
    body.rigid.position = [3.0 * seed, 0.5, -0.25];
    body.rigid.linear_momentum = [0.25, -0.5, 0.125];
    body
}

/// A world with gravity — the recorder must carry the world's settings, so
/// the reference rollout below is not accidentally the free-body one.
fn test_world() -> World {
    World::new().with_gravity([0.0, -9.81, 0.0])
}

/// The independent reference: step a private copy of the bodies exactly
/// like the recorder must, collecting each body's pose after each step.
/// `poses[b][i]` is body `b`'s pose after `i` steps, `i = 0` being the
/// pre-step pose.
fn reference_poses(
    world: &World,
    bodies: &[Body],
    joints: &[Joint],
    dt: f64,
    steps: usize,
) -> Vec<Vec<Motor3>> {
    let mut state = bodies.to_vec();
    let mut poses: Vec<Vec<Motor3>> = state.iter().map(|b| vec![b.rigid.pose()]).collect();
    for _ in 1..=steps {
        world.step(&mut state, joints, dt);
        for (row, b) in poses.iter_mut().zip(state.iter()) {
            row.push(b.pose());
        }
    }
    poses
}

/// A primitive flattened to (variant tag, vertices in emission order,
/// style) — extended from R-0002's helper with the `Edges` tag.
fn prim_parts(prim: &Prim2) -> (u8, Vec<Pt2>, Style) {
    match prim {
        Prim2::Point { at, style } => (0, vec![*at], *style),
        Prim2::Segment { a, b, style } => (1, vec![*a, *b], *style),
        Prim2::Polyline { points, style } => (2, points.clone(), *style),
        Prim2::Edges { segments, style } => (
            3,
            segments.iter().flat_map(|(a, b)| [*a, *b]).collect(),
            *style,
        ),
    }
}

/// Every f64 an emitted primitive carries (coordinates and style scalars) —
/// the never-non-finite invariant's audit surface (R-0002 §2.2, which AC5
/// extends to `Edges`).
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

/// One of the three Euclidean basis bivectors of PGA — a valid rotation
/// plane (squares to −1): `e1e2`, `e1e3`, `e2e3`.
fn pga_axis(choice: usize) -> Pga3 {
    Pga3::basis([0b0011, 0b0101, 0b0110][choice % 3])
}

/// An ideal point (a direction): the difference of two unit-weight points
/// has weight 0, so its `to_euclidean` weight-divide is non-finite — the
/// cull guard's job (R-0002 §2.5).
fn ideal_point(x: f64, y: f64, z: f64) -> pga::Point {
    pga::Point::from_multivector(Pga3::point(x, y, z) - Pga3::point(0.0, 0.0, 0.0))
}

/// The twelve edges of an axis-aligned box, from its half-extents — the
/// wireframe a solid needs and a single polyline cannot trace (R-0004 §1).
fn box_edges(hx: f64, hy: f64, hz: f64) -> Vec<(pga::Point, pga::Point)> {
    let half = [hx, hy, hz];
    let corner = |i: usize| {
        let sign = |bit: usize| if i & (1 << bit) == 0 { -1.0 } else { 1.0 };
        pga::Point::new(sign(0) * half[0], sign(1) * half[1], sign(2) * half[2])
    };
    let mut edges = Vec::with_capacity(12);
    for i in 0..8usize {
        for bit in 0..3usize {
            let j = i | (1 << bit);
            if j != i {
                edges.push((corner(i), corner(j)));
            }
        }
    }
    edges
}

/// A scene holding one wireframe under a pinhole camera 4 back, focal 2 —
/// the geometry sits at view depth 4 and every corner is comfortably
/// inside the frame.
fn wireframe_scene(edges: Vec<(pga::Point, pga::Point)>, style: Style) -> Scene {
    let mut scene = Scene::new(1.0);
    scene.camera = Camera::pinhole(Motor3::translator(0.0, 0.0, 4.0), 2.0);
    scene.add(Object::edges(edges).with_style(style));
    scene
}

// --- AC1: keys at derived times carrying the stepwise poses --------------

// AC1 — recording `n` steps at fixed `dt` puts key `i` at exactly
// `i as f64 * dt` carrying the pose after `i` steps, key 0 being the
// pre-step pose. Observed through `Track::eval` at each derived key time
// (SPEC-0004 §6: `Track` has no key iterator), compared bit-for-bit against
// an independently stepped reference rollout — renormalized, because keys
// are renormalized on ingest (SPEC-0001; the same reconciliation R-0001
// AC8 made). `dt = 0.1` is deliberately non-dyadic: a recorder that summed
// `t += dt` instead of deriving `i as f64 * dt` misses these bits at 13 of
// the 21 key times (probe-measured), so this is a real test of the law.
#[test]
fn ac1_keys_land_at_derived_times_with_the_stepwise_poses() {
    let world = test_world();
    let bodies = [spinner(0.0), spinner(1.0)];
    let joints: Vec<Joint> = Vec::new();
    let (dt, steps) = (0.1, 20);

    let tracks = record(&world, &bodies, &joints, dt, steps).expect("a valid rollout");
    let reference = reference_poses(&world, &bodies, &joints, dt, steps);
    assert_eq!(tracks.len(), bodies.len(), "one track per body");

    for (b, track) in tracks.iter().enumerate() {
        for (i, t) in key_times(dt, steps).into_iter().enumerate() {
            assert_eq!(
                motor_bits(&track.eval(t)),
                motor_bits(&reference[b][i].renormalize()),
                "body {b} key {i}: the pose at t = {t} must be the pose after {i} steps"
            );
        }
    }

    // Key 0 is the initial pose, before any step — stated separately
    // because it is the half of AC1 an off-by-one would silently break.
    for (b, track) in tracks.iter().enumerate() {
        assert_eq!(
            motor_bits(&track.eval(0.0)),
            motor_bits(&bodies[b].rigid.pose().renormalize()),
            "body {b}: key 0 is the pose before any step"
        );
    }

    // The rollout actually moves — otherwise every assertion above would
    // hold for a recorder that never stepped anything.
    assert_ne!(
        motor_bits(&tracks[0].eval(0.0)),
        motor_bits(&tracks[0].eval(steps as f64 * dt)),
        "the recorded rollout must not be a constant pose"
    );
}

// AC1 — key count is `steps + 1`, proxied through clamping (SPEC-0004 §6):
// evaluating anywhere past `steps as f64 * dt` returns the same motor as
// evaluating at it, and evaluating before 0 returns key 0. `steps + 1`
// distinct key times are each individually exact (the test above), so no
// key is missing; clamping shows none is extra.
#[test]
fn ac1_key_count_is_steps_plus_one_by_clamping() {
    let world = test_world();
    let bodies = [spinner(0.0)];
    let joints: Vec<Joint> = Vec::new();
    let (dt, steps) = (1.0 / 240.0, 120);

    let tracks = record(&world, &bodies, &joints, dt, steps).expect("a valid rollout");
    let last = steps as f64 * dt;
    let want = motor_bits(&tracks[0].eval(last));
    for beyond in [last, last + 1e-9, last + 1.0, 1.0e12, f64::INFINITY] {
        assert_eq!(
            motor_bits(&tracks[0].eval(beyond)),
            want,
            "t = {beyond} is past the last key {last} and must clamp to it"
        );
    }
    let first = motor_bits(&tracks[0].eval(0.0));
    for before in [-1e-9, -1.0, f64::NEG_INFINITY] {
        assert_eq!(
            motor_bits(&tracks[0].eval(before)),
            first,
            "t = {before} is before key 0 and must clamp to it"
        );
    }
    assert_ne!(first, want, "the first and last keys must differ");
}

// AC1 — one `Track` per body, in input order: recording the same three
// bodies in a permuted order returns the permuted tracks, bit-for-bit, and
// the three tracks are mutually distinguishable so the claim is not
// vacuous. (No contact occurs between these well-separated bodies, so the
// rollouts are independent of slice order — probe-confirmed.)
#[test]
fn ac1_tracks_come_back_in_input_order() {
    let world = test_world();
    let (a, b, c) = (spinner(0.0), spinner(1.0), spinner(2.0));
    let joints: Vec<Joint> = Vec::new();
    let (dt, steps) = (1.0 / 120.0, 60);

    let forward = record(&world, &[a, b, c], &joints, dt, steps).expect("a valid rollout");
    let reverse = record(&world, &[c, b, a], &joints, dt, steps).expect("a valid rollout");
    assert_eq!(forward.len(), 3);
    assert_eq!(reverse.len(), 3);

    for (i, j) in [(0, 2), (1, 1), (2, 0)] {
        assert_eq!(
            track_bits(&forward[i], dt, steps),
            track_bits(&reverse[j], dt, steps),
            "track {i} of the forward order is track {j} of the reversed one"
        );
    }
    let mid = steps as f64 * dt / 2.0;
    assert_ne!(
        motor_bits(&forward[0].eval(mid)),
        motor_bits(&forward[1].eval(mid))
    );
    assert_ne!(
        motor_bits(&forward[1].eval(mid)),
        motor_bits(&forward[2].eval(mid))
    );
}

// --- AC2: a recorded track is ordinary track data ------------------------

// AC2 — a recorded track goes into an `Object` through `with_track` and
// renders through `Scene::eval` / `Scene::render` exactly like an authored
// one. The sharpest observable form of "nothing downstream learns that a
// simulation produced it": an authored `Track::keys` built from the very
// same `(time, Motor)` samples renders byte-identical frames.
#[test]
fn ac2_a_recorded_track_drives_an_object_through_eval_and_render() {
    let world = test_world();
    let bodies = [spinner(0.0)];
    let joints: Vec<Joint> = Vec::new();
    let (dt, steps) = (1.0 / 120.0, 120);
    let mut tracks = record(&world, &bodies, &joints, dt, steps).expect("a valid rollout");
    let recorded = tracks.remove(0);

    // The same `(time, Motor)` samples, authored by hand through the
    // ordinary constructor from an independently stepped rollout. Built
    // from the *raw* poses, not from `recorded.eval`: `Track::keys`
    // renormalizes on ingest and `Motor3::renormalize` is not bit-
    // idempotent (it moves 21 of 121 probe poses by up to 4.4e-16), so
    // re-authoring an already-ingested key would compare a
    // twice-renormalized motor against a once-renormalized one.
    let authored = Track::keys(
        reference_poses(&world, &bodies, &joints, dt, steps)[0]
            .iter()
            .enumerate()
            .map(|(i, pose)| (i as f64 * dt, *pose))
            .collect::<Vec<_>>(),
    )
    .expect("the recorded samples are strictly increasing");

    let build = |track: Track| {
        let mut scene = Scene::new(1.0);
        scene.camera = Camera::pinhole(Motor3::translator(0.0, 0.0, 8.0), 2.0);
        scene.add(Object::edges(box_edges(0.3, 0.2, 0.1)).with_track(track));
        scene
    };

    // `Scene::eval` is happy at an interior, slerp-bearing time.
    let simulated = build(recorded);
    let prims = simulated.eval(0.37);
    assert_eq!(prims.len(), 1, "one object, one primitive");
    match &prims[0] {
        Prim2::Edges { segments, .. } => assert_eq!(segments.len(), 12),
        other => panic!("a recorded track must still emit its shape's primitive: {other:?}"),
    }

    let base = tmp_dir("r0004_ac2_render");
    let (sim_dir, authored_dir) = (base.join("simulated"), base.join("authored"));
    let mut sink = SvgSink::new(&sim_dir).expect("sink for the recorded track");
    simulated
        .render(60.0, &mut sink)
        .expect("render the recording");
    let mut sink = SvgSink::new(&authored_dir).expect("sink for the authored track");
    build(authored)
        .render(60.0, &mut sink)
        .expect("render the authored twin");

    let names = dir_names(&sim_dir);
    assert_eq!(names.len(), 60, "1 s at 60 fps");
    assert_eq!(dir_names(&authored_dir), names);
    for name in &names {
        let from_sim = fs::read(sim_dir.join(name)).expect("a simulated frame");
        let from_authored = fs::read(authored_dir.join(name)).expect("an authored frame");
        assert!(!from_sim.is_empty(), "{name} must not be empty");
        assert!(
            from_sim == from_authored,
            "{name}: the renderer must not be able to tell a recorded track \
             from an authored one"
        );
    }
}

// AC2 (SPEC-0004 §2.1's enforced boundary) — within `src/`, `record.rs` is
// the only module that names `garust::physics`; the renderer has no
// simulation-aware code path. Asserted over source text, the way R-0003
// AC5 asserts the dependency set. Examples necessarily build a `World`, so
// the rule scopes to the library.
#[test]
fn ac2_only_record_rs_names_garust_physics() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders = Vec::new();
    let mut record_rs_seen = false;
    for entry in fs::read_dir(&src).expect("read the crate's src directory") {
        let path = entry.expect("directory entry").path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("UTF-8 module name")
            .to_string();
        let text = fs::read_to_string(&path).expect("read a src module");
        let names_physics = text.contains("garust::physics");
        if name == "record.rs" {
            record_rs_seen = true;
            assert!(
                names_physics,
                "record.rs is the module that owns the physics boundary — it \
                 must be the one naming garust::physics"
            );
        } else if names_physics {
            offenders.push(name);
        }
    }
    assert!(
        record_rs_seen,
        "src/record.rs must exist — SPEC-0004 §2.0 pins the module layout"
    );
    assert_eq!(
        offenders,
        Vec::<String>::new(),
        "only src/record.rs may name garust::physics"
    );
}

// --- AC3: the caller's inputs are untouched ------------------------------

// AC3 — recording borrows and copies: every field of every caller body is
// bit-identical afterwards, so the initial state can be recorded again.
#[test]
fn ac3_recording_leaves_the_callers_bodies_untouched() {
    let world = test_world();
    let bodies = [spinner(0.0), spinner(1.0)];
    let joints: Vec<Joint> = Vec::new();
    let before = bodies_bits(&bodies);
    let world_before = world;

    record(&world, &bodies, &joints, 1.0 / 240.0, 240).expect("a valid rollout");

    assert_eq!(
        bodies_bits(&bodies),
        before,
        "the caller's bodies must be untouched by recording"
    );
    assert_eq!(
        world_before, world,
        "the world's settings are untouched too"
    );
    assert!(joints.is_empty(), "the joints slice is untouched too");
}

// AC3 — recording twice from the same initial state reproduces the same
// rollout: the second recording agrees with the first at every derived key
// time, for every body.
#[test]
fn ac3_re_recording_from_the_same_state_reproduces_the_rollout() {
    let world = test_world();
    let bodies = [spinner(0.0), spinner(1.0)];
    let joints: Vec<Joint> = Vec::new();
    let (dt, steps) = (1.0 / 240.0, 240);

    let first = record(&world, &bodies, &joints, dt, steps).expect("a valid rollout");
    let second = record(&world, &bodies, &joints, dt, steps).expect("a valid rollout");
    assert_eq!(
        rollout_bits(&first, dt, steps),
        rollout_bits(&second, dt, steps),
        "the caller's state was not consumed — the rollout repeats"
    );
}

// --- AC4: determinism ----------------------------------------------------

// AC4 — two recordings in one process are bit-identical (`to_bits` over all
// 16 coefficients of `eval` at every derived key time — the observable form
// of "every key motor", SPEC-0004 §6) and the frames rendered from them are
// byte-identical. Same machine, same process: R-0004 §4 declines the
// cross-platform claim for a trig-heavy rollout.
#[test]
fn ac4_two_recordings_are_bit_identical_and_render_identical_frames() {
    let world = test_world();
    let bodies = [spinner(0.0), spinner(1.0)];
    let joints: Vec<Joint> = Vec::new();
    let (dt, steps) = (1.0 / 240.0, 480);

    let first = record(&world, &bodies, &joints, dt, steps).expect("a valid rollout");
    let second = record(&world, &bodies, &joints, dt, steps).expect("a valid rollout");
    assert_eq!(
        rollout_bits(&first, dt, steps),
        rollout_bits(&second, dt, steps),
        "two recordings in one process must be bit-identical"
    );

    let build = |tracks: Vec<Track>| {
        let mut scene = Scene::new(2.0);
        scene.camera = Camera::pinhole(Motor3::translator(0.0, 0.0, 12.0), 2.0);
        for track in tracks {
            scene.add(Object::edges(box_edges(0.4, 0.25, 0.15)).with_track(track));
        }
        scene
    };
    let base = tmp_dir("r0004_ac4_frames");
    let (a, b) = (base.join("a"), base.join("b"));
    let mut sink = SvgSink::new(&a).expect("sink a");
    build(first).render(60.0, &mut sink).expect("render a");
    let mut sink = SvgSink::new(&b).expect("sink b");
    build(second).render(60.0, &mut sink).expect("render b");

    let names = dir_names(&a);
    assert_eq!(names.len(), 120, "2 s at 60 fps");
    assert_eq!(dir_names(&b), names);
    for name in &names {
        let frame_a = fs::read(a.join(name)).expect("frame in a/");
        let frame_b = fs::read(b.join(name)).expect("frame in b/");
        assert!(!frame_a.is_empty(), "{name} must not be empty");
        assert!(frame_a == frame_b, "{name} differs between the two renders");
    }
}

// --- AC5: `Shape::Edges` in the shape vocabulary -------------------------

// AC5 — a `Shape::Edges` object emits exactly one `Prim2::Edges` carrying
// every segment in authored order, with the style passed through verbatim;
// the 1:1 object→primitive invariant is preserved (R-0002), so a twelve-edge
// box is one primitive and one scene entry, not twelve.
#[test]
fn ac5_edges_emit_one_styled_primitive_with_every_segment() {
    let style = Style {
        stroke: Rgb {
            r: 0x12,
            g: 0x34,
            b: 0x56,
        },
        width: 0.125,
        alpha: 0.75,
    };
    // Dyadic half-extents at view depth 4 with focal 2: image = x/2, y/2,
    // exact — no tolerance needed.
    let scene = wireframe_scene(box_edges(0.5, 0.25, 0.125), style);
    let prims = scene.eval(0.0);
    assert_eq!(prims.len(), 1, "one object, one primitive — never twelve");
    match &prims[0] {
        Prim2::Edges { segments, style: s } => {
            assert_eq!(segments.len(), 12, "all twelve edges ride one primitive");
            assert_eq!(*s, style, "the style is carried through unchanged");
            // The first edge runs from (−0.5, −0.25, −0.125) to
            // (0.5, −0.25, −0.125); at view depth 4.125 and 3.875 the
            // pinhole divide is exact for these dyadic values.
            let (a, b) = segments[0];
            assert_eq!(
                (a.x, a.y),
                (-0.5 * 2.0 / 4.125, -0.25 * 2.0 / 4.125),
                "the first endpoint projects where the pinhole rule says"
            );
            assert_eq!((b.x, b.y), (0.5 * 2.0 / 4.125, -0.25 * 2.0 / 4.125));
        }
        other => panic!("Shape::Edges must emit Prim2::Edges, got {other:?}"),
    }

    // An empty edge list is still one primitive carrying no segments — the
    // precedent an empty `Polyline` already set (SPEC-0004 §2.3): no
    // length-dependent branch, no broken 1:1 invariant.
    let empty = wireframe_scene(Vec::new(), style);
    assert_eq!(
        empty.eval(0.0),
        vec![Prim2::Edges {
            segments: Vec::new(),
            style,
        }],
        "an empty edge list emits an empty primitive, not nothing"
    );
}

// AC5 (R-0002 §4's whole-primitive cull rule) — one failing endpoint of one
// edge drops the entire wireframe. A partially drawn solid would
// misrepresent the geometry exactly as a partially drawn polyline would, so
// there is no clipping, no dropped edge, no substituted vertex.
#[test]
fn ac5_one_bad_endpoint_culls_the_whole_wireframe() {
    let style = Style::default();
    let good = box_edges(0.5, 0.25, 0.125);

    // Control: every endpoint visible, every segment emitted.
    match wireframe_scene(good.clone(), style).eval(0.0).as_slice() {
        [Prim2::Edges { segments, .. }] => assert_eq!(segments.len(), 12),
        other => panic!("the all-visible control must survive intact: {other:?}"),
    }

    let bad_endpoints = [
        pga::Point::new(0.0, 0.0, 6.0),      // behind the camera (depth −2)
        pga::Point::new(0.0, 0.0, 4.0),      // exactly on the camera plane
        ideal_point(1.0, 0.0, 0.0),          // weight 0: non-finite readback
        pga::Point::new(f64::NAN, 0.0, 0.0), // a NaN coordinate
    ];
    // The bad endpoint takes each role in turn: first/last edge, `a` or
    // `b` — no position in the list may be treated specially.
    for bad in bad_endpoints {
        for at in [0usize, 5, 11] {
            for slot in [0usize, 1] {
                let mut edges = good.clone();
                if slot == 0 {
                    edges[at].0 = bad;
                } else {
                    edges[at].1 = bad;
                }
                assert_eq!(
                    wireframe_scene(edges, style).eval(0.0),
                    Vec::new(),
                    "one bad endpoint (edge {at}, slot {slot}) must drop the \
                     whole wireframe"
                );
            }
        }
    }

    // Culled means absent, not substituted: draw order among the survivors
    // is untouched.
    let mut scene = Scene::new(1.0);
    scene.camera = Camera::pinhole(Motor3::translator(0.0, 0.0, 4.0), 2.0);
    scene.add(Object::point(pga::Point::new(0.0, 0.0, 0.0)));
    scene.add(Object::edges(vec![(
        pga::Point::new(0.0, 0.0, 0.0),
        pga::Point::new(0.0, 0.0, 6.0),
    )]));
    scene.add(Object::point(pga::Point::new(1.0, 0.0, 0.0)));
    assert_eq!(
        scene.eval(0.0),
        vec![
            Prim2::Point {
                at: Pt2 { x: 0.0, y: 0.0 },
                style: Style::default(),
            },
            Prim2::Point {
                at: Pt2 { x: 0.5, y: 0.0 },
                style: Style::default(),
            },
        ],
        "the culled wireframe is absent; the survivors keep insertion order"
    );
}

// --- AC6: the SVG grammar gains one pinned template ----------------------

/// SPEC-0004 §2.3's pinned edge template inside SPEC-0003 §2.5's default
/// header/footer: one `<path>` element per primitive, `M x,y L x,y` pairs
/// separated by a single space with a comma between coordinates (the
/// `polyline` convention), fixed attribute order, style attributes always
/// emitted, and **no `stroke-linejoin`** — `M`/`L` subpairs are disjoint
/// segments with no joins, so the attribute would be inert. An empty edge
/// list emits `d=""`, the precedent an empty `Polyline` → `points=""`
/// already set.
const EDGES_TEMPLATE_DOC: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8"?>"#,
    "\n",
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="1920" height="1080" viewBox="-1.6 -0.9 3.2 1.8">"#,
    "\n",
    r#"<g transform="scale(1 -1)">"#,
    "\n",
    r##"<path d="M -1.5,0.75 L 0.5,-0.25 M 0,0 L 1,0.5" fill="none" stroke="#abcdef" stroke-width="0.25" stroke-opacity="1" stroke-linecap="round"/>"##,
    "\n",
    r##"<path d="M 0.3,0.5 L -1,-0.5" fill="none" stroke="#ff0011" stroke-width="0.03125" stroke-opacity="0.25" stroke-linecap="round"/>"##,
    "\n",
    r##"<path d="" fill="none" stroke="#000000" stroke-width="0.1" stroke-opacity="0.5" stroke-linecap="round"/>"##,
    "\n",
    "</g>\n</svg>\n",
);

// AC6 — the edge primitive's template matches its pinned bytes: multi-pair,
// single-pair, and empty edge lists, in slice order, through
// shortest-roundtrip `f64` `Display`, byte-for-byte.
#[test]
fn ac6_edge_template_matches_its_pinned_bytes() {
    let dir = tmp_dir("r0004_edge_template");
    let mut sink = SvgSink::new(&dir).expect("sink construction");
    let prims = [
        Prim2::Edges {
            segments: vec![
                (Pt2 { x: -1.5, y: 0.75 }, Pt2 { x: 0.5, y: -0.25 }),
                (Pt2 { x: 0.0, y: 0.0 }, Pt2 { x: 1.0, y: 0.5 }),
            ],
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
        Prim2::Edges {
            segments: vec![(Pt2 { x: 0.3, y: 0.5 }, Pt2 { x: -1.0, y: -0.5 })],
            style: Style {
                stroke: Rgb {
                    r: 0xff,
                    g: 0x00,
                    b: 0x11,
                },
                width: 0.03125,
                alpha: 0.25,
            },
        },
        Prim2::Edges {
            segments: Vec::new(),
            style: Style {
                stroke: Rgb { r: 0, g: 0, b: 0 },
                width: 0.1,
                alpha: 0.5,
            },
        },
    ];
    sink.frame(0, &prims).expect("write the template frame");
    let doc = read_frame(&dir, 0);
    assert_eq!(doc, EDGES_TEMPLATE_DOC);
    assert!(
        !doc.contains("stroke-linejoin"),
        "disjoint M/L subpairs have no joins — the attribute stays absent"
    );
}

/// R-0003's checked-in golden fixture, byte-for-byte.
const R0003_GOLDEN: &[u8] = include_bytes!("golden/frame_00000.svg");

/// SPEC-0003 §3's trig-free golden scene, reproduced here so this
/// requirement can prove its own amendment additive: the same three
/// objects, the same styles, the same `Camera::default()` that
/// `Scene::new` installs. R-0003's own test file is untouched.
fn r0003_golden_scene() -> Scene {
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
        width: 0.06,
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

// AC6 — frames containing no edge primitive are byte-unchanged: R-0003's
// golden scene still renders to the checked-in fixture byte-for-byte after
// the grammar gains its fourth template, and the fixture itself carries no
// `<path>` element. This is the cheapest possible proof that the amendment
// is additive; R-0003's own golden test stays untouched and green in the
// same suite run.
#[test]
fn ac6_r0003_golden_fixture_still_passes_untouched() {
    assert!(
        !String::from_utf8_lossy(R0003_GOLDEN).contains("<path"),
        "R-0003's fixture contains no edge primitive — its bytes cannot move"
    );
    let dir = tmp_dir("r0004_r0003_golden");
    let mut sink = SvgSink::new(&dir).expect("sink construction");
    r0003_golden_scene()
        .render(1.0, &mut sink)
        .expect("golden render");
    assert_eq!(dir_names(&dir), ["frame_00000.svg"]);
    let got = fs::read(dir.join("frame_00000.svg")).expect("read the rendered frame");
    assert!(
        got == R0003_GOLDEN,
        "adding Prim2::Edges moved R-0003's golden bytes\n\
         --- rendered ---\n{}--- fixture ---\n{}",
        String::from_utf8_lossy(&got),
        String::from_utf8_lossy(R0003_GOLDEN)
    );
}

// --- AC7: first simulated light ------------------------------------------

/// The demo's wireframe object — the scene entry whose shape is a
/// `Shape::Edges`, found by shape so the test does not pin the demo's
/// insertion order.
fn tumbling_box_object(scene: &Scene) -> &Object {
    scene
        .objects
        .iter()
        .find(|o| matches!(o.shape, Shape::Edges(_)))
        .expect("the tumbling-box demo draws its solid as a wireframe")
}

// AC7 — the demo renders 4 s at 60 fps to exactly 240 frames
// (frame_00000.svg … frame_00239.svg, nothing else) and rendering it twice
// in the same process is bit-identical on every corresponding pair
// (SPEC-0004 §2.5: same-machine determinism; the rollout is trig-heavy, so
// R-0004 §4 declines the cross-platform claim).
#[test]
fn ac7_tumbling_box_240_frames_twice_bit_identical() {
    let base = tmp_dir("r0004_tumbling_box");
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

    // First *light*: the solid is actually on screen. Every sampled frame
    // carries the box as one twelve-edge wireframe primitive — nothing is
    // culled away, and no frame is an empty document. (Sampled, not
    // asserted to be the frame's only primitive: nothing in R-0004 forbids
    // the demo from drawing something alongside the box.)
    let demo = scene::scene();
    for index in [0usize, 41, 123, 205, 239] {
        let prims = demo.eval(index as f64 / 60.0);
        let wireframes: Vec<usize> = prims
            .iter()
            .filter_map(|p| match p {
                Prim2::Edges { segments, .. } => Some(segments.len()),
                _ => None,
            })
            .collect();
        assert_eq!(
            wireframes,
            [12],
            "frame {index}: the box must be visible as one twelve-edge \
             wireframe, got {prims:?}"
        );
    }
    assert_eq!(demo.duration, 4.0, "the demo is 4 seconds long");
}

// AC7 (SPEC-0004 §6) — at least two visible flips, asserted from the
// recorded track's poses rather than from pixels or from simulation state
// the recorder discards: the body `+y` axis transformed by each key pose
// swings between ±1, and the intermediate-axis instability reverses the
// sign of its world y-component once per flip. SPEC-0004 §2.4's measured
// crossings are t ≈ 0.6875 / 2.0542 / 3.425 s; the demo's parameters are
// pinned there (ε = 0.5 rad/s about the min-moment axis, dt = 1/240,
// 960 steps), and with ε dropped to zero the body never flips at all — so
// the timings are asserted, not merely the count.
#[test]
fn ac7_tumbling_box_flips_end_over_end_at_the_measured_times() {
    let demo = scene::scene();
    let track = &tumbling_box_object(&demo).track;

    // SPEC-0004 §2.4's rollout: 960 steps of dt = 1/240 over the demo's 4 s.
    let (dt, steps) = (1.0 / 240.0, 960);
    let body_up = pga::Point::new(0.0, 1.0, 0.0);
    let world_y = |t: f64| body_up.transform(&track.eval(t)).to_euclidean().1;

    let mut crossings = Vec::new();
    let mut previous = world_y(0.0);
    assert!(
        previous.abs() > 0.9,
        "the flip signal starts at full amplitude, got {previous}"
    );
    for t in key_times(dt, steps).into_iter().skip(1) {
        let y = world_y(t);
        if y != 0.0 && y.signum() != previous.signum() {
            crossings.push(t);
        }
        previous = y;
    }

    assert!(
        crossings.len() >= 2,
        "AC7 needs at least two visible flips, saw {}: {crossings:?}",
        crossings.len()
    );
    let expected = [0.6875, 2.054_166_666_666_666_7, 3.425];
    assert_eq!(
        crossings.len(),
        expected.len(),
        "SPEC-0004 §2.4 measured three flips, saw {crossings:?}"
    );
    for (got, want) in crossings.iter().zip(expected.iter()) {
        assert!(
            (got - want).abs() < 0.05,
            "a flip landed at {got} s, SPEC-0004 §2.4 measured {want} s — \
             the demo's parameters have drifted from the measured ones"
        );
    }
    // The signal really is large-amplitude: it reaches full deflection on
    // both sides, so these are flips and not numerical dithering near zero.
    let extremes = key_times(dt, steps)
        .into_iter()
        .map(world_y)
        .fold((0.0f64, 0.0f64), |(lo, hi), y| (lo.min(y), hi.max(y)));
    assert!(
        extremes.0 < -0.9 && extremes.1 > 0.9,
        "the body's +y axis must swing between ±1, saw {extremes:?}"
    );
}

// --- SPEC-0004 §6: `RecordError` -----------------------------------------

// §6 — `dt` outside its contract is `InvalidTimestep`, returned before the
// world is stepped at all (and before any allocation the caller could
// observe).
#[test]
fn record_error_invalid_timestep_for_zero_negative_nan_and_infinite_dt() {
    let world = test_world();
    let bodies = [spinner(0.0)];
    let joints: Vec<Joint> = Vec::new();
    let before = bodies_bits(&bodies);
    for dt in [0.0, -1.0, -0.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            record(&world, &bodies, &joints, dt, 10)
                .expect_err("dt outside its contract must be rejected"),
            RecordError::InvalidTimestep,
            "dt = {dt} must be rejected"
        );
        assert_eq!(
            bodies_bits(&bodies),
            before,
            "dt = {dt}: nothing was stepped"
        );
    }
}

// §6 — `steps = 0` is valid and yields single-key tracks: a hold at the
// initial pose, consistent with `Track::hold` (SPEC-0004 §2.1).
#[test]
fn record_error_zero_steps_yields_single_key_holds() {
    let world = test_world();
    let bodies = [spinner(0.0), spinner(1.0)];
    let joints: Vec<Joint> = Vec::new();

    let tracks = record(&world, &bodies, &joints, 1.0 / 60.0, 0).expect("steps = 0 is valid");
    assert_eq!(tracks.len(), 2, "one track per body, even with no steps");
    for (b, track) in tracks.iter().enumerate() {
        let want = motor_bits(&bodies[b].rigid.pose().renormalize());
        for t in [-1.0e6, -1.0, 0.0, 1.0e-9, 1.0, 1.0e6] {
            assert_eq!(
                motor_bits(&track.eval(t)),
                want,
                "body {b}: a single-key track holds the initial pose at t = {t}"
            );
        }
    }
}

// §6 — an empty `bodies` slice yields an empty `Vec`: there is nothing to
// record and nothing to fail, at any step count.
#[test]
fn record_error_empty_bodies_yields_an_empty_vec() {
    let world = test_world();
    let empty: [Body; 0] = [];
    let joints: Vec<Joint> = Vec::new();
    for steps in [0usize, 1, 100] {
        let tracks = record(&world, &empty, &joints, 1.0 / 60.0, steps)
            .expect("an empty body slice is not an error");
        assert!(tracks.is_empty(), "steps = {steps}: nothing to record");
    }
}

// §6 (SPEC-0004 §2.1) — the `Track(TrackError)` arm is reachable, not
// decorative: `dt = 1e308, steps = 2` overflows the derived time
// `2 · 1e308` to infinity, so the key chain is rejected by its own
// constructor and the error surfaces through `?` rather than a panic or an
// unwrap.
#[test]
fn record_error_track_arm_is_reachable_without_a_panic() {
    let world = test_world();
    let bodies = [spinner(0.0)];
    let joints: Vec<Joint> = Vec::new();
    assert_eq!(
        record(&world, &bodies, &joints, 1e308, 2)
            .expect_err("an absurd dt cannot produce a valid key chain"),
        RecordError::Track(TrackError::NonIncreasing { index: 2 }),
        "an absurd dt must surface the track error, not panic"
    );
}

// §6 (constitution §6, SPEC-0004 §2.1) — `RecordError` is a typed error
// with `Display`, `std::error::Error`, a `source()` that exposes the inner
// `TrackError` for the `Track` arm, and `From<TrackError>` so the
// constructor's error propagates with `?`. The demo's `main` puts it in a
// `Box<dyn Error>`.
#[test]
fn record_error_display_error_source_and_from() {
    let invalid = RecordError::InvalidTimestep;
    let wrapped = RecordError::Track(TrackError::NonIncreasing { index: 2 });

    assert_eq!(
        RecordError::from(TrackError::NonIncreasing { index: 2 }),
        wrapped,
        "From<TrackError> wraps into the Track arm"
    );

    for err in [invalid, wrapped] {
        let shown = err.to_string();
        assert!(!shown.is_empty(), "{err:?} must Display as something");
        assert!(
            !shown.contains("RecordError"),
            "{err:?}: Display is prose, not the Debug spelling — got {shown:?}"
        );
    }
    assert_ne!(
        invalid.to_string(),
        wrapped.to_string(),
        "the two arms must not read alike"
    );

    let dynamic: &dyn std::error::Error = &wrapped;
    let source = dynamic
        .source()
        .expect("the Track arm exposes its inner TrackError as its source");
    assert_eq!(
        source.to_string(),
        TrackError::NonIncreasing { index: 2 }.to_string(),
        "source() is the inner TrackError"
    );
    let dynamic: &dyn std::error::Error = &invalid;
    assert!(
        dynamic.source().is_none(),
        "InvalidTimestep wraps nothing, so it has no source"
    );

    // The demo's `main` shape: both arms box into `dyn Error`.
    fn boxed(err: RecordError) -> Box<dyn std::error::Error> {
        Box::new(err)
    }
    assert!(!boxed(invalid).to_string().is_empty());
    assert!(!boxed(wrapped).to_string().is_empty());
}

// SPEC-0004 §2.1 — `record` takes the `joints` slice `World::step`
// requires even though R-0004 ships no joint demo, so R-0005 needs no
// signature break. Callers with no joints pass `&[]`; the parameter's type
// is pinned here by passing a `Vec<Joint>` explicitly.
#[test]
fn record_takes_a_joint_slice_it_does_not_need() {
    let world = test_world();
    let bodies = [spinner(0.0)];
    let (dt, steps) = (1.0 / 120.0, 12);
    let joints: Vec<Joint> = Vec::new();
    let from_vec = record(&world, &bodies, &joints, dt, steps).expect("a valid rollout");
    let from_literal = record(&world, &bodies, &[], dt, steps).expect("a valid rollout");
    assert_eq!(
        rollout_bits(&from_vec, dt, steps),
        rollout_bits(&from_literal, dt, steps),
        "`&[]` and an empty Vec<Joint> are the same input"
    );
}

// --- AC5 (property): the never-non-finite invariant ----------------------

prop_compose! {
    /// A rigid motion: `translator(dx, dy, dz) · rotor(angle, plane)`.
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

/// A hostile coordinate: near the camera or out to ±1e9 (R-0002's).
fn arb_hostile_coord() -> impl Strategy<Value = f64> {
    prop_oneof![-8.0f64..8.0, -1.0e9f64..1.0e9]
}

/// A hostile endpoint: a finite point anywhere, or an ideal point (weight
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
    // AC5 (never-non-finite) — under hostile wireframes (endpoints to ±1e9,
    // at/behind-plane points, ideal points, empty and long edge lists,
    // focal across nine decades, both projection models, arbitrary t) a
    // `Shape::Edges` object either vanishes entirely or emits a primitive
    // in which every f64 is finite. The invariant R-0002 §2.2 established
    // holds for the new shape exactly as for the others.
    #[test]
    fn ac5_edges_never_leak_a_non_finite_coordinate(
        camera_pose in arb_motor(4.0, TAU / 4.0),
        projection in arb_hostile_projection(),
        objects in prop::collection::vec(
            (
                prop::collection::vec(
                    (arb_hostile_vertex(), arb_hostile_vertex()),
                    0..=6,
                ),
                arb_motor(2.0, TAU / 4.0),
                arb_style(),
            ),
            1..=3,
        ),
        t in -1.0e3f64..1.0e3,
    ) {
        let mut scene = Scene::new(1.0);
        scene.camera = Camera { pose: camera_pose, projection };
        for (edges, pose, style) in objects {
            scene.add(Object::edges(edges).with_style(style).at(pose));
        }
        let prims = scene.eval(t);
        for prim in &prims {
            prop_assert!(
                matches!(prim, Prim2::Edges { .. }),
                "an Edges object must emit an Edges primitive or nothing"
            );
        }
        for value in every_f64(&prims) {
            prop_assert!(
                value.is_finite(),
                "a non-finite f64 left Scene::eval: {}",
                value
            );
        }
    }
}
