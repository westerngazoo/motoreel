//! Gradients: a colour that varies across a fill (R-0016, SPEC-0016 §2.1–2.4).
//!
//! Pure and std-only: validation, the frame, the offset of a point and the
//! colour at an offset. The sinks decide how to paint with it; nothing
//! here knows about pixels, files or the camera.

use std::fmt;

use crate::prim::{Pt2, Rgb};

/// The most stops a gradient holds (R-0016 AC1). Fixed so a gradient is
/// `Copy`, like the [`crate::Style`] that carries it.
pub const MAX_STOPS: usize = 8;

/// One colour stop: where along the gradient, what colour, how opaque.
///
/// Contracts, checked by the [`Gradient`] constructors: `offset` and
/// `alpha` finite and in `[0, 1]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stop {
    /// Position along the gradient, 0 at its start and 1 at its end.
    pub offset: f64,
    /// sRGB colour at this stop.
    pub colour: Rgb,
    /// Opacity at this stop, multiplied by the fill's own opacity.
    pub alpha: f64,
}

/// Which way a gradient's offset varies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GradientKind {
    /// Along an axis: constant on lines perpendicular to it.
    Linear,
    /// Out from a centre: constant on concentric circles.
    Radial,
}

/// Why a gradient could not be built (R-0016 AC2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GradientError {
    /// Fewer than 2 or more than [`MAX_STOPS`] stops.
    StopCount {
        /// How many were given.
        count: usize,
    },
    /// A stop's offset is non-finite or outside `[0, 1]`.
    StopOffset {
        /// The offending stop.
        index: usize,
    },
    /// A stop's offset is smaller than its predecessor's.
    StopsUnsorted {
        /// The offending stop.
        index: usize,
    },
    /// A stop's opacity is non-finite or outside `[0, 1]`.
    StopAlpha {
        /// The offending stop.
        index: usize,
    },
    /// A point or the radius is non-finite, or the frame they span
    /// overflows.
    NonFinite,
    /// A linear gradient's two points coincide.
    DegenerateAxis,
    /// A radial gradient's radius is zero or negative.
    NonPositiveRadius,
}

impl fmt::Display for GradientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GradientError::StopCount { count } => {
                write!(f, "a gradient needs 2 to {MAX_STOPS} stops, got {count}")
            }
            GradientError::StopOffset { index } => {
                write!(f, "stop {index}: offset must be finite and in [0, 1]")
            }
            GradientError::StopsUnsorted { index } => {
                write!(
                    f,
                    "stop {index}: offset is smaller than the previous stop's"
                )
            }
            GradientError::StopAlpha { index } => {
                write!(f, "stop {index}: opacity must be finite and in [0, 1]")
            }
            GradientError::NonFinite => f.write_str("gradient geometry must be finite"),
            GradientError::DegenerateAxis => {
                f.write_str("a linear gradient's two points must differ")
            }
            GradientError::NonPositiveRadius => {
                f.write_str("a radial gradient's radius must be positive")
            }
        }
    }
}

impl std::error::Error for GradientError {}

/// A linear or radial gradient with 2 to [`MAX_STOPS`] stops and pad
/// spread (R-0016 AC1).
///
/// Built only by [`Gradient::linear`] and [`Gradient::radial`], which
/// validate, so every value of this type is a valid gradient. Its geometry
/// is a **frame** of three points (SPEC-0016 §2.2): offset-space `(0, 0)`,
/// `(1, 0)` and `(0, 1)`. Authored in the model plane; [`crate::Scene::eval`]
/// carries the frame into image space with the object it fills.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gradient {
    kind: GradientKind,
    origin: Pt2,
    x_end: Pt2,
    y_end: Pt2,
    /// Slots past `len` repeat the last stop, so the derived `PartialEq`
    /// compares meaning, not leftovers.
    stops: [Stop; MAX_STOPS],
    len: usize,
}

impl Gradient {
    /// A linear gradient: offset 0 at `p0`, 1 at `p1`, constant on lines
    /// perpendicular to `p0 → p1`.
    ///
    /// # Errors
    ///
    /// The stop errors of [`GradientError`], in stop order; then
    /// [`GradientError::NonFinite`] for a non-finite point or an
    /// overflowing axis, and [`GradientError::DegenerateAxis`] if
    /// `p0 == p1`.
    pub fn linear(p0: Pt2, p1: Pt2, stops: &[Stop]) -> Result<Self, GradientError> {
        let stops = checked_stops(stops)?;
        if !(finite(p0) && finite(p1)) {
            return Err(GradientError::NonFinite);
        }
        if p0 == p1 {
            return Err(GradientError::DegenerateAxis);
        }
        let (dx, dy) = (p1.x - p0.x, p1.y - p0.y);
        let y_end = Pt2 {
            x: p0.x - dy,
            y: p0.y + dx,
        };
        Gradient::framed(GradientKind::Linear, p0, p1, y_end, stops)
    }

    /// A radial gradient: offset 0 at `centre`, 1 on the circle of
    /// `radius`, constant on concentric circles.
    ///
    /// # Errors
    ///
    /// The stop errors of [`GradientError`], in stop order; then
    /// [`GradientError::NonFinite`] for a non-finite centre or radius, and
    /// [`GradientError::NonPositiveRadius`] if `radius ≤ 0`.
    pub fn radial(centre: Pt2, radius: f64, stops: &[Stop]) -> Result<Self, GradientError> {
        let stops = checked_stops(stops)?;
        if !(finite(centre) && radius.is_finite()) {
            return Err(GradientError::NonFinite);
        }
        if radius <= 0.0 {
            return Err(GradientError::NonPositiveRadius);
        }
        let x_end = Pt2 {
            x: centre.x + radius,
            y: centre.y,
        };
        let y_end = Pt2 {
            x: centre.x,
            y: centre.y + radius,
        };
        Gradient::framed(GradientKind::Radial, centre, x_end, y_end, stops)
    }

    /// Linear or radial.
    pub fn kind(&self) -> GradientKind {
        self.kind
    }

    /// The frame `(origin, x_end, y_end)`: the points at offset-space
    /// `(0, 0)`, `(1, 0)` and `(0, 1)` (SPEC-0016 §2.2).
    pub fn frame(&self) -> (Pt2, Pt2, Pt2) {
        (self.origin, self.x_end, self.y_end)
    }

    /// The stops, in order, exactly as given.
    pub fn stops(&self) -> &[Stop] {
        &self.stops[..self.len]
    }

    /// The offset at point `q`: the axis coordinate (linear) or the
    /// distance in radii (radial). Not clamped; [`Gradient::paint_at`]
    /// pads.
    ///
    /// `None` when the frame is singular, which a constructed gradient
    /// never is but a projected one can be (an object seen edge-on); such
    /// a gradient paints nothing (SPEC-0016 §2.2).
    pub fn offset_at(&self, q: Pt2) -> Option<f64> {
        let (ux, uy) = (self.x_end.x - self.origin.x, self.x_end.y - self.origin.y);
        let (vx, vy) = (self.y_end.x - self.origin.x, self.y_end.y - self.origin.y);
        let det = ux * vy - uy * vx;
        if !(det.is_finite() && det != 0.0) {
            return None;
        }
        let (dx, dy) = (q.x - self.origin.x, q.y - self.origin.y);
        let a = (dx * vy - dy * vx) / det;
        Some(match self.kind {
            GradientKind::Linear => a,
            GradientKind::Radial => {
                let b = (ux * dy - uy * dx) / det;
                (a * a + b * b).sqrt()
            }
        })
    }

    /// The paint at offset `t`: sRGB channels in `[0, 255]` and the stop
    /// opacity, unpremultiplied (SPEC-0016 §2.3).
    ///
    /// Pad spread: at or before the first offset (and for NaN) the first
    /// stop, at or after the last offset the last, both exactly. Between
    /// them, linear in `t` within the span; at equal offsets the later
    /// stop wins.
    pub fn paint_at(&self, t: f64) -> ([f64; 3], f64) {
        let stops = self.stops();
        let (first, last) = (stops[0], stops[stops.len() - 1]);
        if t.is_nan() || t <= first.offset {
            return channels(first);
        }
        if t >= last.offset {
            return channels(last);
        }
        // Here first.offset < t < last.offset, so some later stop lies
        // beyond `t` and the span has positive width.
        let i = stops.iter().rposition(|s| s.offset <= t).unwrap_or(0);
        let (a, b) = (stops[i], stops[i + 1]);
        let s = (t - a.offset) / (b.offset - a.offset);
        let (ca, aa) = channels(a);
        let (cb, ab) = channels(b);
        let mix = |x: f64, y: f64| x + (y - x) * s;
        (
            [mix(ca[0], cb[0]), mix(ca[1], cb[1]), mix(ca[2], cb[2])],
            mix(aa, ab),
        )
    }

    /// The one stop every stop equals, if they all do: such a gradient is
    /// a flat colour (R-0016 AC6, §6).
    pub(crate) fn uniform(&self) -> Option<Stop> {
        let first = self.stops[0];
        self.stops()
            .iter()
            .all(|s| s.colour == first.colour && s.alpha == first.alpha)
            .then_some(first)
    }

    /// This gradient with its frame carried by `map`, stops unchanged;
    /// `None` if any frame point fails to map.
    ///
    /// Exact for an affine `map` (SPEC-0016 §2.6). The result may be
    /// singular, which [`Gradient::offset_at`] reports.
    pub(crate) fn map_frame(&self, map: impl Fn(Pt2) -> Option<Pt2>) -> Option<Gradient> {
        Some(Gradient {
            origin: map(self.origin)?,
            x_end: map(self.x_end)?,
            y_end: map(self.y_end)?,
            ..*self
        })
    }

    /// The shared tail of both constructors: reject a frame whose
    /// determinant overflows, then pack the stops.
    fn framed(
        kind: GradientKind,
        origin: Pt2,
        x_end: Pt2,
        y_end: Pt2,
        stops: &[Stop],
    ) -> Result<Self, GradientError> {
        let last = stops[stops.len() - 1];
        let mut packed = [last; MAX_STOPS];
        packed[..stops.len()].copy_from_slice(stops);
        let g = Gradient {
            kind,
            origin,
            x_end,
            y_end,
            stops: packed,
            len: stops.len(),
        };
        if g.offset_at(origin).is_none() {
            return Err(GradientError::NonFinite);
        }
        Ok(g)
    }
}

/// The stops if they are valid (R-0016 AC2), checked in order.
fn checked_stops(stops: &[Stop]) -> Result<&[Stop], GradientError> {
    if !(2..=MAX_STOPS).contains(&stops.len()) {
        return Err(GradientError::StopCount { count: stops.len() });
    }
    let unit = |x: f64| (0.0..=1.0).contains(&x); // false for NaN
    for (index, s) in stops.iter().enumerate() {
        if !unit(s.offset) {
            return Err(GradientError::StopOffset { index });
        }
        if index > 0 && s.offset < stops[index - 1].offset {
            return Err(GradientError::StopsUnsorted { index });
        }
        if !unit(s.alpha) {
            return Err(GradientError::StopAlpha { index });
        }
    }
    Ok(stops)
}

/// A stop's colour as `f64` channels, with its opacity.
fn channels(s: Stop) -> ([f64; 3], f64) {
    (
        [
            f64::from(s.colour.r),
            f64::from(s.colour.g),
            f64::from(s.colour.b),
        ],
        s.alpha,
    )
}

fn finite(p: Pt2) -> bool {
    p.x.is_finite() && p.y.is_finite()
}

#[cfg(test)]
mod tests {
    use super::{Gradient, Stop};
    use crate::prim::{Pt2, Rgb};

    fn two() -> [Stop; 2] {
        [
            Stop {
                offset: 0.0,
                colour: Rgb::BLACK,
                alpha: 1.0,
            },
            Stop {
                offset: 1.0,
                colour: Rgb::WHITE,
                alpha: 1.0,
            },
        ]
    }

    /// A frame collapsed onto a line is singular: no offset anywhere.
    #[test]
    fn a_collapsed_frame_has_no_offset() {
        let g = Gradient::radial(Pt2 { x: 0.0, y: 0.0 }, 1.0, &two()).expect("valid");
        let flat = g.map_frame(|p| Some(Pt2 { x: p.x, y: 0.0 })).expect("maps");
        assert_eq!(flat.offset_at(Pt2 { x: 0.5, y: 0.0 }), None);
    }

    /// A failing frame point fails the whole map.
    #[test]
    fn a_frame_point_that_does_not_map_fails_the_map() {
        let g = Gradient::radial(Pt2 { x: 0.0, y: 0.0 }, 1.0, &two()).expect("valid");
        assert!(g.map_frame(|p| (p.y == 0.0).then_some(p)).is_none());
    }

    #[test]
    fn uniform_means_every_stop_is_equal() {
        let mut s = two();
        let g = Gradient::radial(Pt2 { x: 0.0, y: 0.0 }, 1.0, &s).expect("valid");
        assert_eq!(g.uniform(), None);
        s[1].colour = Rgb::BLACK;
        let g = Gradient::radial(Pt2 { x: 0.0, y: 0.0 }, 1.0, &s).expect("valid");
        assert_eq!(g.uniform(), Some(s[0]));
    }
}
