//! First light: render the RFC-012 §3.5 screw demo twice — 240 SVG frames
//! in `out/` and the same 240 as binary P6 in `out/ppm/` (60 fps × 4 s) —
//! then encode outside the crate.
//!
//! **Use the PPM line.** It works on a stock ffmpeg, with nothing else
//! installed:
//!
//! ```bash
//! cargo run --example first_light
//! ffmpeg -framerate 60 -i out/ppm/frame_%05d.ppm -c:v libx264 -pix_fmt yuv420p first_light.mp4
//! ```
//!
//! The SVG line below is kept because SVG is still the right output for
//! print, for hand-editing, and for anything vector-bound:
//!
//! ```bash
//! ffmpeg -framerate 60 -i out/frame_%05d.svg -pix_fmt yuv420p first_light.mp4
//! ```
//!
//! — but it only runs on an ffmpeg built with librsvg, which most builds
//! are not: ffmpeg ships an `svg_pipe` demuxer and no SVG *decoder*, so it
//! recognises the files and then cannot decode them. Without librsvg,
//! pre-rasterize (`rsvg-convert`/Inkscape) and encode the PNGs — or just
//! use the PPM line, which is why R-0006 added it.
//!
//! Encode from a clean directory: a shorter re-render leaves stale
//! higher-numbered frames behind, and ffmpeg will happily include them.

use motoreel::{PpmSink, SvgSink};

mod scene;

fn main() -> std::io::Result<()> {
    let mut svg = SvgSink::new("out/")?;
    scene::scene().render(60.0, &mut svg)?;

    let mut ppm = PpmSink::new("out/ppm/")?;
    scene::scene().render(60.0, &mut ppm)?;

    println!("240 SVG frames in out/ and 240 P6 frames in out/ppm/ —");
    println!("see the header for the ffmpeg line (use the PPM one).");
    Ok(())
}
