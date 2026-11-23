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
#[path = "../curl_orbit/set.rs"]
mod set;
#[path = "../curl_orbit/studio.rs"]
mod studio;
#[path = "../curl_orbit/timeline.rs"]
mod timeline;

use motoreel::Scene;

use crate::studio::Framing;

/// Wide enough for the rack and the machine on either side of the toy.
const FRAMING: Framing = Framing {
    target: [-0.7, 1.05, 0.0],
    elevation: 10.0,
    distance: 10.5,
    focal: 2500.0,
};

/// The platform and the props are one depth group, so the object order
/// sorts them against each other; the floor marks end it, and the toy is
/// the second group, never behind a prop by the layout.
fn scene() -> Scene {
    let mut scene = Scene::new(timeline::DURATION);
    scene.view = set::VIEW;
    for object in set::platform()
        .into_iter()
        .chain(props::gym())
        .chain(set::floor_marks())
        .chain(rig::figure())
    {
        scene.add(object);
    }
    scene
}

fn main() -> std::io::Result<()> {
    orbit::run("gym_orbit", &scene(), &FRAMING)
}
