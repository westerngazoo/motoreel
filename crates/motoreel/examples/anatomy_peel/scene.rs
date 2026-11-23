//! What the anatomy peel shows at each instant and from where: the scene
//! built for the moment, and its primitives under the moment's look.
//! `main.rs` renders it; `benches/anatomy.rs` times both halves (R-0022).

use motoreel::{Finish, Look, Prim2, Pt2, Scene, Sweep};

use crate::body::Body;
use crate::palette::CIAN;
use crate::set;
use crate::studio::{self, Framing};
use crate::timeline::{self, State};

/// On the toy, a little wider than the curl's close-up.
pub const FRAMING: Framing = Framing {
    target: [0.02, 0.95, 0.0],
    elevation: 8.0,
    distance: 4.6,
    focal: 2700.0,
};

/// The scene at `t` (state `s`): the set, the peeled body and the lever.
pub fn scene(body: &Body, t: f64, s: &State) -> Scene {
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
    scene
}

/// Every primitive of `scene` at `t`, under the look of `s`.
pub fn paint(scene: &Scene, t: f64, s: &State) -> Vec<Prim2> {
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

/// Every primitive at `t`.
pub fn frame(body: &Body, t: f64) -> Vec<Prim2> {
    let s = timeline::at(t);
    paint(&scene(body, t, &s), t, &s)
}
