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
mod scene;
mod set;
mod studio;
mod timeline;

fn main() -> std::io::Result<()> {
    orbit::run("curl_orbit", &scene::scene(), &scene::FRAMING)
}
