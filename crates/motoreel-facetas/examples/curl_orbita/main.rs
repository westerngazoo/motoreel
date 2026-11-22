//! SPIKE sample: the fbf toy curls two dumbbells, freezes mid-rep, and the
//! camera orbits 150° around it. The same clip in each finish:
//!
//! ```bash
//! cargo run --release -p motoreel-facetas --example curl_orbita            # all three
//! cargo run --release -p motoreel-facetas --example curl_orbita -- pintura # one
//! ```
//!
//! Writes `out/curl_orbita/<finish>/frame_%05d.ppm`, then
//! `out/curl_orbita/<finish>.mp4` and, when all three ran,
//! `out/curl_orbita/comparacion.mp4` (side by side). 1080 × 1920, 30 fps.

mod encode;
mod palette;
mod rig;
mod set;
mod timeline;

use std::io;
use std::path::{Path, PathBuf};
use std::time::Instant;

use motoreel::{FrameSink, PpmSink, Prim2, Pt2};
use motoreel_facetas::acabado::{fill_only, polygon};
use motoreel_facetas::{
    faces, paint, Acabado, Barrido, Brush, Facetas, Light, Orbit, Pintura, RayosX,
};

use crate::palette::{BLANCO, CIAN, MARINO, TINTA};
use crate::timeline::Xray;

const FPS: u32 = 30;
const SIZE: (u32, u32) = (1080, 1920);

/// Which finish a clip is painted in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Look {
    Facetas,
    Pintura,
    RayosX,
}

impl Look {
    const ALL: [Look; 3] = [Look::Facetas, Look::Pintura, Look::RayosX];

    fn name(self) -> &'static str {
        match self {
            Look::Facetas => "facetas",
            Look::Pintura => "pintura",
            Look::RayosX => "rayos-x",
        }
    }

    fn parse(s: &str) -> Option<Look> {
        Look::ALL.into_iter().find(|l| l.name() == s)
    }
}

const FACETAS: Facetas = Facetas {
    ink: TINTA,
    seam: 1.0,
};
const PINTURA: Pintura = Pintura {
    seed: 0x00F1_5BF0_C0DE,
    brushes: [
        Brush {
            width: 0.055,
            length: 0.11,
            density: 1.3,
            tone_jitter: 0.14,
            alpha: 0.9,
            refine: false,
        },
        Brush {
            width: 0.024,
            length: 0.055,
            density: 1.2,
            tone_jitter: 0.22,
            alpha: 0.85,
            refine: true,
        },
    ],
    seam: 1.0,
};
const RAYOS_X: RayosX = RayosX {
    glass: CIAN,
    solid: FACETAS,
};

/// Every primitive of the frame at `t` in `look`.
fn frame(look: Look, t: f64) -> Vec<Prim2> {
    let s = timeline::at(t);
    let orbit = Orbit {
        target: [0.02, 0.93, 0.0],
        distance: 4.3,
        azimuth: s.azimuth,
        elevation: 8f64.to_radians(),
        focal: 2700.0,
    };
    let camera = orbit.camera();
    let Some(light) = Light::new([0.55, 0.8, 0.45], 0.3) else {
        return Vec::new();
    };
    let sweep;
    let acabado: &dyn Acabado = match (look, s.xray) {
        (Look::Facetas, _) | (Look::RayosX, Xray::Off) => &FACETAS,
        (Look::Pintura, _) => &PINTURA,
        (Look::RayosX, Xray::On) => &RAYOS_X,
        (Look::RayosX, Xray::Sweep(line)) => {
            sweep = Barrido {
                left: &RAYOS_X,
                right: &FACETAS,
                line,
            };
            &sweep
        }
    };
    let platform = set::platform();
    let figure = rig::figure(s.elbow);
    let mut out = set::backdrop(&camera);
    out.extend(paint(&faces(&platform, &camera, &light), acabado));
    out.extend(set::floor_marks(&camera));
    out.extend(paint(&faces(&figure, &camera, &light), acabado));
    if let (Look::RayosX, Xray::Sweep(x)) = (look, s.xray) {
        let (top, bottom) = (set::VIEW.1 / 2.0, -set::VIEW.1 / 2.0);
        for (w, a) in [(26.0, 0.18), (5.0, 1.0)] {
            let line = [
                Pt2 {
                    x: x - w / 2.0,
                    y: bottom,
                },
                Pt2 {
                    x: x + w / 2.0,
                    y: bottom,
                },
                Pt2 {
                    x: x + w / 2.0,
                    y: top,
                },
                Pt2 {
                    x: x - w / 2.0,
                    y: top,
                },
            ];
            out.extend(polygon(&line, fill_only(CIAN, a)));
        }
    }
    if s.flash > 0.0 {
        let (hw, hh) = (set::VIEW.0 / 2.0, set::VIEW.1 / 2.0);
        let all = [
            Pt2 { x: -hw, y: -hh },
            Pt2 { x: hw, y: -hh },
            Pt2 { x: hw, y: hh },
            Pt2 { x: -hw, y: hh },
        ];
        out.extend(polygon(&all, fill_only(BLANCO, s.flash)));
    }
    out
}

/// Renders `look` into `dir` on every core; returns the summed per-frame
/// render time (seconds), sink writes included.
fn render(look: Look, dir: &Path) -> io::Result<f64> {
    let frames = (timeline::DURATION * f64::from(FPS)).ceil() as usize;
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|k| {
                scope.spawn(move || -> io::Result<f64> {
                    let mut sink =
                        PpmSink::with_view(dir, SIZE, set::VIEW)?.with_background(MARINO);
                    let mut busy = 0.0;
                    for i in (k..frames).step_by(threads) {
                        let start = Instant::now();
                        let t = i as f64 / f64::from(FPS); // derived, never accumulated
                        sink.frame(i, &frame(look, t))?;
                        busy += start.elapsed().as_secs_f64();
                    }
                    Ok(busy)
                })
            })
            .collect();
        workers.into_iter().try_fold(0.0, |sum, w| {
            Ok(sum
                + w.join()
                    .map_err(|_| io::Error::other("render thread panicked"))??)
        })
    })
    .map(|busy: f64| busy / frames as f64)
}

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let looks: Vec<Look> = if args.is_empty() {
        Look::ALL.to_vec()
    } else {
        args.iter()
            .map(|a| {
                Look::parse(a).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("unknown finish {a:?}: facetas | pintura | rayos-x"),
                    )
                })
            })
            .collect::<io::Result<_>>()?
    };
    let out = PathBuf::from("out/curl_orbita");
    let mut clips = Vec::new();
    for look in looks {
        let dir = out.join(look.name());
        let wall = Instant::now();
        let per_frame = render(look, &dir)?;
        println!(
            "{}: {:.0} ms/frame (one core), {:.1} s wall",
            look.name(),
            per_frame * 1e3,
            wall.elapsed().as_secs_f64()
        );
        let mp4 = out.join(format!("{}.mp4", look.name()));
        encode::clip(&dir, FPS, &mp4)?;
        println!("  -> {}", mp4.display());
        clips.push(mp4);
    }
    if clips.len() == Look::ALL.len() {
        let refs: Vec<&Path> = clips.iter().map(PathBuf::as_path).collect();
        let cmp = out.join("comparacion.mp4");
        encode::side_by_side(&refs, &cmp)?;
        println!("  -> {}", cmp.display());
    }
    Ok(())
}
