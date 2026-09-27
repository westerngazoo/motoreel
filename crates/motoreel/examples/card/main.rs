//! The explanatory card: a panel, a friction circle with a point running
//! round its rim, and the shaded area under a curve — R-0012's paths and
//! fills in one portrait clip. Renders 90 P6 frames (30 fps × 3 s) at
//! 1080 × 1920 into `out/card/`, then encode with a stock ffmpeg:
//!
//! ```bash
//! cargo run --release --example card
//! ffmpeg -framerate 30 -i out/card/frame_%05d.ppm -c:v libx264 -pix_fmt yuv420p card.mp4
//! ```
//!
//! Encode from a clean directory: a shorter re-render leaves stale
//! higher-numbered frames behind, and ffmpeg will happily include them.
//!
//! Every colour is the example's, background included: the engine has no
//! palette.

use motoreel::{PpmSink, Rgb};

mod scene;

/// `--ink`: the page behind the card.
const BACKGROUND: Rgb = Rgb {
    r: 0x0a,
    g: 0x0a,
    b: 0x0b,
};

fn main() -> std::io::Result<()> {
    // The view must be the scene's own portrait window (SPEC-0012 §2.12):
    // 600 px per image unit.
    let mut sink =
        PpmSink::with_view("out/card/", (1080, 1920), (1.8, 3.2))?.with_background(BACKGROUND);
    scene::scene().render(30.0, &mut sink)?;
    println!("90 P6 frames in out/card/ — see the header for the ffmpeg line.");
    Ok(())
}
