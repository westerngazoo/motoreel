//! The tumbling box: a freely rotating rigid body with three distinct
//! principal moments, spun about its **intermediate** axis. Such a body
//! flips end-over-end periodically — the intermediate axis theorem, seen
//! in orbit as the Dzhanibekov effect — which is hard to believe from the
//! equations and obvious in motion.
//!
//! Shared verbatim with `tests/r0004_physics_playback.rs` via `#[path]`,
//! so the shipped demo and its determinism test cannot drift apart.
//! motoreel items are named through `motoreel::`, garust's through
//! `garust::`.

use garust::physics::world::{Body, World};
use garust::physics::{Inertia, RigidBody};
use garust::{pga, Motor3};
use motoreel::{record, Camera, Object, Rgb, Scene, Style};

/// Box half-extents: a flat, elongated slab, so all three principal
/// moments differ and the intermediate axis is unmistakable.
const HALF: [f64; 3] = [0.8, 0.5, 0.1];
/// Physics timestep — four steps per rendered frame at 60 fps.
const DT: f64 = 1.0 / 240.0;
/// 4 seconds of simulation.
const STEPS: usize = 960;

/// Build the tumbling-box scene from a recorded rollout.
pub fn scene() -> Scene {
    let mass = 1.0;
    let [hx, hy, hz] = HALF;
    let (dx, dy, dz) = (2.0 * hx, 2.0 * hy, 2.0 * hz);

    // Cuboid moments (m/12)(d² + d²) — the formulas are normative, so no
    // inertia constant is hand-written.
    let inertia = Inertia::principal([
        mass * (dy * dy + dz * dz) / 12.0,
        mass * (dx * dx + dz * dz) / 12.0,
        mass * (dx * dx + dy * dy) / 12.0,
    ]);

    // Spin about the intermediate (y) axis with an explicit 2% nudge on x.
    // The perturbation must be explicit: at exactly zero, this integrator
    // never flips — roundoff does not seed the instability.
    let p = Inertia::principal_planes();
    let omega = p[0] * 0.5 + p[1] * 10.0;

    let mut body = Body::ball(mass, (hx * hx + hy * hy + hz * hz).sqrt());
    body.inertia = inertia;
    body.rigid = RigidBody::spinning(Motor3::identity(), &inertia, omega);

    // Free rotation: no gravity, no ground, no contacts.
    let world = World {
        gravity: [0.0, 0.0, 0.0],
        ground: None,
    };
    let mut tracks =
        record(&world, &[body], &[], DT, STEPS).expect("static demo rollout parameters are valid");

    let box_edges = Object::edges(box_wireframe(hx, hy, hz))
        .with_style(Style {
            stroke: Rgb {
                r: 0xe0,
                g: 0xe0,
                b: 0xe0,
            },
            width: 0.012,
            alpha: 1.0,
        })
        .with_track(tracks.remove(0));

    // The angular momentum stays fixed near world +y for the whole
    // rollout, so the camera looks perpendicular to it — along it, the
    // flip would foreshorten into near-invisibility.
    let mut scene = Scene::new(STEPS as f64 * DT);
    scene.camera = Camera::pinhole(Motor3::translator(0.0, 0.0, 3.5), 2.5);
    scene.add(box_edges);
    scene
}

/// The twelve edges of a box centred on the origin.
fn box_wireframe(hx: f64, hy: f64, hz: f64) -> Vec<(pga::Point, pga::Point)> {
    let corner = |sx: f64, sy: f64, sz: f64| pga::Point::new(sx * hx, sy * hy, sz * hz);
    let mut edges = Vec::with_capacity(12);
    for &a in &[-1.0, 1.0] {
        for &b in &[-1.0, 1.0] {
            edges.push((corner(-1.0, a, b), corner(1.0, a, b))); // along x
            edges.push((corner(a, -1.0, b), corner(a, 1.0, b))); // along y
            edges.push((corner(a, b, -1.0), corner(a, b, 1.0))); // along z
        }
    }
    edges
}
