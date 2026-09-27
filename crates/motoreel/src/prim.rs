//! The 2D output vocabulary — the sink-facing boundary.
//!
//! Depends on `std` alone: no garust type crosses this module, which is
//! the concrete form of "the SVG sink never learns 3D existed". Paths
//! borrow their generic shape from [`crate::path`], which is std-only too.

use crate::path::Subpath;

/// A point in a plane: `x` right, `y` up.
///
/// In image space the origin is on the optical axis. The shape generators
/// ([`crate::shapes`]) emit it too, as planar model coordinates that
/// [`crate::Object::planar`] lifts to `z = 0` (SPEC-0012 §2.3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pt2 {
    /// Rightward image coordinate.
    pub x: f64,
    /// Upward image coordinate (sinks flip for SVG's y-down).
    pub y: f64,
}

/// An sRGB color, 8 bits per channel: a stroke's or a fill's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
}

impl Rgb {
    /// Pure white — the default stroke for the assumed dark background.
    pub const WHITE: Rgb = Rgb {
        r: 255,
        g: 255,
        b: 255,
    };
    /// Pure black.
    pub const BLACK: Rgb = Rgb { r: 0, g: 0, b: 0 };
}

/// Interior paint: one flat color and an opacity (R-0012 AC2).
///
/// Gradients are a later requirement. Contract, as for [`Style`]: `alpha`
/// in `[0, 1]`, documented, not validated. Sinks read it through a total
/// clamp, and a non-finite `alpha` paints nothing (SPEC-0012 §2.13).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fill {
    /// Fill color.
    pub colour: Rgb,
    /// Fill opacity, 0 transparent to 1 opaque.
    pub alpha: f64,
}

impl Fill {
    /// A flat fill of `colour` at `alpha`.
    ///
    /// The constructor downstream code should use: the gradients
    /// requirement can then grow `Fill` without breaking literals again.
    pub const fn solid(colour: Rgb, alpha: f64) -> Self {
        Fill { colour, alpha }
    }
}

/// Paint style — R-0002's stroke vocabulary (color, width, alpha) plus
/// R-0012's optional fill.
///
/// Contracts (documented, not validated — style is author data carried
/// verbatim to the emitted primitive): `width` finite and ≥ 0, in image
/// units; `alpha` in `[0, 1]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    /// Stroke color.
    pub stroke: Rgb,
    /// Stroke width, in image units.
    pub width: f64,
    /// Stroke opacity, 0 transparent to 1 opaque.
    pub alpha: f64,
    /// Interior paint (R-0012 OQ-3). `None` is the behaviour every
    /// primitive had before paths.
    ///
    /// Honoured by [`Prim2::Path`] only; every other variant ignores it
    /// (SPEC-0012 §2.2). A fill-only path has a `width` of 0.
    pub fill: Option<Fill>,
}

impl Default for Style {
    /// White, width 0.01 (≈ 6 px at 1080p in SPEC-0003's default view),
    /// opaque, no fill.
    fn default() -> Self {
        Style {
            stroke: Rgb::WHITE,
            width: 0.01,
            alpha: 1.0,
            fill: None,
        }
    }
}

/// Horizontal placement of a text run relative to its anchor point.
///
/// Maps 1:1 to SVG `text-anchor` (`start` / `middle` / `end`). motoreel text
/// is ASCII, single-line and left-to-right (R-0007 §4), so the directional
/// names are accurate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    /// The anchor is the run's left edge.
    Left,
    /// The anchor is the run's horizontal centre.
    Center,
    /// The anchor is the run's right edge.
    Right,
}

/// A flat 2D drawing primitive — everything a sink needs, no trace of 3D.
///
/// Invariant upheld by `Scene::eval`: every coordinate is finite. Styles
/// are carried verbatim under their own documented contracts.
#[derive(Clone, Debug, PartialEq)]
pub enum Prim2 {
    /// A dot.
    Point {
        /// Image position.
        at: Pt2,
        /// Stroke style, passed through unchanged. `fill` is ignored: a
        /// filled dot is a [`crate::shapes::circle`] (R-0012 OQ-4).
        style: Style,
    },
    /// A straight stroke from `a` to `b`.
    Segment {
        /// Stroke start.
        a: Pt2,
        /// Stroke end.
        b: Pt2,
        /// Stroke style, passed through unchanged. `fill` is ignored: a
        /// segment encloses no area.
        style: Style,
    },
    /// An open polyline through `points` in order (< 2 points draws
    /// nothing or a dot — the sink's call).
    Polyline {
        /// Image positions, in order.
        points: Vec<Pt2>,
        /// Stroke style, passed through unchanged. `fill` is ignored
        /// (SPEC-0012 §2.2): an area is a [`Prim2::Path`].
        style: Style,
    },
    /// A wireframe: disjoint segments sharing one style, drawn as one
    /// primitive so a solid is culled and styled as a whole.
    Edges {
        /// Image-space endpoint pairs, in order.
        segments: Vec<(Pt2, Pt2)>,
        /// Stroke style, passed through unchanged. `fill` is ignored:
        /// disjoint segments enclose no area.
        style: Style,
    },
    /// A single line of printable-ASCII text pinned to one image-space
    /// point.
    ///
    /// Invariants upheld by [`crate::Scene::eval`]: `at` is finite, and
    /// every byte of `text` lies in `0x20..=0x7E`. `size` is author data
    /// under the same passthrough rule as [`Style`] — carried verbatim,
    /// guarded by each sink, never silently sanitized.
    Text {
        /// Image position of the alignment point, on the text baseline.
        at: Pt2,
        /// The line to draw: one line, any text the chosen face covers,
        /// no markup.
        ///
        /// The ASCII restriction this field used to carry was removed by
        /// R-0009. It was not a rendering limit: it was applied at eval,
        /// upstream of both sinks, so the SVG sink — which emits `<text>`
        /// and lets the consumer's font engine work — was handed
        /// pre-destroyed text for months and drew «b?ceps».
        text: String,
        /// Em height in image units. Contract: finite and > 0.
        size: f64,
        /// Horizontal placement of `at` relative to the run.
        align: Align,
        /// Which registered face to set this in: an index into the
        /// registry the sink was built with. A plain `usize`, not a typed
        /// id, so the core keeps no font dependency of its own.
        face: usize,
        /// Fill colour and opacity; [`Style::width`] is unused for text,
        /// and so is [`Style::fill`]: glyph coverage is painted in
        /// `stroke`.
        style: Style,
    },
    /// An outline of straight and cubic pieces, stroked, filled, or both
    /// (R-0012, SPEC-0012 §2.3).
    ///
    /// Invariants upheld by [`crate::Scene::eval`]: every coordinate is
    /// finite, there is at least one subpath, and no subpath is empty of
    /// segments. Sinks paint the fill first and the stroke second, each
    /// composited once per pixel (SVG's `paint-order: normal`). The fill
    /// follows the nonzero winding rule and treats every subpath as
    /// closed; the stroke draws a closing segment only where `closed` is
    /// set.
    Path {
        /// Image-space subpaths, in order.
        subpaths: Vec<Subpath<Pt2>>,
        /// Stroke and fill, passed through unchanged.
        style: Style,
    },
}

#[cfg(test)]
mod tests {
    use super::{Rgb, Style};

    /// The adjudicated defaults: white, 0.01 image units, opaque.
    #[test]
    fn default_style_is_the_adjudicated_one() {
        let s = Style::default();
        assert_eq!(s.stroke, Rgb::WHITE);
        assert_eq!(s.width, 0.01);
        assert_eq!(s.alpha, 1.0);
        assert_eq!(s.fill, None);
    }
}
