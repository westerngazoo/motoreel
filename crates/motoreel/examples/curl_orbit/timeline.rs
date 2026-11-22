//! What happens when: the orbit, the x-ray sweep, the freeze flash. The
//! curl itself is the forearms' motor track (`rig.rs`). Every value is a
//! pure function of `t`, so any frame renders alone.

use crate::rig::FREEZE;

/// Clip length, seconds.
pub const DURATION: f64 = 5.0;
/// The orbit: when, and from which azimuth to which (degrees; 0 is the
/// right-side profile, 90 the front).
const ORBIT_T: (f64, f64) = (1.7, 4.4);
const ORBIT_DEG: (f64, f64) = (15.0, 165.0);
/// The x-ray sweep in and out (seconds), across this image-space span.
const SWEEP_IN: (f64, f64) = (1.9, 2.5);
const SWEEP_OUT: (f64, f64) = (3.9, 4.5);
const SWEEP_X: f64 = 560.0;
/// The freeze flash's length (seconds).
const FLASH: f64 = 0.25;

/// The x-ray at one instant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Xray {
    /// Outside only.
    Off,
    /// The sweep line at image x: x-ray to its left.
    Sweep(f64),
    /// Inside only.
    On,
}

/// One instant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct State {
    /// Camera azimuth, radians.
    pub azimuth: f64,
    /// The x-ray.
    pub xray: Xray,
    /// The freeze flash's opacity.
    pub flash: f64,
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
    let orbit = ease(ramp(t, ORBIT_T));
    let azimuth = (ORBIT_DEG.0 + (ORBIT_DEG.1 - ORBIT_DEG.0) * orbit).to_radians();
    let xray = if t < SWEEP_IN.0 || t >= SWEEP_OUT.1 {
        Xray::Off
    } else if t < SWEEP_IN.1 {
        Xray::Sweep(-SWEEP_X + 2.0 * SWEEP_X * ease(ramp(t, SWEEP_IN)))
    } else if t < SWEEP_OUT.0 {
        Xray::On
    } else {
        Xray::Sweep(SWEEP_X - 2.0 * SWEEP_X * ease(ramp(t, SWEEP_OUT)))
    };
    let flash = if t >= FREEZE {
        0.35 * (1.0 - ramp(t, (FREEZE, FREEZE + FLASH)))
    } else {
        0.0
    };
    State {
        azimuth,
        xray,
        flash,
    }
}
