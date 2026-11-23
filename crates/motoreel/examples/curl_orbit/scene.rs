//! What the curl orbit shows and from where: the scene and its framing.
//! `main.rs` renders it; `benches/orbit.rs` times it (R-0022), so the
//! reel and its benchmark cannot drift apart.

use motoreel::Scene;

use crate::studio::Framing;
use crate::{rig, set, timeline};

/// Close on the toy: 4.3 m back, 8° down.
pub const FRAMING: Framing = Framing {
    target: [0.02, 0.93, 0.0],
    elevation: 8.0,
    distance: 4.3,
    focal: 2700.0,
};

/// The scene: platform, floor marks, figure. The floor marks are ordinary
/// paths, so the platform and the figure are two depth groups.
pub fn scene() -> Scene {
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
