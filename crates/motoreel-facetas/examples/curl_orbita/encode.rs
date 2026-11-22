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

/// The clips side by side, each scaled to 720 × 1280.
pub fn side_by_side(clips: &[&Path], mp4: &Path) -> io::Result<()> {
    let mut args: Vec<String> = Vec::new();
    for c in clips {
        args.extend(["-i".into(), c.display().to_string()]);
    }
    let scaled: String = (0..clips.len())
        .map(|i| format!("[{i}:v]scale=720:1280[v{i}];"))
        .collect();
    let inputs: String = (0..clips.len()).map(|i| format!("[v{i}]")).collect();
    args.extend([
        "-filter_complex".into(),
        format!("{scaled}{inputs}hstack=inputs={}", clips.len()),
    ]);
    args.extend(OUT.iter().map(|s| s.to_string()));
    args.push(mp4.display().to_string());
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    ffmpeg(&refs)
}
