//! «Facetas» (R-0017 AC5, SPEC-0017 §2.4): the house's low-poly look.
//!
//! Each surface is lit by `n·l / (|n| |l|)`, the lit values are cut into
//! bands of equal population over the **whole batch**, and each surface is
//! painted in its material's tone for its band. The arithmetic is
//! rotorf-sico rf-marca's `lambert`, `cuantizar`, `faceta`,
//! `faceta_con_junta` and `velo` (`brand/rf-marca/src/poly.rs` at
//! `162ce66`), operation for operation, so the output is the same `Prim2`
//! bit for bit. Only `+ − × ÷`, `sqrt` and comparisons enter.

use crate::finish::{Finish, FrameCtx, Layer, Light, Material, Surface};
use crate::prim::{Fill, Prim2, Style};

/// The faceted finish.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Faceted {
    bands: usize,
    stroke: f64,
}

/// Why a [`Faceted`] could not be built. Each variant names its field.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FacetedError {
    /// `bands`: zero.
    Bands,
    /// `stroke`: negative or non-finite; carries the value.
    Stroke(f64),
}

impl std::fmt::Display for FacetedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FacetedError::Bands => f.write_str("bands: at least one"),
            FacetedError::Stroke(w) => write!(f, "stroke: {w} is not finite and >= 0"),
        }
    }
}

impl std::error::Error for FacetedError {}

impl Faceted {
    /// `bands` tone bands; an opaque face is stroked `stroke` image units
    /// wide in its own colour (rf-marca's `u(1.0)`: one canvas pixel, a
    /// scale only the caller knows).
    ///
    /// # Errors
    /// [`FacetedError::Bands`] for zero bands, [`FacetedError::Stroke`]
    /// for a negative or non-finite stroke.
    pub fn new(bands: usize, stroke: f64) -> Result<Faceted, FacetedError> {
        if bands == 0 {
            return Err(FacetedError::Bands);
        }
        if !(stroke.is_finite() && stroke >= 0.0) {
            return Err(FacetedError::Stroke(stroke));
        }
        Ok(Faceted { bands, stroke })
    }

    /// The band count.
    #[must_use]
    pub fn bands(&self) -> usize {
        self.bands
    }

    /// Each surface's band, shadow `0` to light `bands − 1`, quantised over
    /// the batch's shown surfaces; `None` for a [`Layer::Inner`] surface,
    /// which this finish hides and which does not count toward the bands.
    #[must_use]
    pub fn bands_of(&self, batch: &[Surface], light: &Light) -> Vec<Option<usize>> {
        let shown = |s: &&Surface| s.material().layer() != Layer::Inner;
        let values: Vec<f64> = batch
            .iter()
            .filter(shown)
            .map(|s| lambert(s.facing().normal(), light.toward()))
            .collect();
        let mut bands = quantize(&values, self.bands).into_iter();
        batch
            .iter()
            .map(|s| shown(&s).then(|| bands.next()).flatten())
            .collect()
    }

    /// The colour band `band` takes in `material`: ramp index
    /// `band · len / bands`, so a ramp as long as the band count is read
    /// one to one.
    #[must_use]
    pub fn tone(&self, material: &Material, band: usize) -> crate::prim::Rgb {
        let tones = material.tones();
        tones[(band * tones.len() / self.bands).min(tones.len() - 1)]
    }

    /// The primitive for `surface` painted at `band` (rf-marca's three
    /// helpers):
    ///
    /// - a seam → `faceta_con_junta`: the fill at the material's opacity,
    ///   stroked in the seam's colour and width at the same opacity;
    /// - opaque → `faceta`: fill and a `stroke`-wide stroke in the same
    ///   colour, which hides the hairline two antialiased fills leave;
    /// - translucent → `velo`: the fill alone (a zero-width, zero-alpha
    ///   stroke), so the edge is not doubled.
    #[must_use]
    pub fn prim(&self, surface: &Surface, band: usize) -> Prim2 {
        let material = surface.material();
        let c = self.tone(material, band);
        let a = material.opacity();
        let style = match material.seam() {
            Some(seam) => Style {
                stroke: seam.colour,
                width: seam.width,
                alpha: a,
                fill: Some(Fill::solid(c, a)),
            },
            None if a == 1.0 => Style {
                stroke: c,
                width: self.stroke,
                alpha: 1.0,
                fill: Some(Fill::solid(c, 1.0)),
            },
            None => Style {
                stroke: c,
                width: 0.0,
                alpha: 0.0,
                fill: Some(Fill::solid(c, a)),
            },
        };
        Prim2::Path {
            subpaths: surface.outline().to_vec(),
            style,
        }
    }
}

impl Finish for Faceted {
    /// One primitive per shown surface, in batch order.
    fn paint(&self, batch: &[Surface], light: &Light, _ctx: &FrameCtx) -> Vec<Prim2> {
        batch
            .iter()
            .zip(self.bands_of(batch, light))
            .filter_map(|(s, band)| Some(self.prim(s, band?)))
            .collect()
    }
}

/// The light value of a face with normal `n` under a light toward `l`:
/// `n·l / (|n| |l|)`, the normal first turned to face the viewer when its
/// `z < 0`. rf-marca's `lambert` after its cross product, verbatim.
#[must_use]
pub fn lambert(n: [f64; 3], l: [f64; 3]) -> f64 {
    let n = if n[2] < 0.0 { [-n[0], -n[1], -n[2]] } else { n };
    let nl = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    let ll = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt();
    (n[0] * l[0] + n[1] * l[1] + n[2] * l[2]) / (nl * ll)
}

/// Each value's band, `0` (darkest) to `bands − 1`, with bands of equal
/// population: rf-marca's `cuantizar`, verbatim (a stable `total_cmp` sort,
/// cuts at `len·k / bands`). Empty input gives empty output.
#[must_use]
pub fn quantize(values: &[f64], bands: usize) -> Vec<usize> {
    if values.is_empty() {
        return Vec::new();
    }
    let mut order = values.to_vec();
    order.sort_by(f64::total_cmp);
    let cuts: Vec<f64> = (1..bands).map(|k| order[order.len() * k / bands]).collect();
    values
        .iter()
        .map(|v| cuts.iter().filter(|&&c| *v >= c).count())
        .collect()
}
