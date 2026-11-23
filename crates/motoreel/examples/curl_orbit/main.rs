//! The fbf toy curls two dumbbells, freezes mid-rep, and the camera orbits
//! 150° around it while a «Rayos X» sweep crosses the frame and back: the
//! house's reveal on the production API (R-0017, R-0018, R-0019).
//!
//! ```bash
//! cargo run --release -p motoreel --example curl_orbit
//! ```
//!
//! Writes `out/curl_orbit/frames/frame_%05d.ppm` (1080 × 1920, 30 fps,
//! 150 frames) and encodes `out/curl_orbit/curl_orbit.mp4` with ffmpeg
//! (BT.709, faststart, bitexact), then prints the per-frame time; a second
//! run reproduces the md5.
//!
//! Every part is a mesh object posed by a motor track; the camera is a
//! motor per frame; the depth order, the culling and the finish are
//! motoreel's. Colours are the example's (`palette.rs`): the engine has no
//! palette.

mod encode;
mod kit;
mod orbit;
mod palette;
mod rig;
mod set;
mod studio;
mod timeline;

use motoreel::Scene;

use crate::studio::Framing;

/// Close on the toy: 4.3 m back, 8° down.
const FRAMING: Framing = Framing {
    target: [0.02, 0.93, 0.0],
    elevation: 8.0,
    distance: 4.3,
    focal: 2700.0,
};

/// The scene: platform, floor marks, figure. The floor marks are ordinary
/// paths, so the platform and the figure are two depth groups.
fn scene() -> Scene {
    let mut scene = Scene::new(timeline::DURATION);
    scene.view = set::VIEW;
    for object in set::platform()
        .into_iter()
        .chain(set::floor_marks())
        .chain(rig::figure())
    {
        scene.add(object);
    }
    scene
}

fn main() -> std::io::Result<()> {
    orbit::run("curl_orbit", &scene(), &FRAMING)
}
