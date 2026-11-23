//! The curl orbit in a gym: the toy curls, freezes, and the camera orbits
//! while the x-ray sweeps in and out, with a power rack and a cable
//! machine modelled in Blender and loaded from OBJ (R-0020) beside him.
//! Under the x-ray the cable machine's shroud turns to glass and its
//! weight stack shows: the materials' names tag them (`skin_shroud`,
//! `inner_stack`).
//!
//! ```bash
//! cargo run --release -p motoreel --example gym_orbit
//! ```
//!
//! Writes `out/gym_orbit/frames/frame_%05d.ppm` (1080 × 1920, 30 fps, 150
//! frames) and `out/gym_orbit/gym_orbit.mp4`, and prints the per-frame
//! time. The toy, the set, the timeline and the render loop are
//! `curl_orbit`'s, read in place, so the two examples cannot drift apart.

#[path = "../curl_orbit/encode.rs"]
mod encode;
#[path = "../curl_orbit/kit.rs"]
mod kit;
#[path = "../curl_orbit/orbit.rs"]
mod orbit;
#[path = "../curl_orbit/palette.rs"]
mod palette;
mod props;
#[path = "../curl_orbit/rig.rs"]
mod rig;
mod scene;
#[path = "../curl_orbit/set.rs"]
mod set;
#[path = "../curl_orbit/studio.rs"]
mod studio;
#[path = "../curl_orbit/timeline.rs"]
mod timeline;

fn main() -> std::io::Result<()> {
    orbit::run("gym_orbit", &scene::scene(), &scene::FRAMING)
}
