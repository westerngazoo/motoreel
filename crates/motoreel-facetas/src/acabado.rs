//! The interchangeable finish («acabado»): how a lit face becomes paint.
//!
//! The geometry, the lighting and the sort are shared; only this step
//! changes between [`crate::Facetas`], [`crate::Pintura`] and
//! [`crate::RayosX`]. A finish is deterministic: the same face gives the
//! same primitives, so the same scene gives the same bytes.

use motoreel::{Fill, Prim2, Pt2, Rgb, Style};

use crate::face::Face;

/// Turns one face into motoreel primitives.
///
/// Called once per visible face, far to near; whatever a call appends is
/// drawn over everything appended before it. A finish may emit nothing
/// for a layer it does not show (the faceted finish hides
/// [`crate::Layer::Inside`]).
pub trait Acabado {
    /// Appends the primitives for `face` to `out`.
    fn paint(&self, face: &Face, out: &mut Vec<Prim2>);
}

/// A filled (and optionally stroked) polygon, or `None` when `pts` is not
/// a polygon (fewer than three distinct finite corners). motoreel's
/// `Prim2::Path` must hold at least one subpath, so the empty case is
/// dropped here rather than handed to a sink.
#[must_use]
pub fn polygon(pts: &[Pt2], style: Style) -> Option<Prim2> {
    let subpaths = motoreel::shapes::polygon(pts);
    (!subpaths.is_empty()).then_some(Prim2::Path { subpaths, style })
}

/// A fill-only style.
#[must_use]
pub fn fill_only(colour: Rgb, alpha: f64) -> Style {
    Style {
        stroke: colour,
        width: 0.0,
        alpha: 0.0,
        fill: Some(Fill::solid(colour, alpha)),
    }
}

/// A fill plus a stroke of the same colour `seam` wide: covers the hairline
/// of background an antialiased edge leaves between two abutting faces.
#[must_use]
pub fn sealed(colour: Rgb, alpha: f64, seam: f64) -> Style {
    Style {
        stroke: colour,
        width: seam,
        alpha,
        fill: Some(Fill::solid(colour, alpha)),
    }
}
