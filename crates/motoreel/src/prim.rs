//! The 2D output vocabulary — the sink-facing boundary.
//!
//! Depends on `std` alone: no garust type crosses this module, which is
//! the concrete form of "the SVG sink never learns 3D existed".

/// A point in image space: `x` right, `y` up, origin on the optical axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pt2 {
    /// Rightward image coordinate.
    pub x: f64,
    /// Upward image coordinate (sinks flip for SVG's y-down).
    pub y: f64,
}

/// An sRGB stroke color, 8 bits per channel.
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

/// Stroke style — R-0002's minimal vocabulary: color, width, alpha.
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
}

impl Default for Style {
    /// White, width 0.01 (≈ 6 px at 1080p in SPEC-0003's default view),
    /// opaque.
    fn default() -> Self {
        Style {
            stroke: Rgb::WHITE,
            width: 0.01,
            alpha: 1.0,
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
        /// Stroke style, passed through unchanged.
        style: Style,
    },
    /// A straight stroke from `a` to `b`.
    Segment {
        /// Stroke start.
        a: Pt2,
        /// Stroke end.
        b: Pt2,
        /// Stroke style, passed through unchanged.
        style: Style,
    },
    /// An open polyline through `points` in order (< 2 points draws
    /// nothing or a dot — the sink's call).
    Polyline {
        /// Image positions, in order.
        points: Vec<Pt2>,
        /// Stroke style, passed through unchanged.
        style: Style,
    },
    /// A wireframe: disjoint segments sharing one style, drawn as one
    /// primitive so a solid is culled and styled as a whole.
    Edges {
        /// Image-space endpoint pairs, in order.
        segments: Vec<(Pt2, Pt2)>,
        /// Stroke style, passed through unchanged.
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
        /// The line to draw: printable ASCII, one line, no markup.
        text: String,
        /// Em height in image units. Contract: finite and > 0.
        size: f64,
        /// Horizontal placement of `at` relative to the run.
        align: Align,
        /// Fill colour and opacity; [`Style::width`] is unused for text.
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
    }
}
