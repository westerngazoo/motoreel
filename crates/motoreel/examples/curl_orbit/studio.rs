//! What the 3D examples share: the two looks, the orbit camera, the key
//! light and the parallel render loop. `curl_orbit` owns it; `gym_orbit`
//! and `anatomy_peel` read it through `#[path]`, so all render one way.

use std::io;
use std::path::{Path, PathBuf};
use std::time::Instant;

use garust::{Motor3, Pga3};
use motoreel::{Camera, Faceted, FrameSink, Glass, Glow, Light, PpmSink, Prim2, XRay};

use crate::encode;
use crate::kit::V3;
use crate::palette::{CIAN, MARINO};
use crate::set;

const FPS: u32 = 30;
const SIZE: (u32, u32) = (1080, 1920);

/// The key light, fixed in the world (it stays put while the camera
/// orbits, which is what makes the orbit read as 3D).
const LIGHT: [f64; 3] = [0.55, 0.8, 0.45];

/// Three bands for three-tone ramps; a 2 px self-stroke closes every
/// seam, oblique ones included (R-0018 AC7).
pub fn faceted() -> Faceted {
    Faceted::new(3, 2.0).unwrap_or_else(|e| panic!("valid: {e}"))
}

/// The spike's x-ray look, now without stacking glows.
pub fn xray() -> XRay {
    XRay::new(
        faceted(),
        Glass {
            tint: CIAN,
            alpha: 0.10,
            edge_width: 1.6,
            edge_alpha: 0.30,
        },
        Glow {
            width: 12.0,
            alpha: 0.16,
        },
    )
    .unwrap_or_else(|e| panic!("valid: {e}"))
}

/// Where the orbit camera looks from: the target, the tilt down
/// (degrees), the distance (m) and the focal length (px).
pub struct Framing {
    pub target: V3,
    pub elevation: f64,
    pub distance: f64,
    pub focal: f64,
}

/// The orbit camera: the target, a turn about +y, a tilt down, a dolly
/// back along the camera's +z. A product of motors, nothing else.
pub fn camera(framing: &Framing, azimuth: f64) -> Camera {
    let [x, y, z] = framing.target;
    let turn_y = Motor3::rotor(-azimuth, Pga3::basis(0b0101));
    let tilt = Motor3::rotor(-framing.elevation.to_radians(), Pga3::basis(0b0110));
    let pose = Motor3::translator(x, y, z)
        .compose(&turn_y)
        .compose(&tilt)
        .compose(&Motor3::translator(0.0, 0.0, framing.distance));
    Camera::pinhole(pose, framing.focal)
}

/// The key light as `camera` sees it.
pub fn light(camera: &Camera) -> Light {
    Light::new(camera.view_direction(LIGHT)).unwrap_or_else(|e| panic!("valid: {e}"))
}

/// Renders `frames` frames of `frame` into `dir` on every core; returns
/// the mean single-core time per frame (seconds), sink writes included.
fn render(
    frames: usize,
    frame: &(impl Fn(f64) -> Vec<Prim2> + Sync),
    dir: &Path,
) -> io::Result<f64> {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let busy = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|k| {
                scope.spawn(move || -> io::Result<f64> {
                    let mut sink =
                        PpmSink::with_view(dir, SIZE, set::VIEW)?.with_background(MARINO);
                    let mut busy = 0.0;
                    for i in (k..frames).step_by(threads) {
                        let start = Instant::now();
                        let t = i as f64 / f64::from(FPS); // derived, never accumulated
                        sink.frame(i, &frame(t))?;
                        busy += start.elapsed().as_secs_f64();
                    }
                    Ok(busy)
                })
            })
            .collect();
        workers.into_iter().try_fold(0.0, |sum, w| {
            Ok::<f64, io::Error>(
                sum + w
                    .join()
                    .map_err(|_| io::Error::other("render thread panicked"))??,
            )
        })
    })?;
    Ok(busy / frames as f64)
}

/// Renders `duration` seconds of `frame` into `out/<name>/frames`,
/// encodes `out/<name>/<name>.mp4` and prints the per-frame time.
pub fn run(name: &str, duration: f64, frame: impl Fn(f64) -> Vec<Prim2> + Sync) -> io::Result<()> {
    let out = PathBuf::from("out").join(name);
    let frames = out.join("frames");
    if frames.exists() {
        std::fs::remove_dir_all(&frames)?; // no stale frames for ffmpeg
    }
    let wall = Instant::now();
    let count = (duration * f64::from(FPS)).ceil() as usize;
    let per_frame = render(count, &frame, &frames)?;
    println!(
        "{name}: {:.0} ms/frame (one core), {:.1} s wall",
        per_frame * 1e3,
        wall.elapsed().as_secs_f64()
    );
    let mp4 = out.join(format!("{name}.mp4"));
    encode::clip(&frames, FPS, &mp4)?;
    println!("  -> {}", mp4.display());
    Ok(())
}
