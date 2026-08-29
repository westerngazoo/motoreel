//! motoreel — motor-native mathematical animation on garust.
//!
//! Scenes are described as objects with motor tracks — keyframed, or
//! recorded from a garust-physics rollout — and rendered offline to
//! deterministic numbered frames. See `docs/RFC-012-garust-anim.md` and
//! `ROADMAP.md`.
//!
//! Code lands only through the requirement loop (`CLAUDE.md` §4). M1 is
//! complete: R-0001 (`Track` + `Ease`), R-0002 (scene, camera,
//! projection), and R-0003 (`SvgSink` and the render walk). M2 begins
//! with R-0004: recording a physics rollout into ordinary motor tracks.

mod camera;
mod ease;
mod font;
pub mod label;
mod object;
pub mod ppm;
mod prim;
mod record;
mod scene;
pub mod sink;
pub mod svg;
mod track;

pub use camera::{Camera, Projection};
pub use ease::Ease;
pub use label::{Anchor, Label, ScreenAnchor};
pub use object::{Object, Shape};
pub use ppm::PpmSink;
pub use prim::{Align, Prim2, Pt2, Rgb, Style};
pub use record::{record, RecordError};
pub use scene::{ObjectId, Scene};
pub use sink::FrameSink;
pub use svg::SvgSink;
pub use track::{Track, TrackError};
