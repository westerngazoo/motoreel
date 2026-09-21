//! The one place a coordinate stops being an `f64`.
//!
//! Type is measured in `f64` because layout arithmetic accumulates, and
//! rasterized against an `f32` API on an integer grid. Both narrowings are
//! deliberate and bounded, and clippy is right to ask about them — so they
//! are asked and answered **here**, once, instead of being waved through at
//! seven call sites.

use ab_glyph::PxScale;

/// An em size as the rasterizer's scale.
///
/// Sizes are display type on a 1080 × 1920 canvas — tens, not millions —
/// so `f32` holds them exactly enough that a glyph outline is unaffected.
/// A size large enough to lose precision here would already be many times
/// the canvas.
#[allow(
    clippy::cast_possible_truncation,
    reason = "em sizes are canvas-scale; f32 is the rasterizer's own unit"
)]
#[must_use]
pub(crate) fn px(size: f64) -> PxScale {
    PxScale::from(size as f32)
}

/// Whole-pixel part as an integer, fractional part in `[0, 1)`.
///
/// Splitting this way keeps the `f32` handed to the rasterizer inside one
/// pixel, so a glyph at the bottom of a 1920-tall frame is positioned as
/// precisely as one at the top.
#[allow(
    clippy::cast_possible_truncation,
    reason = "the integer part of a canvas coordinate; the fraction is kept separately"
)]
#[must_use]
pub(crate) fn split(v: f64) -> (i64, f32) {
    let i = v.floor();
    (i as i64, (v - i) as f32)
}

/// A pixel-grid coordinate the rasterizer reported, as an integer.
#[allow(
    clippy::cast_possible_truncation,
    reason = "px_bounds is already a pixel grid; the value is integral by construction"
)]
#[must_use]
pub(crate) fn grid(v: f32) -> i64 {
    v as i64
}

/// A canvas coordinate rounded to the nearest pixel — for rules, whose
/// edges must be hard.
#[allow(
    clippy::cast_possible_truncation,
    reason = "canvas coordinates are thousands at most"
)]
#[must_use]
pub(crate) fn round(v: f64) -> i64 {
    v.round() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitting_keeps_the_fraction_in_range() {
        for v in [0.0, 1.0, 2.5, -0.25, -3.75, 1919.9] {
            let (i, f) = split(v);
            assert!((0.0..1.0).contains(&f), "{v} -> {i}, {f}");
            let back = f64::from(i32::try_from(i).unwrap()) + f64::from(f);
            assert!((back - v).abs() < 1e-6, "{v}");
        }
    }

    #[test]
    fn rounding_goes_to_the_nearest_pixel() {
        assert_eq!(round(10.4), 10);
        assert_eq!(round(10.6), 11);
        assert_eq!(round(-0.4), 0);
    }
}
