//! Easing: pure time-remaps applied to a span-local parameter.
//!
//! An [`Ease`] reshapes *when* a span's motion happens, never *where* it
//! goes — evaluation remaps the local parameter `u ∈ [0, 1]` before the
//! geodesic screw is taken, so the path is untouched (R-0001 AC3).

/// A time-remap for one keyframe span.
///
/// All built-ins are *anchored*: `f(0) = 0` and `f(1) = 1`. A
/// [`Ease::Custom`] curve must be anchored too — an unanchored curve
/// returns non-key motors at interior key times and breaks continuity at
/// span joins. This is a documented contract, not a checked one; inputs
/// and outputs of [`Ease::apply`] are clamped to `[0, 1]` regardless.
#[derive(Clone, Copy, Debug)]
pub enum Ease {
    /// The identity schedule: a constant-speed screw across the span.
    Linear,
    /// Hermite `3u² − 2u³`: eases into and out of the span.
    SmoothStep,
    /// Perlin `6u⁵ − 15u⁴ + 10u³`: like [`Ease::SmoothStep`] with zero
    /// second derivative at the ends.
    SmootherStep,
    /// A caller-supplied anchored curve (`f(0) = 0`, `f(1) = 1`).
    ///
    /// A plain `fn` pointer — not a closure — keeps tracks `Clone` and
    /// evaluation deterministic; a pure time-remap has no state to capture.
    Custom(fn(f64) -> f64),
}

impl Ease {
    /// Remap a span-local parameter; input and output are clamped to
    /// `[0, 1]`, with NaN clamping low to `0.0` (matching
    /// [`Track::eval`](crate::Track::eval)'s convention).
    ///
    /// The output clamp means even an ill-behaved [`Ease::Custom`] curve
    /// cannot push evaluation outside its span.
    #[must_use]
    pub fn apply(self, u: f64) -> f64 {
        let u = clamp01(u);
        let s = match self {
            Ease::Linear => u,
            Ease::SmoothStep => u * u * (3.0 - 2.0 * u),
            Ease::SmootherStep => u * u * u * (u * (u * 6.0 - 15.0) + 10.0),
            Ease::Custom(f) => f(u),
        };
        clamp01(s)
    }
}

/// Clamp to `[0, 1]`; NaN clamps low (`f64::clamp` would propagate it).
fn clamp01(x: f64) -> f64 {
    if x.is_nan() {
        0.0
    } else {
        x.clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::Ease;

    /// Every built-in is anchored: `f(0) = 0`, `f(1) = 1`, exactly.
    #[test]
    fn built_ins_are_anchored() {
        for e in [Ease::Linear, Ease::SmoothStep, Ease::SmootherStep] {
            assert_eq!(e.apply(0.0), 0.0);
            assert_eq!(e.apply(1.0), 1.0);
        }
    }

    /// Inputs outside `[0, 1]` (including NaN) clamp before the curve runs.
    #[test]
    fn inputs_clamp_including_nan() {
        assert_eq!(Ease::SmoothStep.apply(-3.0), 0.0);
        assert_eq!(Ease::SmoothStep.apply(2.0), 1.0);
        // NaN clamps low, matching `Track::eval`'s convention.
        assert_eq!(Ease::Linear.apply(f64::NAN), 0.0);
        // A Custom curve returning NaN is floored by the output clamp.
        assert_eq!(Ease::Custom(|_| f64::NAN).apply(0.5), 0.0);
    }
}
