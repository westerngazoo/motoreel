//! What happens when: the curl, the freeze, the orbit, the x-ray sweep.
//! Every value is a pure function of `t`, so any frame renders alone.

/// Clip length, seconds.
pub const DURATION: f64 = 5.0;
/// The freeze: the curl stops here, mid-rep.
const FREEZE: f64 = 1.5;
/// Elbow flexion at the start and at the freeze (degrees). 90° is the
/// largest lever arm of the load: the moment worth freezing.
const ELBOW: (f64, f64) = (10.0, 90.0);
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

/// The x-ray's state at one instant.
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
    /// Elbow flexion, radians.
    pub elbow: f64,
    /// Camera azimuth, radians.
    pub azimuth: f64,
    /// The x-ray (used by the x-ray clip only).
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
    // Accelerating into the freeze, so it cuts the motion at speed.
    let curl = ramp(t, (0.0, FREEZE)).powf(1.5);
    let elbow = (ELBOW.0 + (ELBOW.1 - ELBOW.0) * curl).to_radians();
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
        elbow,
        azimuth,
        xray,
        flash,
    }
}
