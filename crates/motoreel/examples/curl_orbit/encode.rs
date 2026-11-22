//! ffmpeg, the way motoreel documents it (numbered P6 frames in, H.264
//! out), with the delivery flags the reels use: BT.709 tags, yuv420p,
//! faststart, and bitexact so the MP4 bytes are reproducible too.

use std::io;
use std::path::Path;
use std::process::Command;

/// Runs ffmpeg with `args`; an error if it is missing or fails.
fn ffmpeg(args: &[&str]) -> io::Result<()> {
    let status = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-y"])
        .args(args)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("ffmpeg exited with {status}")))
    }
}

/// The H.264 output flags shared by every clip.
const OUT: [&str; 22] = [
    "-c:v",
    "libx264",
    "-preset",
    "slow",
    "-crf",
    "16",
    "-pix_fmt",
    "yuv420p",
    "-threads",
    "1",
    "-colorspace",
    "bt709",
    "-color_primaries",
    "bt709",
    "-color_trc",
    "bt709",
    "-movflags",
    "+faststart",
    "-fflags",
    "+bitexact",
    "-flags:v",
    "+bitexact",
];

/// `frames/frame_%05d.ppm` at `fps` into `mp4`.
pub fn clip(frames: &Path, fps: u32, mp4: &Path) -> io::Result<()> {
    let pattern = frames.join("frame_%05d.ppm");
    let fps = fps.to_string();
    let mut args = vec![
        "-framerate",
        &fps,
        "-i",
        pattern.to_str().unwrap_or_default(),
        "-vf",
        "scale=out_color_matrix=bt709",
    ];
    args.extend(OUT);
    args.push(mp4.to_str().unwrap_or_default());
    ffmpeg(&args)
}
