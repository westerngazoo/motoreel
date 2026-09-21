//! Box tree to absolute positions. Still no pixels.
//!
//! Coordinates match motoreel's `prim::Pt2`: **x right, y up**. A glyph
//! is placed at its baseline origin, so `y` is where the baseline sits
//! and the ink extends `height` above it and `depth` below.
//!
//! Keeping this step separate from rasterizing is what lets a layout gate
//! ask "where does every glyph land" — and therefore "does anything leave
//! the canvas, or cover anything else" — without a frame buffer.

use crate::node::{FaceId, Metrics, Node};

/// One glyph, resolved to an absolute baseline origin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    /// Baseline origin, x.
    pub x: f64,
    /// Baseline origin, y — increasing upward.
    pub y: f64,
    /// Which face to draw it from.
    pub face: FaceId,
    /// The face's glyph id.
    pub id: u16,
    /// Em size.
    pub size: f64,
    /// Left side bearing: how far the ink starts from `x`. May be negative.
    pub bearing: f64,
    /// What the glyph covers, measured from `x + bearing`.
    pub ink: Metrics,
}

/// A filled rectangle, resolved to absolute coordinates: the fraction
/// bar, a radical's overbar, an underline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlacedRule {
    /// Left edge.
    pub x: f64,
    /// Baseline; the rule spans `height` above and `depth` below it.
    pub y: f64,
    /// Extent.
    pub metrics: Metrics,
}

/// Everything a sink needs to draw, in absolute coordinates.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
    /// Glyphs, in the order they were measured.
    pub glyphs: Vec<Placed>,
    /// Rules.
    pub rules: Vec<PlacedRule>,
}

impl Layout {
    /// The bounding box of all ink, as `(left, bottom, right, top)`, or
    /// `None` when nothing is inked.
    ///
    /// This is the number a collision gate wants, and it is available
    /// without rasterizing — which is the whole point of R-0009 AC4.
    #[must_use]
    pub fn ink_bounds(&self) -> Option<(f64, f64, f64, f64)> {
        let mut b: Option<(f64, f64, f64, f64)> = None;
        let mut grow = |l: f64, bo: f64, r: f64, t: f64| {
            b = Some(match b {
                None => (l, bo, r, t),
                Some((l0, b0, r0, t0)) => (l0.min(l), b0.min(bo), r0.max(r), t0.max(t)),
            });
        };
        for g in &self.glyphs {
            if g.ink.width == 0.0 && g.ink.total_height() == 0.0 {
                continue; // a space claims advance, not area
            }
            // The ink starts at the bearing, not at the pen. Skipping
            // that term under-reports the box on both edges, and the
            // acceptance suite catches it as ink outside its own bounds.
            grow(
                g.x + g.bearing,
                g.y - g.ink.depth,
                g.x + g.bearing + g.ink.width,
                g.y + g.ink.height,
            );
        }
        for r in &self.rules {
            grow(
                r.x,
                r.y - r.metrics.depth,
                r.x + r.metrics.width,
                r.y + r.metrics.height,
            );
        }
        b
    }
}

/// Where a run sits relative to the anchor it was given.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    /// The anchor is the run's left edge.
    #[default]
    Left,
    /// The anchor is the run's horizontal centre.
    Center,
    /// The anchor is the run's right edge.
    Right,
}

/// Resolve a tree to absolute positions, with `at` as the baseline origin
/// and `align` deciding which part of the run lands on it.
#[must_use]
pub fn place(node: &Node, at: (f64, f64), align: Align) -> Layout {
    let w = node.metrics().width;
    let x = match align {
        Align::Left => at.0,
        Align::Center => at.0 - w / 2.0,
        Align::Right => at.0 - w,
    };
    let mut out = Layout::default();
    walk(node, x, at.1, &mut out);
    out
}

fn walk(node: &Node, x: f64, y: f64, out: &mut Layout) {
    match node {
        Node::Glyph(g) => out.glyphs.push(Placed {
            x,
            y,
            face: g.face,
            id: g.id,
            size: g.size,
            bearing: g.bearing,
            ink: g.ink,
        }),
        Node::Row(items) => {
            let mut pen = x;
            for n in items {
                walk(n, pen, y, out);
                pen += n.metrics().width;
            }
        }
        Node::Stack(s) => {
            if s.items.is_empty() {
                return;
            }
            let base = if s.base < s.items.len() { s.base } else { 0 };
            let width = s.metrics().width;
            // Items narrower than the stack are centred — which is what a
            // numerator over a wider denominator has to do.
            let put = |n: &Node, yy: f64, out: &mut Layout| {
                let dx = (width - n.metrics().width) / 2.0;
                walk(n, x + dx, yy, out);
            };
            put(&s.items[base], y, out);
            // Upward from the base: clear this item's height, the gap,
            // then the neighbour's depth, and we are on its baseline.
            let mut up = y + s.items[base].metrics().height;
            for n in s.items[..base].iter().rev() {
                let m = n.metrics();
                up += s.gap + m.depth;
                put(n, up, out);
                up += m.height;
            }
            let mut down = y - s.items[base].metrics().depth;
            for n in &s.items[base + 1..] {
                let m = n.metrics();
                down -= s.gap + m.height;
                put(n, down, out);
                down -= m.depth;
            }
        }
        Node::Glue(_) => {}
        Node::Rule(m) => out.rules.push(PlacedRule { x, y, metrics: *m }),
    }
}

#[cfg(test)]
mod tests {
    // Exactness is the assertion, not an accident: these boxes are built
    // from small decimals whose sums are exact in binary floating point,
    // and a test that tolerated drift would stop catching the arithmetic
    // it exists to pin down.
    #![allow(
        clippy::float_cmp,
        reason = "the box arithmetic here is exact by construction"
    )]

    use super::*;
    use crate::node::{Glyph, Stack};

    fn g(advance: f64) -> Node {
        Node::Glyph(Glyph {
            face: FaceId(0),
            id: 1,
            size: 10.0,
            advance,
            bearing: 0.0,
            ink: Metrics {
                width: advance,
                height: 7.0,
                depth: 0.0,
            },
        })
    }

    #[test]
    fn a_row_advances_left_to_right() {
        let l = place(&Node::Row(vec![g(3.0), g(4.0)]), (10.0, 100.0), Align::Left);
        assert_eq!(l.glyphs[0].x, 10.0);
        assert_eq!(l.glyphs[1].x, 13.0);
        assert!(l.glyphs.iter().all(|p| p.y == 100.0), "one baseline");
    }

    #[test]
    fn centring_puts_the_middle_on_the_anchor() {
        let row = Node::Row(vec![g(3.0), g(4.0)]);
        let l = place(&row, (100.0, 0.0), Align::Center);
        let (left, _, right, _) = l.ink_bounds().unwrap();
        assert!(
            (left.midpoint(right) - 100.0).abs() < 1e-9,
            "{left}..{right}"
        );
    }

    #[test]
    fn right_alignment_ends_on_the_anchor() {
        let l = place(&Node::Row(vec![g(3.0), g(4.0)]), (100.0, 0.0), Align::Right);
        assert_eq!(l.glyphs[0].x, 93.0);
        assert_eq!(l.glyphs[1].x, 96.0);
    }

    #[test]
    fn glue_moves_the_pen_and_draws_nothing() {
        let l = place(
            &Node::Row(vec![g(3.0), Node::Glue(10.0), g(4.0)]),
            (0.0, 0.0),
            Align::Left,
        );
        assert_eq!(l.glyphs.len(), 2, "glue is not a glyph");
        assert_eq!(l.glyphs[1].x, 13.0, "but it did move the pen");
    }

    /// Proves the stack arithmetic on the shape stage 3 needs: the
    /// numerator must end up above the bar and the denominator below,
    /// with the bar on the surrounding baseline.
    #[test]
    fn a_fraction_stacks_around_its_bar() {
        let bar = Metrics {
            width: 6.0,
            height: 1.0,
            depth: 0.0,
        };
        let frac = Node::Stack(Stack {
            items: vec![g(4.0), Node::Rule(bar), g(4.0)],
            base: 1,
            gap: 2.0,
        });
        let l = place(&frac, (0.0, 0.0), Align::Left);
        assert_eq!(l.rules[0].y, 0.0, "the bar owns the baseline");
        assert!(l.glyphs[0].y > 0.0, "numerator above");
        assert!(l.glyphs[1].y < 0.0, "denominator below");
        // narrower items are centred over the widest
        assert_eq!(l.glyphs[0].x, 1.0, "(6 - 4) / 2");
    }

    /// `place` centres on the **advance** box, which is what every text
    /// engine does; the ink box then sits wherever the side bearings put
    /// it. Asserting the ink is centred instead is wrong, and it is the
    /// assertion the first cut of the acceptance suite made.
    #[test]
    fn centring_uses_the_advance_not_the_ink() {
        let wonky = Node::Glyph(Glyph {
            face: FaceId(0),
            id: 1,
            size: 10.0,
            advance: 10.0,
            bearing: 3.0,
            ink: Metrics {
                width: 4.0,
                height: 7.0,
                depth: 0.0,
            },
        });
        let l = place(&wonky, (100.0, 0.0), Align::Center);
        assert_eq!(l.glyphs[0].x, 95.0, "the advance box is centred");
        let (left, _, right, _) = l.ink_bounds().unwrap();
        assert_eq!((left, right), (98.0, 102.0), "the ink lands off-centre");
    }

    #[test]
    fn a_negative_bearing_widens_the_bounds_to_the_left() {
        let hook = Node::Glyph(Glyph {
            face: FaceId(0),
            id: 1,
            size: 10.0,
            advance: 4.0,
            bearing: -1.5,
            ink: Metrics {
                width: 7.0,
                height: 7.0,
                depth: 2.0,
            },
        });
        let l = place(&hook, (0.0, 0.0), Align::Left);
        let (left, bottom, right, top) = l.ink_bounds().unwrap();
        assert_eq!((left, right), (-1.5, 5.5));
        assert_eq!((bottom, top), (-2.0, 7.0));
    }

    #[test]
    fn nothing_inked_has_no_bounds() {
        assert_eq!(
            place(&Node::Row(vec![]), (0.0, 0.0), Align::Left).ink_bounds(),
            None
        );
        assert_eq!(
            place(&Node::Glue(5.0), (0.0, 0.0), Align::Left).ink_bounds(),
            None,
            "glue claims advance, not area"
        );
    }
}
