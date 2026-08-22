//! The eye: a motor pose plus the only non-motor step in the pipeline.

use garust::{pga, Motor3};

use crate::prim::Pt2;

/// How view-space geometry maps to the image plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Projection {
    /// Perspective through the camera center onto a plane `focal` ahead:
    /// image = `(focal·x/d, focal·y/d)` with view depth `d = −z`.
    /// Contract: `focal` finite and > 0 (documented, not validated — the
    /// cull guard keeps output coordinates finite regardless).
    Pinhole {
        /// Center-of-projection to image-plane distance, in world units.
        focal: f64,
    },
    /// Parallel projection: image = view `(x, y)`; depth only culls.
    Orthographic,
}

impl Projection {
    /// Near-plane distance in view units: a vertex with depth below this
    /// (behind, at plane, or non-finite) culls its whole primitive.
    pub const NEAR: f64 = 1e-9;

    /// Project one **view-space** point to image space; `None` means
    /// culled. Never returns a non-finite coordinate (SPEC-0002 §2.5).
    pub fn project(&self, view_point: &pga::Point) -> Option<Pt2> {
        let (x, y, z) = view_point.to_euclidean();
        let d = -z; // the camera looks along −z (SPEC-0002 §2.1)
        if !d.is_finite() || d < Self::NEAR {
            return None; // behind / at plane / degenerate — cull, never NaN
        }
        let p = match *self {
            Projection::Pinhole { focal } => Pt2 {
                x: focal * x / d,
                y: focal * y / d,
            },
            Projection::Orthographic => Pt2 { x, y },
        };
        (p.x.is_finite() && p.y.is_finite()).then_some(p)
    }
}

/// The eye: a motor pose plus a projection rule.
///
/// The camera looks along its local −z with +y up; the view transform is
/// `pose.inverse()`. Camera moves are authored in the same currency as
/// object motion — a motor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// World-from-camera rigid pose — a motor, like every other motion.
    pub pose: Motor3,
    /// Pinhole or orthographic image formation.
    pub projection: Projection,
}

impl Camera {
    /// A pinhole camera at `pose` with the given focal length.
    pub fn pinhole(pose: Motor3, focal: f64) -> Self {
        Camera {
            pose,
            projection: Projection::Pinhole { focal },
        }
    }

    /// An orthographic camera at `pose` — the classic math-diagram look.
    ///
    /// Content must sit in front of the camera: the uniform cull rule
    /// (SPEC-0002 §2.5) applies to orthographic views too.
    pub fn orthographic(pose: Motor3) -> Self {
        Camera {
            pose,
            projection: Projection::Orthographic,
        }
    }
}

impl Default for Camera {
    /// Orthographic, standing 5 units back on +z, looking at the origin.
    fn default() -> Self {
        Camera::orthographic(Motor3::translator(0.0, 0.0, 5.0))
    }
}

#[cfg(test)]
mod tests {
    use super::Projection;
    use garust::pga;

    /// Hand-computed divide and non-divide on the same view-space point.
    #[test]
    fn projection_rules_are_hand_exact() {
        let p = pga::Point::new(1.0, 2.0, -4.0);
        let pin = Projection::Pinhole { focal: 2.0 }.project(&p).unwrap();
        assert_eq!((pin.x, pin.y), (0.5, 1.0));
        let ort = Projection::Orthographic.project(&p).unwrap();
        assert_eq!((ort.x, ort.y), (1.0, 2.0));
        assert!(Projection::Orthographic
            .project(&pga::Point::new(0.0, 0.0, 1.0))
            .is_none());
    }
}
