//! First light: render the RFC-012 §3.5 screw demo to `out/` as 240 SVG
//! frames (60 fps × 4 s), then encode outside the crate:
//!
//! ```bash
//! cargo run --example first_light
//! ffmpeg -framerate 60 -i out/frame_%05d.svg -pix_fmt yuv420p first_light.mp4
//! ```
//!
//! SVG demuxing needs an ffmpeg built with librsvg; otherwise
//! pre-rasterize (`rsvg-convert`/Inkscape) and encode the PNGs. Encode
//! from a clean directory: a shorter re-render leaves stale
//! higher-numbered frames behind.

use motoreel::SvgSink;

mod scene;

fn main() -> std::io::Result<()> {
    let mut sink = SvgSink::new("out/")?;
    scene::scene().render(60.0, &mut sink)?;
    println!("240 frames in out/ — see the header for the ffmpeg line.");
    Ok(())
}
