//! One key light and an ambient floor: Lambert per face.

use crate::vec3::{dot, normalize, V3};

/// A directional key light, fixed in the world (it stays put while the
/// camera orbits, which is what makes the orbit read as 3D).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    /// Unit direction **toward** the light.
    dir: V3,
    /// The light term of a face turned fully away, in `[0, 1]`.
    ambient: f64,
}

impl Light {
    /// A light from direction `toward` (any length) with floor `ambient`.
    /// `None` for a zero or non-finite direction.
    #[must_use]
    pub fn new(toward: V3, ambient: f64) -> Option<Light> {
        Some(Light {
            dir: normalize(toward)?,
            ambient: ambient.clamp(0.0, 1.0),
        })
    }

    /// The unit direction toward the light.
    #[must_use]
    pub fn dir(&self) -> V3 {
        self.dir
    }

    /// Lambert: `ambient + (1 − ambient)·max(0, n·l)` for unit normal `n`.
    #[must_use]
    pub fn term(&self, n: V3) -> f64 {
        self.ambient + (1.0 - self.ambient) * dot(n, self.dir).max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lambert_spans_ambient_to_one() {
        let l = Light::new([0.0, 2.0, 0.0], 0.3).unwrap();
        assert_eq!(l.term([0.0, 1.0, 0.0]), 1.0);
        assert_eq!(l.term([0.0, -1.0, 0.0]), 0.3);
        assert!(Light::new([0.0; 3], 0.3).is_none());
    }
}
