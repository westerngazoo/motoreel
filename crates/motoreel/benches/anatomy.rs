//! R-0022's frame-time bench for `anatomy_peel`: every frame of its
//! timeline through the example's own scene (read in place), timed in two
//! parts: `build` (the scene for the instant: the peel applied, the layers
//! bound) and `paint` (`Scene::eval_with` and the set: the depth order and
//! the finish). The raster sink is not timed.
//!
//! ```bash
//! cargo bench -p motoreel --bench anatomy
//! ```

mod common;

#[path = "../examples/anatomy_peel/body.rs"]
mod body;
#[path = "../examples/curl_orbit/encode.rs"]
#[allow(dead_code)]
mod encode;
#[path = "../examples/curl_orbit/kit.rs"]
#[allow(dead_code)]
mod kit;
#[path = "../examples/curl_orbit/palette.rs"]
#[allow(dead_code)]
mod palette;
#[path = "../examples/anatomy_peel/scene.rs"]
#[allow(dead_code)]
mod scene;
#[path = "../examples/curl_orbit/set.rs"]
mod set;
#[path = "../examples/curl_orbit/studio.rs"]
#[allow(dead_code)]
mod studio;
#[path = "../examples/anatomy_peel/timeline.rs"]
mod timeline;

use crate::body::Body;
use crate::timeline::Look;

/// The reel's frame rate (`studio.rs`).
const FPS: f64 = 30.0;

/// The phase frame `t` is in: the three layers whole, the skin peeling,
/// the muscle under `Faceted`, the sweep, and the bones under `XRay`.
fn phase(t: f64) -> &'static str {
    let s = timeline::at(t);
    match (s.moment.peel, s.look) {
        (Some(0.0), _) => "facetas",
        (Some(_), _) => "pelado",
        (None, Look::Faceted) => "musculo",
        (None, Look::Sweep(_)) => "barrido",
        (None, Look::XRay) => "rayos-x",
    }
}

fn main() {
    let rounds = common::rounds();
    common::header(rounds);
    let body = Body::load();
    if common::selected("anatomy_peel") {
        reel(&body, rounds);
    }
    if common::selected("toy-orbit") {
        orbit(&body, rounds);
    }
    println!("# load at end: {}", common::load());
}

/// The reel: every frame of its timeline, `build` and `paint` apart.
fn reel(body: &Body, rounds: usize) {
    let count = (timeline::DURATION * FPS).ceil() as usize;
    let t = |i: usize| i as f64 / FPS;
    let phase_of: Vec<&str> = (0..count).map(|i| phase(t(i))).collect();
    let phases = ["facetas", "pelado", "musculo", "barrido", "rayos-x"];

    let build = common::time_frames(
        count,
        rounds,
        |i| timeline::at(t(i)),
        |i, s| scene::scene(body, t(i), s),
        |_, _| {},
    );
    let mut print = common::Fingerprint::new();
    let paint = common::time_frames(
        count,
        rounds,
        |i| {
            let s = timeline::at(t(i));
            (scene::scene(body, t(i), &s), s)
        },
        |i, (built, s)| scene::paint(built, t(i), s),
        |_, prims| print.add(prims),
    );
    common::rows("anatomy_peel", "build", &phases, &phase_of, &build);
    common::rows("anatomy_peel", "paint", &phases, &phase_of, &paint);
    println!("# anatomy_peel fingerprint {}", print.hex());
}

/// The interactive case (the fitAI spike's): the three-layer toy held at
/// one instant while the camera turns a full circle, 5° a frame, under
/// `Faceted`, under `XRay` and half peeled. Only `paint` is timed.
fn orbit(body: &Body, rounds: usize) {
    const HELD: f64 = 1.4;
    const STEPS: usize = 72;
    let configs = [
        ("facetas", Look::Faceted, 0.0),
        ("rayos-x", Look::XRay, 0.0),
        ("pelado-.5", Look::Faceted, 0.5),
    ];
    let mut print = common::Fingerprint::new();
    for (name, look, peel) in configs {
        let state = |i: usize| {
            let mut s = timeline::at(HELD);
            s.azimuth = (i as f64 * 360.0 / STEPS as f64).to_radians();
            s.look = look;
            s.moment.peel = Some(peel);
            s
        };
        let ms = common::time_frames(
            STEPS,
            rounds,
            |i| {
                let s = state(i);
                (scene::scene(body, HELD, &s), s)
            },
            |_, (built, s)| scene::paint(built, HELD, s),
            |_, prims| print.add(prims),
        );
        common::row("toy-orbit", name, "paint", &ms);
    }
    println!("# toy-orbit fingerprint {}", print.hex());
}
