//! motoreel — motor-native mathematical animation on garust.
//!
//! Scenes are described as objects with motor tracks — keyframed, or
//! recorded from a garust-physics rollout — and rendered offline to
//! deterministic numbered frames. See `docs/RFC-012-garust-anim.md` and
//! `ROADMAP.md`.
//!
//! Code lands only through the requirement loop (`CLAUDE.md` §4). R-0001
//! (`Track` + `Ease`) and R-0002 (scene, camera, projection) are
//! implemented; the SVG sink follows as R-0003.

mod camera;
mod ease;
mod object;
mod prim;
mod scene;
mod track;

pub use camera::{Camera, Projection};
pub use ease::Ease;
pub use object::{Object, Shape};
pub use prim::{Prim2, Pt2, Rgb, Style};
pub use scene::{ObjectId, Scene};
pub use track::{Track, TrackError};
