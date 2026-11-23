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
//! The creator pipeline (MC) adds R-0006 (`PpmSink`), R-0007 (anchored
//! labels), R-0009 (real type), R-0012 (paths and fills: [`path`],
//! [`shapes`], [`Fill`]), R-0016 (gradient fills: [`gradient`],
//! [`Paint`]), R-0017 (interchangeable finishes: [`Surface`], [`Finish`],
//! [`Faceted`]), R-0018 (faceted 3D: [`Mesh`], [`Scene::eval_with`]) and
//! R-0019 (the x-ray finish and its sweep: [`XRay`], [`Sweep`]) and R-0020
//! (meshes loaded from OBJ: [`Obj`]) and R-0021 (layers and the peel:
//! [`Peel`], [`Material::mix`]).

mod camera;
mod ease;
mod faceted;
mod finish;
pub mod gradient;
pub mod label;
mod mesh;
pub mod obj;
mod object;
pub mod path;
mod peel;
pub mod ppm;
mod prim;
mod record;
mod scene;
pub mod shapes;
pub mod sink;
mod solid;
#[cfg(test)]
mod solid_reference;
pub mod svg;
mod track;
mod xray;

pub use camera::{Camera, Projection};
pub use ease::Ease;
pub use faceted::{lambert, quantize, Faceted, FacetedError};
pub use finish::{
    Facing, Finish, FrameCtx, Layer, Light, LightError, Material, MaterialError, Seam, Surface,
    SurfaceError, MAX_TONES,
};
pub use gradient::{Gradient, GradientError, GradientKind, Stop};
pub use label::{Anchor, Label, ScreenAnchor};
pub use mesh::{Mesh, MeshError, MeshFace, PLANAR_TOL};
pub use obj::{Axes, Obj, ObjError, ObjErrorKind};
pub use object::{Object, Shape};
pub use path::{Seg, Subpath};
pub use peel::{Peel, PeelError};
pub use ppm::PpmSink;
pub use prim::{Align, Fill, Paint, Prim2, Pt2, Rgb, Style};
pub use record::{record, RecordError};
pub use scene::{Look, ObjectId, Scene};
pub use sink::FrameSink;
pub use svg::SvgSink;
pub use track::{Track, TrackError};
pub use xray::{Glass, Glow, Sweep, SweepError, XRay, XRayError};
