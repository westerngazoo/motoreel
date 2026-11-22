//! What a surface is made of: three palette tones and the layer it lives in.
//!
//! The tones are the house's (`monito-*-luz`, base, `-sombra`): the caller
//! passes palette tokens, this crate never names a colour of its own.

use motoreel::Rgb;

/// A surface's three tones: lit side, base, shadow side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tones {
    /// The lit side.
    pub light: Rgb,
    /// The base colour.
    pub base: Rgb,
    /// The shadow side.
    pub shadow: Rgb,
}

impl Tones {
    /// The colour at light term `k` in `[0, 1]`: shadow at 0, base at
    /// [`Tones::BASE_AT`], light at 1, linear in sRGB between them.
    #[must_use]
    pub fn at(&self, k: f64) -> Rgb {
        let k = if k.is_nan() { 0.0 } else { k.clamp(0.0, 1.0) };
        if k < Self::BASE_AT {
            mix(self.shadow, self.base, k / Self::BASE_AT)
        } else {
            mix(
                self.base,
                self.light,
                (k - Self::BASE_AT) / (1.0 - Self::BASE_AT),
            )
        }
    }

    /// Where on the light term the base tone sits.
    pub const BASE_AT: f64 = 0.55;
}

/// `a` to `b` by `t`, per channel, rounded.
#[must_use]
pub fn mix(a: Rgb, b: Rgb, t: f64) -> Rgb {
    let c = |x: u8, y: u8| {
        (f64::from(x) + (f64::from(y) - f64::from(x)) * t)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Rgb {
        r: c(a.r, b.r),
        g: c(a.g, b.g),
        b: c(a.b, b.b),
    }
}

/// Which layer a surface belongs to: what each finish does with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    /// The toy's skin and clothes: opaque outside, glass in x-ray.
    Body,
    /// Props and set (dumbbell, platform): opaque in every finish.
    Object,
    /// What is inside the body (bone, the biceps line): x-ray only.
    Inside,
    /// The ink hull of a body part (inverted hull): back faces only.
    BodyOutline,
    /// The ink hull of an object.
    ObjectOutline,
}

impl Layer {
    /// Whether the pipeline keeps the faces turned **away** from the
    /// camera (an inverted hull) instead of the ones facing it.
    #[must_use]
    pub fn is_outline(self) -> bool {
        matches!(self, Layer::BodyOutline | Layer::ObjectOutline)
    }
}

/// A surface: its tones and its layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Material {
    /// The three tones.
    pub tones: Tones,
    /// What each finish does with it.
    pub layer: Layer,
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: Tones = Tones {
        light: Rgb {
            r: 200,
            g: 200,
            b: 200,
        },
        base: Rgb {
            r: 100,
            g: 100,
            b: 100,
        },
        shadow: Rgb { r: 0, g: 0, b: 0 },
    };

    #[test]
    fn ramp_hits_its_three_tones() {
        assert_eq!(T.at(0.0), T.shadow);
        assert_eq!(T.at(Tones::BASE_AT), T.base);
        assert_eq!(T.at(1.0), T.light);
        assert_eq!(T.at(-3.0), T.shadow);
        assert_eq!(T.at(f64::NAN), T.shadow);
    }
}
