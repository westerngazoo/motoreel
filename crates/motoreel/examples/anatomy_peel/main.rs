//! The toy's anatomy (R-0021): it curls while the camera orbits and the
//! layers come off. The skin opens in facets and fades, head first; the
//! muscles show and the biceps lights up in the load colour; a sweep turns
//! the muscles to glass and shows the bones; the forearm bones light up
//! and the forearm is drawn as a lever, held at 90°.
//!
//! ```bash
//! cargo run --release -p motoreel --example anatomy_peel
//! ```
//!
//! Writes `out/anatomy_peel/frames/frame_%05d.ppm` (1080 × 1920, 30 fps,
//! 270 frames) and `out/anatomy_peel/anatomy_peel.mp4`, and prints the
//! per-frame time. The layers are `assets/anatomy/` (built by
//! `tools/blender/anatomy.py`); the looks, the camera, the set and the
//! render loop are `curl_orbit`'s, read in place.

mod body;
#[path = "../curl_orbit/encode.rs"]
mod encode;
// Read in place for `set.rs`'s platform; the toy's generators go unused.
#[allow(dead_code)]
#[path = "../curl_orbit/kit.rs"]
mod kit;
// Read in place; this example binds only part of the palette.
#[allow(dead_code)]
#[path = "../curl_orbit/palette.rs"]
mod palette;
mod scene;
#[path = "../curl_orbit/set.rs"]
mod set;
#[path = "../curl_orbit/studio.rs"]
mod studio;
mod timeline;

fn main() -> std::io::Result<()> {
    let body = body::Body::load();
    studio::run("anatomy_peel", timeline::DURATION, |t| {
        scene::frame(&body, t)
    })
}
