//! Paths: outlines made of straight and cubic Bézier pieces (SPEC-0012 §2.1).
//!
//! Generic over the point type, and naming none: one definition serves
//! model space (`pga::Point`), image space ([`crate::Pt2`]) and the raster
//! sink's pixel space. Depends on `std` alone and imports nothing from the
//! crate — the 2D vocabulary imports [`Subpath`] from here, so the module
//! graph stays acyclic (architect finding 4).
//!
//! A path is a `Vec<Subpath<P>>`. There is no wrapper type: a wrapper with
//! no invariant of its own would be ceremony.

/// One piece of a subpath, ending at its last point. It starts wherever
/// the previous piece ended (or at [`Subpath::start`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Seg<P> {
    /// A straight line to the point.
    Line(P),
    /// A cubic Bézier: first handle, second handle, then the end point.
    Cubic(P, P, P),
}

/// A connected outline: a start point and the segments that follow it.
#[derive(Clone, Debug, PartialEq)]
pub struct Subpath<P> {
    /// Where the first segment begins.
    pub start: P,
    /// The pieces, in drawing order.
    pub segs: Vec<Seg<P>>,
    /// Whether a stroke draws the closing segment back to `start`.
    ///
    /// For **fill**, every subpath is treated as closed, which is SVG's
    /// behaviour (R-0012 AC3).
    pub closed: bool,
}

impl<P: Copy> Subpath<P> {
    /// An open subpath at `start` with no segments yet.
    pub fn new(start: P) -> Self {
        Subpath {
            start,
            segs: Vec::new(),
            closed: false,
        }
    }

    /// Append a straight line to `end` (builder).
    #[must_use]
    pub fn line_to(mut self, end: P) -> Self {
        self.segs.push(Seg::Line(end));
        self
    }

    /// Append a cubic Bézier through handles `h1`, `h2` to `end` (builder).
    #[must_use]
    pub fn cubic_to(mut self, h1: P, h2: P, end: P) -> Self {
        self.segs.push(Seg::Cubic(h1, h2, end));
        self
    }

    /// Mark the subpath closed (builder). Adds no segment: the closing
    /// side runs from the last point back to `start`.
    #[must_use]
    pub fn close(mut self) -> Self {
        self.closed = true;
        self
    }

    /// The same outline with every control point passed through `f`,
    /// handles included — the one way a subpath changes point type. Eval
    /// uses it to carry control points into view and image space (§2.4).
    pub fn map<Q>(&self, f: impl Fn(&P) -> Q) -> Subpath<Q> {
        Subpath {
            start: f(&self.start),
            segs: self
                .segs
                .iter()
                .map(|seg| match seg {
                    Seg::Line(end) => Seg::Line(f(end)),
                    Seg::Cubic(h1, h2, end) => Seg::Cubic(f(h1), f(h2), f(end)),
                })
                .collect(),
            closed: self.closed,
        }
    }

    /// The same outline traversed backwards: segments in reverse order,
    /// each cubic's handles swapped, `closed` kept.
    ///
    /// Under the nonzero rule this is how an author makes a hole: a
    /// subpath inside another of the opposite orientation cancels its
    /// winding (R-0012 AC3).
    #[must_use]
    pub fn reversed(&self) -> Self {
        // The on-curve points in drawing order: `ends[i]` is where segment
        // `i` begins, and the last one is where the subpath ends.
        let ends: Vec<P> = std::iter::once(self.start)
            .chain(self.segs.iter().map(|seg| match *seg {
                Seg::Line(end) | Seg::Cubic(_, _, end) => end,
            }))
            .collect();
        let segs = self
            .segs
            .iter()
            .zip(&ends)
            .rev()
            .map(|(seg, &from)| match *seg {
                Seg::Line(_) => Seg::Line(from),
                Seg::Cubic(h1, h2, _) => Seg::Cubic(h2, h1, from),
            })
            .collect();
        Subpath {
            start: ends[self.segs.len()],
            segs,
            closed: self.closed,
        }
    }
}

/// De Casteljau halving at `t = ½`: the control points of the cubic's two
/// halves, which meet at `B(½)`.
///
/// `mid` is the caller's midpoint, so any point type works. Built from
/// midpoints alone, the split is exact up to their rounding. Pinhole
/// projection subdivides view-space `(x, y, z)` tuples with it (§2.4).
pub fn split_cubic<P: Copy>(p: [P; 4], mid: impl Fn(&P, &P) -> P) -> ([P; 4], [P; 4]) {
    let p01 = mid(&p[0], &p[1]);
    let p12 = mid(&p[1], &p[2]);
    let p23 = mid(&p[2], &p[3]);
    let p012 = mid(&p01, &p12);
    let p123 = mid(&p12, &p23);
    let half = mid(&p012, &p123);
    ([p[0], p01, p012, half], [half, p123, p23, p[3]])
}

/// The cubic Bernstein weights at `t`: `B(t) = Σ wᵢ·pᵢ` over the control
/// points `p₀…p₃`, per coordinate.
///
/// Plain `+ − ×` with no `mul_add` (SPEC-0012 §2.10), so every place that
/// evaluates a cubic — pinhole error probes, raster chords — spells it the
/// same way and gets the same bits.
pub(crate) fn bernstein(t: f64) -> [f64; 4] {
    let u = 1.0 - t;
    [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t]
}
