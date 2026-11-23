//! R-0022's frame-time bench for the two orbit reels: `curl_orbit` and
//! `gym_orbit`, every frame of their timeline, through the examples' own
//! scene and frame function (read in place, so the reels and the bench
//! cannot drift apart). Everything but the raster sink is timed.
//!
//! ```bash
//! cargo bench -p motoreel --bench orbit
//! MOTOREEL_BENCH_ROUNDS=9 cargo bench -p motoreel --bench orbit
//! ```

mod common;

#[path = "../examples/curl_orbit/encode.rs"]
#[allow(dead_code)]
mod encode;
#[path = "../examples/curl_orbit/kit.rs"]
mod kit;
#[path = "../examples/curl_orbit/orbit.rs"]
#[allow(dead_code)]
mod orbit;
#[path = "../examples/curl_orbit/palette.rs"]
mod palette;
#[path = "../examples/gym_orbit/props.rs"]
mod props;
#[path = "../examples/curl_orbit/rig.rs"]
mod rig;
#[path = "../examples/curl_orbit/set.rs"]
mod set;
#[path = "../examples/curl_orbit/studio.rs"]
#[allow(dead_code)]
mod studio;
#[path = "../examples/curl_orbit/timeline.rs"]
mod timeline;

// The two examples' `scene.rs`, under names that tell them apart.
#[path = "../examples/curl_orbit/scene.rs"]
mod curl;
#[path = "../examples/gym_orbit/scene.rs"]
mod gym;

use motoreel::Scene;

use crate::studio::Framing;
use crate::timeline::Xray;

/// The reels' frame rate (`studio.rs`).
const FPS: f64 = 30.0;

/// Times every frame of one orbit reel and prints its rows.
fn bench(name: &str, scene: &Scene, framing: &Framing, rounds: usize) {
    let count = (timeline::DURATION * FPS).ceil() as usize;
    let t = |i: usize| i as f64 / FPS;
    let phase_of: Vec<&str> = (0..count)
        .map(|i| match timeline::at(t(i)).xray {
            Xray::Off => "facetas",
            Xray::Sweep(_) => "barrido",
            Xray::On => "rayos-x",
        })
        .collect();
    let mut print = common::Fingerprint::new();
    let ms = common::time_frames(
        count,
        rounds,
        |_| (),
        |i, ()| orbit::frame(scene, framing, t(i)),
        |_, prims| print.add(prims),
    );
    common::rows(
        name,
        "frame",
        &["facetas", "barrido", "rayos-x"],
        &phase_of,
        &ms,
    );
    println!("# {name} fingerprint {}", print.hex());
}

fn main() {
    let rounds = common::rounds();
    common::header(rounds);
    if common::selected("curl_orbit") {
        bench("curl_orbit", &curl::scene(), &curl::FRAMING, rounds);
    }
    if common::selected("gym_orbit") {
        bench("gym_orbit", &gym::scene(), &gym::FRAMING, rounds);
    }
    println!("# load at end: {}", common::load());
}
