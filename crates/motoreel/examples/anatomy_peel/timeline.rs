//! What happens when: the orbit, the peel, the lit groups, the sweep from
//! muscle to bone and the lever. The curl is the forearms' tracks
//! (`body.rs`). Every value is a pure function of `t`.

use crate::body::Moment;

/// Clip length, seconds.
pub const DURATION: f64 = 9.0;
/// The orbit (degrees; 0 is the right-side profile, 90 the front).
const ORBIT_DEG: (f64, f64) = (20.0, 160.0);
/// The skin's peel, start and end (seconds).
const PEEL: (f64, f64) = (0.8, 3.0);
/// The biceps lit in the load colour.
const BICEPS: (f64, f64) = (3.2, 3.8);
/// The sweep from muscle to bone, across this image-space span.
const SWEEP: (f64, f64) = (4.8, 5.6);
const SWEEP_X: f64 = 560.0;
/// The forearm bones lit, and the lever drawn.
const FOREARM: (f64, f64) = (5.8, 6.4);
const LEVER: (f64, f64) = (6.0, 6.6);

/// The finish at one instant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Look {
    /// Skin and muscle, faceted.
    Faceted,
    /// The sweep line at image x: x-ray to its left.
    Sweep(f64),
    /// Muscle as glass, bone shown.
    XRay,
}

/// One instant.
pub struct State {
    /// Camera azimuth, radians.
    pub azimuth: f64,
    pub look: Look,
    pub moment: Moment,
}

/// `0` before `a`, `1` after `b`, linear between.
fn ramp(t: f64, (a, b): (f64, f64)) -> f64 {
    ((t - a) / (b - a)).clamp(0.0, 1.0)
}

/// Smooth start and stop.
fn ease(s: f64) -> f64 {
    s * s * (3.0 - 2.0 * s)
}

/// The state at `t` seconds.
pub fn at(t: f64) -> State {
    let orbit = ease(ramp(t, (0.0, DURATION)));
    let azimuth = (ORBIT_DEG.0 + (ORBIT_DEG.1 - ORBIT_DEG.0) * orbit).to_radians();
    let look = if t < SWEEP.0 {
        Look::Faceted
    } else if t < SWEEP.1 {
        Look::Sweep(-SWEEP_X + 2.0 * SWEEP_X * ease(ramp(t, SWEEP)))
    } else {
        Look::XRay
    };
    State {
        azimuth,
        look,
        moment: Moment {
            // The skin goes once every facet has faded.
            peel: (t < PEEL.1).then(|| ramp(t, PEEL)),
            biceps: ease(ramp(t, BICEPS)),
            forearm: ease(ramp(t, FOREARM)),
            lever: ease(ramp(t, LEVER)),
        },
    }
}
