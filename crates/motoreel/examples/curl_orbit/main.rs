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
//! (BT.709, faststart, bitexact), then prints the per-frame time and the
//! md5 a second run must reproduce.
//!
//! Every part is a mesh object posed by a motor track; the camera is a
//! motor per frame; the depth order, the culling and the finish are
//! motoreel's. Colours are the example's (`palette.rs`): the engine has no
//! palette.

mod encode;
mod kit;
mod palette;
mod rig;
mod set;
mod timeline;

use std::io;
use std::path::{Path, PathBuf};
use std::time::Instant;

use garust::{Motor3, Pga3};
use motoreel::{
    Camera, Faceted, Finish, FrameSink, Glass, Glow, Light, Look, PpmSink, Prim2, Pt2, Scene,
    Sweep, XRay,
};

use crate::palette::{BLANCO, CIAN, MARINO};
use crate::timeline::Xray;

const FPS: u32 = 30;
const SIZE: (u32, u32) = (1080, 1920);

/// The key light, fixed in the world (it stays put while the camera
/// orbits, which is what makes the orbit read as 3D).
const LIGHT: [f64; 3] = [0.55, 0.8, 0.45];

/// Three bands for three-tone ramps; a 2 px self-stroke closes every
/// seam, oblique ones included (R-0018 AC7).
fn faceted() -> Faceted {
    Faceted::new(3, 2.0).unwrap_or_else(|e| panic!("valid: {e}"))
}

/// The spike's x-ray look, now without stacking glows.
fn xray() -> XRay {
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

/// The orbit camera: the target, a turn about +y, a tilt down, a dolly
/// back along the camera's +z. A product of motors, nothing else.
fn camera(azimuth: f64) -> Camera {
    let (elevation, distance, focal) = (8f64.to_radians(), 4.3, 2700.0);
    let turn_y = Motor3::rotor(-azimuth, Pga3::basis(0b0101));
    let tilt = Motor3::rotor(-elevation, Pga3::basis(0b0110));
    let pose = Motor3::translator(0.02, 0.93, 0.0)
        .compose(&turn_y)
        .compose(&tilt)
        .compose(&Motor3::translator(0.0, 0.0, distance));
    Camera::pinhole(pose, focal)
}

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

/// Every primitive of the frame at `t`. `scene` is this thread's copy;
/// only its camera changes.
fn frame(scene: &mut Scene, t: f64) -> Vec<Prim2> {
    let s = timeline::at(t);
    scene.camera = camera(s.azimuth);
    let (facetas, rayos) = (faceted(), xray());
    let sweep;
    let finish: &dyn Finish = match s.xray {
        Xray::Off => &facetas,
        Xray::On => &rayos,
        Xray::Sweep(x) => {
            sweep = Sweep::new(&rayos, &facetas, Pt2 { x, y: 0.0 }, Pt2 { x: 1.0, y: 0.0 })
                .unwrap_or_else(|e| panic!("valid: {e}"));
            &sweep
        }
    };
    let light =
        Light::new(scene.camera.view_direction(LIGHT)).unwrap_or_else(|e| panic!("valid: {e}"));
    let look = Look {
        finish,
        light,
        seed: 0,
    };
    let mut out = set::backdrop(&scene.camera);
    out.extend(scene.eval_with(t, &look));
    if let Xray::Sweep(x) = s.xray {
        out.extend(set::bar(x, 26.0, CIAN, 0.18));
        out.extend(set::bar(x, 5.0, CIAN, 1.0));
    }
    if s.flash > 0.0 {
        out.extend(set::whole(BLANCO, s.flash));
    }
    out
}

/// Renders every frame into `dir` on every core; returns the mean
/// single-core time per frame (seconds), sink writes included.
fn render(dir: &Path) -> io::Result<f64> {
    let base = scene();
    let frames = (timeline::DURATION * f64::from(FPS)).ceil() as usize;
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let busy = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|k| {
                let mut scene = base.clone();
                scope.spawn(move || -> io::Result<f64> {
                    let mut sink =
                        PpmSink::with_view(dir, SIZE, set::VIEW)?.with_background(MARINO);
                    let mut busy = 0.0;
                    for i in (k..frames).step_by(threads) {
                        let start = Instant::now();
                        let t = i as f64 / f64::from(FPS); // derived, never accumulated
                        sink.frame(i, &frame(&mut scene, t))?;
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

fn main() -> io::Result<()> {
    let out = PathBuf::from("out/curl_orbit");
    let frames = out.join("frames");
    if frames.exists() {
        std::fs::remove_dir_all(&frames)?; // no stale frames for ffmpeg
    }
    let wall = Instant::now();
    let per_frame = render(&frames)?;
    println!(
        "curl_orbit: {:.0} ms/frame (one core), {:.1} s wall",
        per_frame * 1e3,
        wall.elapsed().as_secs_f64()
    );
    let mp4 = out.join("curl_orbit.mp4");
    encode::clip(&frames, FPS, &mp4)?;
    println!("  -> {}", mp4.display());
    Ok(())
}
