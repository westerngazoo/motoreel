//! An orbit camera built from motors, handed to motoreel's own [`Camera`].
//!
//! The pose is a product of motors (a translator to the target, a turn
//! about the vertical, a tilt, a dolly back), so an orbit is an angle and
//! nothing else; motoreel's pinhole does the divide.

use garust::{Motor3, Pga3};
use motoreel::Camera;

use crate::vec3::V3;

/// The `e13` plane: rotation about the y axis.
fn plane_y() -> Pga3 {
    Pga3::basis(0b0101)
}

/// The `e23` plane: rotation about the x axis.
fn plane_x() -> Pga3 {
    Pga3::basis(0b0110)
}

/// A right-handed turn of `radians` about +y (carries +z toward +x).
///
/// garust's rotor turns the other way for this plane (`e13` is the
/// reversed one), so the sign is flipped here once; all three turns are
/// pinned by a test, as garust's `rotation_about` docs advise.
pub fn turn_y(radians: f64) -> Motor3 {
    Motor3::rotor(-radians, plane_y())
}

/// A right-handed turn of `radians` about +x (carries +y toward +z).
pub fn turn_x(radians: f64) -> Motor3 {
    Motor3::rotor(radians, plane_x())
}

/// A right-handed turn of `radians` about +z (carries +x toward +y).
pub fn turn_z(radians: f64) -> Motor3 {
    Motor3::rotor(radians, Pga3::basis(0b0011))
}

/// A camera circling `target`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Orbit {
    /// The point looked at (world).
    pub target: V3,
    /// Distance from the target.
    pub distance: f64,
    /// Angle about +y from the +z axis: 0 looks from +z, π/2 from +x.
    pub azimuth: f64,
    /// Angle above the horizon (positive looks down).
    pub elevation: f64,
    /// Pinhole focal length, in image units (pixels when the view is).
    pub focal: f64,
}

impl Orbit {
    /// The motor pose: target, then the turn, then the tilt, then the
    /// dolly back along the camera's +z (it looks along −z).
    pub fn pose(&self) -> Motor3 {
        let [x, y, z] = self.target;
        Motor3::translator(x, y, z)
            .compose(&turn_y(self.azimuth))
            .compose(&turn_x(-self.elevation))
            .compose(&Motor3::translator(0.0, 0.0, self.distance))
    }

    /// motoreel's camera at this pose.
    #[must_use]
    pub fn camera(&self) -> Camera {
        Camera::pinhole(self.pose(), self.focal)
    }

    /// The eye position in the world.
    #[must_use]
    pub fn eye(&self) -> V3 {
        let m = self.pose().to_matrix();
        [m[3][0], m[3][1], m[3][2]]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec3::transform;

    fn close(a: V3, b: V3) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-12)
    }

    #[test]
    fn turns_are_right_handed() {
        let q = std::f64::consts::FRAC_PI_2;
        assert!(close(
            transform(&turn_y(q).to_matrix(), [0.0, 0.0, 1.0]),
            [1.0, 0.0, 0.0]
        ));
        assert!(close(
            transform(&turn_x(q).to_matrix(), [0.0, 1.0, 0.0]),
            [0.0, 0.0, 1.0]
        ));
        assert!(close(
            transform(&turn_z(q).to_matrix(), [1.0, 0.0, 0.0]),
            [0.0, 1.0, 0.0]
        ));
    }

    #[test]
    fn the_orbit_eye_circles_the_target() {
        let o = Orbit {
            target: [0.0, 1.0, 0.0],
            distance: 4.0,
            azimuth: 0.0,
            elevation: 0.0,
            focal: 1.0,
        };
        assert!(close(o.eye(), [0.0, 1.0, 4.0]));
        let o = Orbit {
            azimuth: std::f64::consts::FRAC_PI_2,
            ..o
        };
        assert!(close(o.eye(), [4.0, 1.0, 0.0]));
        let up = Orbit {
            elevation: 0.3,
            ..o
        };
        assert!(up.eye()[1] > 1.0);
    }
}
