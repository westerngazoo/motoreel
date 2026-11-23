//! One frame of the curl orbit's timeline (`timeline.rs`) for any scene:
//! the orbit camera, the sweep between the two looks, the sweep bar and
//! the freeze flash. `curl_orbit` and `gym_orbit` share it, and
//! `benches/orbit.rs` times it (R-0022).

use motoreel::{Finish, Look, Prim2, Pt2, Scene, Sweep};

use crate::palette::{BLANCO, CIAN};
use crate::set;
use crate::studio::{self, Framing};
use crate::timeline::{self, Xray};

/// Every primitive of `scene` at `t`, seen from the timeline's azimuth.
pub fn frame(base: &Scene, framing: &Framing, t: f64) -> Vec<Prim2> {
    let s = timeline::at(t);
    let mut scene = base.clone();
    scene.camera = studio::camera(framing, s.azimuth);
    let (facetas, rayos) = (studio::faceted(), studio::xray());
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
    let look = Look {
        finish,
        light: studio::light(&scene.camera),
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

/// Renders the timeline over `scene` as `out/<name>/<name>.mp4`.
pub fn run(name: &str, scene: &Scene, framing: &Framing) -> std::io::Result<()> {
    studio::run(name, timeline::DURATION, |t| frame(scene, framing, t))
}
