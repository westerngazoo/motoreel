//! «Rayos X» and its sweep («barrido») (R-0019, SPEC-0019): the house's
//! reveal, «igual por fuera, distinto por dentro».
//!
//! [`XRay`] turns [`Layer::Skin`] into tinted glass, shows
//! [`Layer::Inner`] parts opaque inside a glow, and paints
//! [`Layer::Solid`] props exactly as [`Faceted`] does, so a dumbbell looks
//! the same on both sides of the sweep. [`Sweep`] splits the image along a
//! moving line between two finishes.
//!
//! **No accumulation.** A run of consecutive surfaces of one material
//! paints its skin, or its glow, as **one** primitive whose subpaths are
//! the run's outlines. A sink composites a primitive's fill and its stroke
//! once per pixel (the nonzero union, and the stroke's union by max), so
//! neighbouring faces never stack their tint or their glow, which a
//! per-face glow did in the spike.

use crate::faceted::{lambert, quantize, Faceted};
use crate::finish::{Finish, FrameCtx, Layer, Light, Surface};
use crate::path::{Seg, Subpath};
use crate::prim::{Fill, Prim2, Pt2, Rgb, Style};

/// The skin under x-ray: a tinted translucent fill with lit facet edges.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glass {
    /// The tint, fill and edges.
    pub tint: Rgb,
    /// The fill's opacity, in `[0, 1]`.
    pub alpha: f64,
    /// The facet edges' width, image units, `≥ 0`.
    pub edge_width: f64,
    /// The facet edges' opacity, in `[0, 1]`.
    pub edge_alpha: f64,
}

/// The halo around an inner part, in the part's lightest tone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glow {
    /// The halo's width, image units, `≥ 0`: it reaches half of it past
    /// the part's outline.
    pub width: f64,
    /// The halo's opacity, in `[0, 1]`.
    pub alpha: f64,
}

/// The x-ray finish.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct XRay {
    solid: Faceted,
    glass: Glass,
    glow: Glow,
}

/// Why an [`XRay`] could not be built. Each variant names its field and
/// carries the value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum XRayError {
    /// `glass.alpha` outside `[0, 1]` or non-finite.
    GlassAlpha(f64),
    /// `glass.edge_width` negative or non-finite.
    EdgeWidth(f64),
    /// `glass.edge_alpha` outside `[0, 1]` or non-finite.
    EdgeAlpha(f64),
    /// `glow.width` negative or non-finite.
    GlowWidth(f64),
    /// `glow.alpha` outside `[0, 1]` or non-finite.
    GlowAlpha(f64),
}

impl std::fmt::Display for XRayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XRayError::GlassAlpha(v) => write!(f, "glass.alpha: {v} is not in [0, 1]"),
            XRayError::EdgeWidth(v) => write!(f, "glass.edge_width: {v} is not finite and >= 0"),
            XRayError::EdgeAlpha(v) => write!(f, "glass.edge_alpha: {v} is not in [0, 1]"),
            XRayError::GlowWidth(v) => write!(f, "glow.width: {v} is not finite and >= 0"),
            XRayError::GlowAlpha(v) => write!(f, "glow.alpha: {v} is not in [0, 1]"),
        }
    }
}

impl std::error::Error for XRayError {}

impl XRay {
    /// The x-ray finish: `solid` paints the props and bands the inner
    /// parts' tones, `glass` the skin, `glow` the halo.
    ///
    /// # Errors
    /// An [`XRayError`] naming the first invalid field.
    pub fn new(solid: Faceted, glass: Glass, glow: Glow) -> Result<XRay, XRayError> {
        let unit = |a: f64| (0.0..=1.0).contains(&a);
        let width = |w: f64| w.is_finite() && w >= 0.0;
        if !unit(glass.alpha) {
            return Err(XRayError::GlassAlpha(glass.alpha));
        }
        if !width(glass.edge_width) {
            return Err(XRayError::EdgeWidth(glass.edge_width));
        }
        if !unit(glass.edge_alpha) {
            return Err(XRayError::EdgeAlpha(glass.edge_alpha));
        }
        if !width(glow.width) {
            return Err(XRayError::GlowWidth(glow.width));
        }
        if !unit(glow.alpha) {
            return Err(XRayError::GlowAlpha(glow.alpha));
        }
        Ok(XRay { solid, glass, glow })
    }

    /// The inner surfaces' bands, quantised among themselves (the props'
    /// bands are [`Faceted::bands_of`], which skips them).
    fn inner_bands(&self, batch: &[Surface], light: &Light) -> Vec<Option<usize>> {
        let inner = |s: &Surface| s.material().layer() == Layer::Inner;
        let values: Vec<f64> = batch
            .iter()
            .filter(|s| inner(s))
            .map(|s| lambert(s.facing().normal(), light.toward()))
            .collect();
        let mut bands = quantize(&values, self.solid.bands()).into_iter();
        batch
            .iter()
            .map(|s| inner(s).then(|| bands.next()).flatten())
            .collect()
    }
}

impl Finish for XRay {
    /// Runs of consecutive surfaces of one material, in batch order:
    /// a solid run surface by surface as [`Faceted`]; a skin run as one
    /// glass primitive; an inner run as one glow primitive, then its
    /// surfaces opaque in banded tones.
    fn paint(&self, batch: &[Surface], light: &Light, _ctx: &FrameCtx) -> Vec<Prim2> {
        let solid = self.solid.bands_of(batch, light);
        let inner = self.inner_bands(batch, light);
        let mut out = Vec::new();
        let mut i = 0;
        while i < batch.len() {
            let material = batch[i].material();
            let end = i + batch[i..]
                .iter()
                .take_while(|s| s.material() == material)
                .count();
            let run = &batch[i..end];
            let union = || run.iter().flat_map(|s| s.outline().to_vec()).collect();
            match material.layer() {
                Layer::Solid => {
                    for (s, band) in run.iter().zip(&solid[i..end]) {
                        out.extend(band.map(|b| self.solid.prim(s, b)));
                    }
                }
                Layer::Skin => out.push(Prim2::Path {
                    subpaths: union(),
                    style: Style {
                        stroke: self.glass.tint,
                        width: self.glass.edge_width,
                        alpha: self.glass.edge_alpha,
                        fill: Some(Fill::solid(self.glass.tint, self.glass.alpha)),
                    },
                }),
                Layer::Inner => {
                    let lightest = material.tones()[material.tones().len() - 1];
                    out.push(Prim2::Path {
                        subpaths: union(),
                        style: Style {
                            stroke: lightest,
                            width: self.glow.width,
                            alpha: self.glow.alpha,
                            fill: None,
                        },
                    });
                    for (s, band) in run.iter().zip(&inner[i..end]) {
                        out.extend(band.map(|b| self.solid.prim(s, b)));
                    }
                }
            }
            i = end;
        }
        out
    }
}

/// Two finishes split by a line in the image (R-0019 AC6): `swept` where
/// the line has already passed, `unswept` ahead of it.
///
/// Both finishes paint the **whole** batch, so a finish that quantises
/// over the figure keeps its tones while the line moves; their outputs are
/// then clipped to their half-planes. The halves do not overlap, so
/// painting one after the other keeps every surface's depth order.
///
/// Clipping is exact for straight outlines. A subpath with a cubic piece
/// is kept whole on the side of its start point (curved outlines are not
/// split; SPEC-0019 §4). A primitive that is not a path goes to the side of
/// its first point.
#[derive(Clone, Copy)]
pub struct Sweep<'a> {
    swept: &'a dyn Finish,
    unswept: &'a dyn Finish,
    at: Pt2,
    toward: Pt2,
}

/// Why a [`Sweep`] could not be built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SweepError {
    /// `at`: a non-finite point.
    At,
    /// `toward`: a zero or non-finite direction.
    Toward,
}

impl std::fmt::Display for SweepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            SweepError::At => "at: a non-finite point",
            SweepError::Toward => "toward: a zero or non-finite direction",
        })
    }
}

impl std::error::Error for SweepError {}

impl<'a> Sweep<'a> {
    /// The line through `at`, moving `toward` (any length): a point `p`
    /// with `(p − at)·toward < 0` is swept.
    ///
    /// # Errors
    /// [`SweepError::At`] or [`SweepError::Toward`].
    pub fn new(
        swept: &'a dyn Finish,
        unswept: &'a dyn Finish,
        at: Pt2,
        toward: Pt2,
    ) -> Result<Sweep<'a>, SweepError> {
        if !(at.x.is_finite() && at.y.is_finite()) {
            return Err(SweepError::At);
        }
        let ok = toward.x.is_finite() && toward.y.is_finite();
        if !ok || (toward.x == 0.0 && toward.y == 0.0) {
            return Err(SweepError::Toward);
        }
        Ok(Sweep {
            swept,
            unswept,
            at,
            toward,
        })
    }

    /// The signed distance-like side of `p`: negative is swept.
    fn side(&self, p: Pt2) -> f64 {
        (p.x - self.at.x) * self.toward.x + (p.y - self.at.y) * self.toward.y
    }
}

impl Finish for Sweep<'_> {
    fn paint(&self, batch: &[Surface], light: &Light, ctx: &FrameCtx) -> Vec<Prim2> {
        let mut out = Vec::new();
        for (finish, swept) in [(self.swept, true), (self.unswept, false)] {
            let keep = |p: Pt2| (self.side(p) < 0.0) == swept;
            for prim in finish.paint(batch, light, ctx) {
                out.extend(clip_prim(prim, &keep, &|p| self.side(p)));
            }
        }
        out
    }
}

/// `prim` restricted to the points `keep` accepts; `None` if nothing is
/// left. `side` is the line's signed function, for the crossing points.
fn clip_prim(prim: Prim2, keep: &dyn Fn(Pt2) -> bool, side: &dyn Fn(Pt2) -> f64) -> Option<Prim2> {
    match prim {
        Prim2::Path { subpaths, style } => {
            let subpaths: Vec<Subpath<Pt2>> = subpaths
                .into_iter()
                .filter_map(|s| clip_subpath(s, keep, side))
                .collect();
            (!subpaths.is_empty()).then_some(Prim2::Path { subpaths, style })
        }
        Prim2::Point { at, .. } | Prim2::Text { at, .. } => keep(at).then_some(prim),
        Prim2::Segment { a, .. } => keep(a).then_some(prim),
        Prim2::Polyline { ref points, .. } => {
            points.first().is_some_and(|&p| keep(p)).then_some(prim)
        }
        Prim2::Edges { ref segments, .. } => segments
            .first()
            .is_some_and(|&(p, _)| keep(p))
            .then_some(prim),
    }
}

/// One subpath clipped to the kept half-plane (Sutherland–Hodgman against
/// one line, which is exact for any polygon). The crossing on edge `p → q`
/// is computed from `p` in both halves, so the two halves meet on exactly
/// the same points.
fn clip_subpath(
    s: Subpath<Pt2>,
    keep: &dyn Fn(Pt2) -> bool,
    side: &dyn Fn(Pt2) -> f64,
) -> Option<Subpath<Pt2>> {
    let mut pts = vec![s.start];
    for seg in &s.segs {
        match *seg {
            Seg::Line(p) => pts.push(p),
            Seg::Cubic(..) => return keep(s.start).then_some(s),
        }
    }
    let n = pts.len();
    let mut out: Vec<Pt2> = Vec::with_capacity(n + 2);
    for k in 0..n {
        let (p, q) = (pts[k], pts[(k + 1) % n]);
        if keep(p) {
            out.push(p);
        }
        if keep(p) != keep(q) {
            let (sp, sq) = (side(p), side(q));
            let u = sp / (sp - sq);
            out.push(Pt2 {
                x: p.x + (q.x - p.x) * u,
                y: p.y + (q.y - p.y) * u,
            });
        }
    }
    out.dedup();
    if out.len() > 1 && out.first() == out.last() {
        out.pop();
    }
    if out.len() < 3 {
        return None;
    }
    let mut sub = Subpath::new(out[0]);
    for &p in &out[1..] {
        sub = sub.line_to(p);
    }
    sub.closed = s.closed;
    Some(sub)
}
