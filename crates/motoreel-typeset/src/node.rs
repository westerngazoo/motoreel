//! The box model — three numbers and a tree, and the whole reason this
//! crate exists rather than a wider font table.
//!
//! Every node reports **width**, **height** above the baseline, and
//! **depth** below it. That triple is TeX's, and it is what lets a
//! fraction, a radical or a big operator be laid out by the same code
//! that lays out a word: they are all boxes whose baselines have to line
//! up. Stage 1 (R-0009) builds nothing but [`Node::Row`]s of
//! [`Node::Glyph`]s. The tree is this shape from the start anyway,
//! because designing "a line of text" now means rewriting it for the
//! first fraction.
//!
//! A node carries its own metrics. Nothing here consults a font: the face
//! is asked once, at measure time, and after that the tree is
//! self-describing. That is what makes [`Node::metrics`] pure, cheap, and
//! callable from a layout gate that must not rasterize (R-0009 AC4).

/// Which loaded face a glyph came from.
///
/// An index into the caller's registry; this crate never owns the
/// mapping, so a consumer is free to name its faces however it likes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FaceId(pub usize);

/// Width, height above the baseline, and depth below it — in the same
/// units as the `size` the run was measured at.
///
/// `height` and `depth` are **non-negative distances**, not coordinates:
/// a box that hangs entirely below the baseline has `height == 0` and a
/// positive `depth`. Keeping them unsigned is what makes stacking a
/// matter of addition rather than sign bookkeeping.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Metrics {
    /// Advance width.
    pub width: f64,
    /// Extent above the baseline.
    pub height: f64,
    /// Extent below the baseline.
    pub depth: f64,
}

impl Metrics {
    /// Total vertical extent, `height + depth`.
    #[must_use]
    pub fn total_height(&self) -> f64 {
        self.height + self.depth
    }
}

/// One positioned-glyph's worth of information, resolved at measure time.
///
/// `advance` is kept separate from the ink on purpose: the ink of a glyph
/// and the space it claims are different things. A comma's ink is narrow
/// and low; its advance is what the next glyph must clear. Layout uses
/// the advance; a collision gate wants the ink.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glyph {
    /// Which face this glyph was looked up in.
    pub face: FaceId,
    /// The face's own id for the glyph. Opaque here.
    pub id: u16,
    /// Em size this glyph was measured at.
    pub size: f64,
    /// How far the pen moves after drawing it, kerning already folded in.
    pub advance: f64,
    /// Left side bearing: how far the ink starts from the pen.
    ///
    /// Not derivable from the other fields, and not optional. A `j` in
    /// most faces has a **negative** bearing — its hook reaches left of
    /// where the pen stands — and an italic `f` overhangs on both sides.
    /// Assuming zero is what made the first cut of this crate promise a
    /// box the ink then spilled out of, by two pixels, in the test that
    /// caught it.
    pub bearing: f64,
    /// What the glyph covers, measured from `bearing`.
    pub ink: Metrics,
}

/// A node in the box tree.
#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    /// A single glyph.
    Glyph(Glyph),
    /// Boxes set side by side on a shared baseline.
    Row(Vec<Node>),
    /// Boxes stacked vertically. See [`Stack`].
    Stack(Stack),
    /// Horizontal space with no ink — TeX's glue, without the stretch.
    Glue(f64),
    /// A filled rectangle: the fraction bar, the radical's overbar, an
    /// underline. Its metrics are given, not derived.
    Rule(Metrics),
}

/// Boxes stacked vertically, with one of them owning the baseline.
///
/// `base` is the index of the item whose baseline becomes the stack's.
/// That single field is what makes a fraction expressible: numerator,
/// rule, denominator, with `base` pointing at the rule so the whole thing
/// sits on the surrounding line the way a fraction should. Stage 1 does
/// not build these; the field is here so stage 3 does not have to change
/// the type.
#[derive(Clone, Debug, PartialEq)]
pub struct Stack {
    /// Top to bottom.
    pub items: Vec<Node>,
    /// Index into `items` whose baseline is the stack's baseline.
    ///
    /// Out of range is treated as `0` by [`Node::metrics`] rather than
    /// panicking: a malformed tree should mis-lay-out visibly, not take
    /// the render down. The constructor in `measure` never produces one.
    pub base: usize,
    /// Vertical space inserted between consecutive items.
    pub gap: f64,
}

impl Node {
    /// This node's metrics. Pure: no face, no allocation, no pixels.
    ///
    /// Recursive over the tree, which is bounded by the expression's own
    /// nesting — a label is a row of glyphs, and even a stage-3 formula
    /// is a handful of levels deep.
    #[must_use]
    pub fn metrics(&self) -> Metrics {
        match self {
            Node::Glyph(g) => Metrics {
                width: g.advance,
                height: g.ink.height,
                depth: g.ink.depth,
            },
            Node::Row(items) => items.iter().fold(Metrics::default(), |acc, n| {
                let m = n.metrics();
                Metrics {
                    width: acc.width + m.width,
                    height: acc.height.max(m.height),
                    depth: acc.depth.max(m.depth),
                }
            }),
            Node::Stack(s) => s.metrics(),
            Node::Glue(w) => Metrics {
                width: *w,
                height: 0.0,
                depth: 0.0,
            },
            Node::Rule(m) => *m,
        }
    }

    /// The ink this node covers, as a span measured from its origin.
    ///
    /// A span, not a width, because ink does not begin where the pen
    /// stands: every glyph has a side bearing, and some are negative. A
    /// run ending in a space has an advance the eye cannot see. A layout
    /// gate asking "does this collide" wants this; a layout engine asking
    /// "where does the next box go" wants [`Node::metrics`].
    #[must_use]
    pub fn ink(&self) -> Ink {
        match self {
            // A glyph with no contours — a space, most often — covers
            // nothing at all, and `EMPTY` is how that is said. A
            // zero-width box *at* the bearing is not the same claim: it
            // would drag the union to that x and report a run as
            // starting at its leading space.
            Node::Glyph(g) if g.ink.width == 0.0 && g.ink.total_height() == 0.0 => Ink::EMPTY,
            Node::Glyph(g) => Ink {
                left: g.bearing,
                right: g.bearing + g.ink.width,
                height: g.ink.height,
                depth: g.ink.depth,
            },
            Node::Row(items) => {
                let mut pen = 0.0_f64;
                let mut acc = Ink::EMPTY;
                for n in items {
                    acc = acc.union(n.ink().shifted(pen));
                    pen += n.metrics().width;
                }
                acc
            }
            Node::Stack(s) => {
                let width = s.metrics().width;
                s.items.iter().fold(Ink::EMPTY, |acc, n| {
                    // Narrower items are centred; the ink follows.
                    acc.union(n.ink().shifted((width - n.metrics().width) / 2.0))
                })
            }
            Node::Glue(_) => Ink::EMPTY,
            Node::Rule(m) => Ink {
                left: 0.0,
                right: m.width,
                height: m.height,
                depth: m.depth,
            },
        }
    }
}

/// Where a node's ink actually falls, relative to the node's origin.
///
/// `left` may be negative and `right` may exceed the node's advance
/// width: ink is not obliged to stay inside the space the pen reserves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ink {
    /// Leftmost inked x, from the origin.
    pub left: f64,
    /// Rightmost inked x, from the origin.
    pub right: f64,
    /// Extent above the baseline.
    pub height: f64,
    /// Extent below the baseline.
    pub depth: f64,
}

impl Ink {
    /// No ink anywhere — the identity for [`Ink::union`].
    pub const EMPTY: Ink = Ink {
        left: f64::INFINITY,
        right: f64::NEG_INFINITY,
        height: 0.0,
        depth: 0.0,
    };

    /// Whether anything is inked.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.left > self.right
    }

    /// How wide the ink is; zero when there is none.
    #[must_use]
    pub fn width(&self) -> f64 {
        if self.is_empty() {
            0.0
        } else {
            self.right - self.left
        }
    }

    /// The same ink, moved `dx` to the right.
    #[must_use]
    pub fn shifted(self, dx: f64) -> Ink {
        if self.is_empty() {
            return self;
        }
        Ink {
            left: self.left + dx,
            right: self.right + dx,
            ..self
        }
    }

    /// The smallest ink containing both.
    #[must_use]
    pub fn union(self, other: Ink) -> Ink {
        if self.is_empty() {
            return other;
        }
        if other.is_empty() {
            return self;
        }
        Ink {
            left: self.left.min(other.left),
            right: self.right.max(other.right),
            height: self.height.max(other.height),
            depth: self.depth.max(other.depth),
        }
    }
}

impl Stack {
    /// The stack's own metrics, measured from the baseline its `base`
    /// item owns.
    #[must_use]
    pub fn metrics(&self) -> Metrics {
        if self.items.is_empty() {
            return Metrics::default();
        }
        let base = if self.base < self.items.len() {
            self.base
        } else {
            0
        };
        let width = self
            .items
            .iter()
            .map(|n| n.metrics().width)
            .fold(0.0_f64, f64::max);
        // Walk out from the baseline item in both directions, adding each
        // neighbour's full vertical extent plus one gap.
        let mut height = self.items[base].metrics().height;
        for n in &self.items[..base] {
            height += n.metrics().total_height() + self.gap;
        }
        let mut depth = self.items[base].metrics().depth;
        for n in &self.items[base + 1..] {
            depth += n.metrics().total_height() + self.gap;
        }
        Metrics {
            width,
            height,
            depth,
        }
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

    fn glyph(advance: f64, height: f64, depth: f64) -> Node {
        Node::Glyph(Glyph {
            face: FaceId(0),
            id: 1,
            size: 10.0,
            advance,
            bearing: 0.0,
            ink: Metrics {
                width: advance,
                height,
                depth,
            },
        })
    }

    #[test]
    fn a_row_sums_widths_and_takes_the_tallest() {
        let row = Node::Row(vec![glyph(3.0, 7.0, 0.0), glyph(4.0, 5.0, 2.0)]);
        let m = row.metrics();
        assert_eq!(m.width, 7.0);
        assert_eq!(m.height, 7.0, "the tallest, not the sum");
        assert_eq!(m.depth, 2.0, "the deepest, not the sum");
    }

    #[test]
    fn glue_has_width_and_no_ink() {
        let row = Node::Row(vec![glyph(3.0, 7.0, 0.0), Node::Glue(5.0)]);
        assert_eq!(row.metrics().width, 8.0, "the advance includes the glue");
        assert_eq!(row.ink().width(), 3.0, "the ink does not");
    }

    #[test]
    fn an_empty_row_measures_to_nothing() {
        assert_eq!(Node::Row(vec![]).metrics(), Metrics::default());
        assert!(Node::Row(vec![]).ink().is_empty());
    }

    #[test]
    fn a_leading_space_does_not_drag_the_ink_left() {
        let space = Node::Glyph(Glyph {
            face: FaceId(0),
            id: 3,
            size: 10.0,
            advance: 5.0,
            bearing: 0.0,
            ink: Metrics::default(),
        });
        let row = Node::Row(vec![space, glyph(3.0, 7.0, 0.0)]);
        let i = row.ink();
        assert_eq!(i.left, 5.0, "the ink starts after the space, not at 0");
        assert_eq!(i.width(), 3.0);
    }

    /// The bug the acceptance suite caught: a glyph whose ink starts left
    /// of the pen, or reaches past its advance, has to be reported where
    /// it really is. Assuming the ink begins at the pen promised a box
    /// the ink then spilled out of.
    #[test]
    fn ink_follows_the_side_bearing_even_when_it_is_negative() {
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
        let i = hook.ink();
        assert_eq!(i.left, -1.5, "it reaches left of the pen");
        assert_eq!(i.right, 5.5, "and past its own advance of 4");
        let row = Node::Row(vec![glyph(3.0, 7.0, 0.0), hook]);
        let i = row.ink();
        assert_eq!(i.left, 0.0, "the first glyph starts at the origin");
        assert_eq!(i.right, 8.5, "pen 3, bearing -1.5, ink 7");
    }

    /// The shape a fraction will take in stage 3, laid out by hand here
    /// to prove the type already holds it: numerator, bar, denominator,
    /// with the bar owning the baseline.
    #[test]
    fn a_stack_hangs_off_its_base_item() {
        let frac = Node::Stack(Stack {
            items: vec![
                glyph(4.0, 6.0, 0.0),
                Node::Rule(Metrics {
                    width: 5.0,
                    height: 1.0,
                    depth: 0.0,
                }),
                glyph(5.0, 6.0, 0.0),
            ],
            base: 1,
            gap: 2.0,
        });
        let m = frac.metrics();
        assert_eq!(m.width, 5.0, "the widest item");
        assert_eq!(m.height, 1.0 + 6.0 + 2.0, "bar, then the numerator above");
        assert_eq!(m.depth, 6.0 + 2.0, "the denominator below");
    }

    #[test]
    fn a_stack_with_a_base_out_of_range_does_not_panic() {
        let s = Node::Stack(Stack {
            items: vec![glyph(4.0, 6.0, 0.0)],
            base: 9,
            gap: 0.0,
        });
        assert_eq!(s.metrics().height, 6.0);
    }
}
