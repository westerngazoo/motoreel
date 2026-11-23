//! The toy's anatomy (R-0021): it curls while the camera orbits and the
//! layers come off. The skin opens in facets and fades, head first; the
//! muscles show and the biceps lights up in the load colour; a sweep turns
//! the muscles to glass and shows the bones; the forearm bones light up
//! and the forearm is drawn as a lever, held at 90°.
//!
//! ```bash
//! cargo run --release -p motoreel --example anatomy_peel
//! ```
//!
//! Writes `out/anatomy_peel/frames/frame_%05d.ppm` (1080 × 1920, 30 fps,
//! 270 frames) and `out/anatomy_peel/anatomy_peel.mp4`, and prints the
//! per-frame time. The layers are `assets/anatomy/` (built by
//! `tools/blender/anatomy.py`); the looks, the camera, the set and the
//! render loop are `curl_orbit`'s, read in place.

mod body;
#[path = "../curl_orbit/encode.rs"]
mod encode;
// Read in place for `set.rs`'s platform; the toy's generators go unused.
#[allow(dead_code)]
#[path = "../curl_orbit/kit.rs"]
mod kit;
// Read in place; this example binds only part of the palette.
#[allow(dead_code)]
#[path = "../curl_orbit/palette.rs"]
mod palette;
#[path = "../curl_orbit/set.rs"]
mod set;
#[path = "../curl_orbit/studio.rs"]
mod studio;
mod timeline;

use motoreel::{Finish, Look, Prim2, Pt2, Scene, Sweep};

use crate::body::Body;
use crate::palette::CIAN;
use crate::studio::Framing;

/// On the toy, a little wider than the curl's close-up.
const FRAMING: Framing = Framing {
    target: [0.02, 0.95, 0.0],
    elevation: 8.0,
    distance: 4.6,
    focal: 2700.0,
};

/// Every primitive at `t`.
fn frame(body: &Body, t: f64) -> Vec<Prim2> {
    let s = timeline::at(t);
    let mut scene = Scene::new(timeline::DURATION);
    scene.view = set::VIEW;
    scene.camera = studio::camera(&FRAMING, s.azimuth);
    for object in set::platform()
        .into_iter()
        .chain(set::floor_marks())
        .chain(body.objects(&s.moment))
        .chain(body.lever(t, s.moment.lever))
    {
        scene.add(object);
    }
    let (facetas, rayos) = (studio::faceted(), studio::xray());
    let sweep;
    let finish: &dyn Finish = match s.look {
        timeline::Look::Faceted => &facetas,
        timeline::Look::XRay => &rayos,
        timeline::Look::Sweep(x) => {
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
    if let timeline::Look::Sweep(x) = s.look {
        out.extend(set::bar(x, 26.0, CIAN, 0.18));
        out.extend(set::bar(x, 5.0, CIAN, 1.0));
    }
    out
}

fn main() -> std::io::Result<()> {
    let body = Body::load();
    studio::run("anatomy_peel", timeline::DURATION, |t| frame(&body, t))
}
