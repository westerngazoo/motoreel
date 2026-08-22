//! motoreel — motor-native mathematical animation on garust.
//!
//! Scenes are described as objects with motor tracks — keyframed, or
//! recorded from a garust-physics rollout — and rendered offline to
//! deterministic numbered frames. See `docs/RFC-012-garust-anim.md` and
//! `ROADMAP.md`.
//!
//! Code lands only through the requirement loop (`CLAUDE.md` §4). R-0001
//! (`Track` + `Ease`) is implemented; scenes and sinks follow as R-0002
//! and R-0003.

mod ease;
mod track;

pub use ease::Ease;
pub use track::{Track, TrackError};
