//! What the gym orbit shows and from where: the scene and its framing.
//! `main.rs` renders it; `benches/orbit.rs` times it (R-0022).

use motoreel::Scene;

use crate::studio::Framing;
use crate::{props, rig, set, timeline};

/// Wide enough for the rack and the machine on either side of the toy.
pub const FRAMING: Framing = Framing {
    target: [-0.7, 1.05, 0.0],
    elevation: 10.0,
    distance: 10.5,
    focal: 2500.0,
};

/// The platform and the props are one depth group, so the object order
/// sorts them against each other; the floor marks end it, and the toy is
/// the second group, never behind a prop by the layout.
pub fn scene() -> Scene {
    let mut scene = Scene::new(timeline::DURATION);
    scene.view = set::VIEW;
    for object in set::platform()
        .into_iter()
        .chain(props::gym())
        .chain(set::floor_marks())
        .chain(rig::figure())
    {
        scene.add(object);
    }
    scene
}
