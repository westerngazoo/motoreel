//! Render the tumbling box — motoreel's first *simulated* animation — to
//! `out/tumbling/` as 240 SVG frames (60 fps × 4 s), then encode outside
//! the crate:
//!
//! ```bash
//! cargo run --example tumbling_box
//! ffmpeg -framerate 60 -i out/tumbling/frame_%05d.svg -pix_fmt yuv420p tumbling_box.mp4
//! ```
//!
//! Nothing here keyframes the motion: the poses come from stepping a
//! garust-physics world at a fixed timestep and recording each pose as a
//! motor. The renderer cannot tell the difference.

use motoreel::SvgSink;

mod scene;

fn main() -> std::io::Result<()> {
    let mut sink = SvgSink::new("out/tumbling/")?;
    scene::scene().render(60.0, &mut sink)?;
    println!("240 frames in out/tumbling/ — see the header for the ffmpeg line.");
    Ok(())
}
