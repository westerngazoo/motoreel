//! Interchangeable finishes («acabados», R-0017, SPEC-0017): what a surface
//! is, kept apart from how it is painted.
//!
//! A caller describes **surfaces** (an image-space outline, an orientation
//! to the light, a view depth, a stable id and a [`Material`]) and hands a
//! whole batch to a [`Finish`], which returns ordinary [`Prim2`]s. Swapping
//! the finish swaps the look and nothing else. This is the separation
//! RenderMan's shading language made standard (R. L. Cook, "Shade Trees",
//! SIGGRAPH '84) and glTF 2.0 keeps between meshes and materials.
//!
//! Std-only and pure: no garust type crosses this module, like [`crate::prim`].

use crate::path::{Seg, Subpath};
use crate::prim::{Prim2, Pt2, Rgb};

/// The most tones a material's ramp may hold (R-0017 AC2).
pub const MAX_TONES: usize = 8;

/// What a surface is made of, under every finish (R-0017 AC7, SPEC-0017
/// §2.2).
///
/// The tag lives on the material, not on the surface or its id: it says
/// what the stuff *is* (skin, bone), which is a property of the material a
/// face is made of, as glTF keeps `alphaMode` on the material. Finishes
/// read it to decide what to show.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Layer {
    /// Opaque in every finish: props, the set, clothes. The default.
    #[default]
    Solid,
    /// An exterior that an x-ray finish turns translucent («piel»). Opaque
    /// under [`crate::Faceted`].
    Skin,
    /// What is inside a skin (bone, a muscle line): hidden by
    /// [`crate::Faceted`], shown by an x-ray finish («por dentro»).
    Inner,
}

/// A material's seam: a stroke of its own colour and width around each
/// surface (rf-marca's `faceta_con_junta`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seam {
    /// The seam's colour.
    pub colour: Rgb,
    /// The seam's width, in image units.
    pub width: f64,
}

/// A validated material (R-0017 AC2): a tone ramp from shadow to light, an
/// opacity, an optional seam and a [`Layer`].
///
/// Unused ramp slots repeat the last tone, so the derived `PartialEq`
/// compares meaning, not leftovers (as `Gradient` does with its stops).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Material {
    tones: [Rgb; MAX_TONES],
    len: usize,
    opacity: f64,
    seam: Option<Seam>,
    layer: Layer,
}

/// Why a [`Material`] could not be built. Each variant names its field.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MaterialError {
    /// `tones`: the ramp is empty.
    NoTones,
    /// `tones`: more than [`MAX_TONES`]; carries the count.
    TooManyTones(usize),
    /// `opacity`: outside `[0, 1]` or non-finite; carries the value.
    Opacity(f64),
    /// `seam.width`: negative or non-finite; carries the value.
    SeamWidth(f64),
}

impl std::fmt::Display for MaterialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MaterialError::NoTones => write!(f, "tones: the ramp is empty"),
            MaterialError::TooManyTones(n) => {
                write!(f, "tones: {n} tones, at most {MAX_TONES}")
            }
            MaterialError::Opacity(a) => write!(f, "opacity: {a} is not in [0, 1]"),
            MaterialError::SeamWidth(w) => write!(f, "seam.width: {w} is not finite and >= 0"),
        }
    }
}

impl std::error::Error for MaterialError {}

impl Material {
    /// An opaque [`Layer::Solid`] material with no seam, its ramp `tones`
    /// running from shadow (first) to light (last).
    ///
    /// # Errors
    /// [`MaterialError::NoTones`] or [`MaterialError::TooManyTones`].
    pub fn new(tones: &[Rgb]) -> Result<Material, MaterialError> {
        let last = *tones.last().ok_or(MaterialError::NoTones)?;
        if tones.len() > MAX_TONES {
            return Err(MaterialError::TooManyTones(tones.len()));
        }
        let mut ramp = [last; MAX_TONES];
        ramp[..tones.len()].copy_from_slice(tones);
        Ok(Material {
            tones: ramp,
            len: tones.len(),
            opacity: 1.0,
            seam: None,
            layer: Layer::Solid,
        })
    }

    /// This material at `opacity` (builder).
    ///
    /// # Errors
    /// [`MaterialError::Opacity`] outside `[0, 1]` or non-finite.
    pub fn with_opacity(self, opacity: f64) -> Result<Material, MaterialError> {
        if !(0.0..=1.0).contains(&opacity) {
            return Err(MaterialError::Opacity(opacity));
        }
        Ok(Material { opacity, ..self })
    }

    /// This material with a seam (builder).
    ///
    /// # Errors
    /// [`MaterialError::SeamWidth`] for a negative or non-finite width.
    pub fn with_seam(self, colour: Rgb, width: f64) -> Result<Material, MaterialError> {
        if !(width.is_finite() && width >= 0.0) {
            return Err(MaterialError::SeamWidth(width));
        }
        Ok(Material {
            seam: Some(Seam { colour, width }),
            ..self
        })
    }

    /// This material in `layer` (builder; infallible).
    #[must_use]
    pub fn with_layer(self, layer: Layer) -> Material {
        Material { layer, ..self }
    }

    /// The ramp, shadow first.
    pub fn tones(&self) -> &[Rgb] {
        &self.tones[..self.len]
    }

    /// The opacity, in `[0, 1]`.
    #[must_use]
    pub fn opacity(&self) -> f64 {
        self.opacity
    }

    /// The seam, if any.
    #[must_use]
    pub fn seam(&self) -> Option<Seam> {
        self.seam
    }

    /// The layer.
    #[must_use]
    pub fn layer(&self) -> Layer {
        self.layer
    }
}

/// How a surface is turned to the light (R-0017 AC1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Facing {
    /// A view-space normal `(x, y, z)`, not necessarily unit: finishes
    /// normalise it themselves, in rf-marca's operation order (SPEC-0017
    /// §2.1 records why it is not normalised here).
    Normal([f64; 3]),
    /// A flat 2D piece in the image plane, facing the viewer: the normal
    /// `(0, 0, 1)`.
    Viewer,
}

impl Facing {
    /// The view-space normal this facing stands for.
    #[must_use]
    pub fn normal(self) -> [f64; 3] {
        match self {
            Facing::Normal(n) => n,
            Facing::Viewer => [0.0, 0.0, 1.0],
        }
    }
}

/// One surface («superficie»): plain, validated data (R-0017 AC1).
#[derive(Clone, Debug, PartialEq)]
pub struct Surface {
    outline: Vec<Subpath<Pt2>>,
    facing: Facing,
    depth: f64,
    id: u64,
    material: Material,
    frame: Option<[Pt2; 3]>,
}

/// Why a [`Surface`] could not be built. Each variant names its field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceError {
    /// `outline`: no subpath, a subpath with no segment, or a non-finite
    /// coordinate.
    Outline,
    /// `facing`: a non-finite or zero normal.
    Facing,
    /// `depth`: non-finite.
    Depth,
    /// `frame`: a non-finite point.
    Frame,
}

impl std::fmt::Display for SurfaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let what = match self {
            SurfaceError::Outline => "outline: empty, or a non-finite coordinate",
            SurfaceError::Facing => "facing: a zero or non-finite normal",
            SurfaceError::Depth => "depth: non-finite",
            SurfaceError::Frame => "frame: a non-finite point",
        };
        f.write_str(what)
    }
}

impl std::error::Error for SurfaceError {}

impl Surface {
    /// A surface with no frame.
    ///
    /// `id` is the caller's, stable across frames for the same piece; it
    /// seeds anything a finish varies per surface.
    ///
    /// # Errors
    /// A [`SurfaceError`] naming the first invalid field: an empty
    /// outline or one with a non-finite coordinate, a zero or non-finite
    /// normal, a non-finite depth.
    pub fn new(
        outline: Vec<Subpath<Pt2>>,
        facing: Facing,
        depth: f64,
        id: u64,
        material: Material,
    ) -> Result<Surface, SurfaceError> {
        let segments_ok = |s: &Subpath<Pt2>| !s.segs.is_empty() && points(s).all(finite);
        if outline.is_empty() || !outline.iter().all(segments_ok) {
            return Err(SurfaceError::Outline);
        }
        let [x, y, z] = facing.normal();
        if !(x.is_finite() && y.is_finite() && z.is_finite()) || (x, y, z) == (0.0, 0.0, 0.0) {
            return Err(SurfaceError::Facing);
        }
        if !depth.is_finite() {
            return Err(SurfaceError::Depth);
        }
        Ok(Surface {
            outline,
            facing,
            depth,
            id,
            material,
            frame: None,
        })
    }

    /// This surface with an image-space frame that rides the piece
    /// (R-0016's three points: origin, `x_end`, `y_end`). A finish that
    /// places marks on the surface anchors them here.
    ///
    /// # Errors
    /// [`SurfaceError::Frame`] for a non-finite point.
    pub fn with_frame(self, frame: [Pt2; 3]) -> Result<Surface, SurfaceError> {
        if !frame.iter().copied().all(finite) {
            return Err(SurfaceError::Frame);
        }
        Ok(Surface {
            frame: Some(frame),
            ..self
        })
    }

    /// The image-space outline.
    pub fn outline(&self) -> &[Subpath<Pt2>] {
        &self.outline
    }

    /// The orientation to the light.
    #[must_use]
    pub fn facing(&self) -> Facing {
        self.facing
    }

    /// The view depth (larger is farther).
    #[must_use]
    pub fn depth(&self) -> f64 {
        self.depth
    }

    /// The stable id.
    #[must_use]
    pub fn id(&self) -> u64 {
        self.id
    }

    /// The material.
    #[must_use]
    pub fn material(&self) -> &Material {
        &self.material
    }

    /// The frame, if any.
    #[must_use]
    pub fn frame(&self) -> Option<[Pt2; 3]> {
        self.frame
    }

    /// The image point at frame coordinates `(u, v)`:
    /// `origin + u·(x_end − origin) + v·(y_end − origin)`. `None` without
    /// a frame.
    #[must_use]
    pub fn frame_point(&self, u: f64, v: f64) -> Option<Pt2> {
        let [o, x, y] = self.frame?;
        Some(Pt2 {
            x: o.x + u * (x.x - o.x) + v * (y.x - o.x),
            y: o.y + u * (x.y - o.y) + v * (y.y - o.y),
        })
    }
}

/// A directional light in view space (R-0017 AC3), stored verbatim: the
/// house candle `[-0.6, -0.6, 0.52]` round-trips bit for bit. Finishes
/// normalise it where they need to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    toward: [f64; 3],
}

/// Why a [`Light`] could not be built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightError {
    /// `toward`: a zero or non-finite direction.
    Direction,
}

impl std::fmt::Display for LightError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("toward: a zero or non-finite direction")
    }
}

impl std::error::Error for LightError {}

impl Light {
    /// A light shining from direction `toward` (view space, any length).
    ///
    /// # Errors
    /// [`LightError::Direction`] for a zero or non-finite vector.
    pub fn new(toward: [f64; 3]) -> Result<Light, LightError> {
        let [x, y, z] = toward;
        if !(x.is_finite() && y.is_finite() && z.is_finite()) || toward == [0.0; 3] {
            return Err(LightError::Direction);
        }
        Ok(Light { toward })
    }

    /// The direction toward the light, as given.
    #[must_use]
    pub fn toward(&self) -> [f64; 3] {
        self.toward
    }
}

/// What a finish may know about the frame it paints (R-0017 AC4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameCtx {
    /// The frame time, seconds.
    pub t: f64,
    /// A seed for anything a finish varies, combined with surface ids.
    pub seed: u64,
}

/// A finish («acabado»): a batch of surfaces in, primitives out (R-0017 AC4).
///
/// **Contract.** The output keeps the batch's order: a finish may emit
/// zero or several primitives per surface, or one primitive for a run of
/// consecutive surfaces, but it never draws a later surface under an
/// earlier one. It is a pure function of its inputs: no interior
/// mutability, no global state, no clock, nothing that depends on the
/// thread. `Send + Sync` lets one finish serve every render thread.
///
/// It takes the whole batch, not one surface at a time, because the
/// faceted look quantises tones by population over the whole figure.
pub trait Finish: Send + Sync {
    /// Paint `batch`, far to near, under `light`.
    fn paint(&self, batch: &[Surface], light: &Light, ctx: &FrameCtx) -> Vec<Prim2>;
}

/// Every point of `s`, handles included.
fn points(s: &Subpath<Pt2>) -> impl Iterator<Item = Pt2> + '_ {
    std::iter::once(s.start).chain(s.segs.iter().flat_map(|seg| match *seg {
        Seg::Line(p) => vec![p],
        Seg::Cubic(a, b, p) => vec![a, b, p],
    }))
}

fn finite(p: Pt2) -> bool {
    p.x.is_finite() && p.y.is_finite()
}
