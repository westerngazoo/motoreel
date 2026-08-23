//! Recording a physics rollout into ordinary motor tracks.
//!
//! This is the only module in the library that names `garust::physics`.
//! Everything downstream — objects, scenes, sinks — sees a [`Track`] and
//! cannot tell a simulated one from an authored one, which is the whole
//! point: keyframed and simulated motion are the same currency.

use core::fmt;

use garust::physics::world::{Body, Joint, World};
use garust::Motor3;

use crate::track::{Track, TrackError};

/// Why a rollout could not be recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordError {
    /// `dt` was not finite, or was not greater than zero.
    InvalidTimestep,
    /// The recorded keys did not form a valid track. Reachable only for
    /// an absurd `dt`, where `steps as f64 * dt` overflows to infinity.
    Track(TrackError),
}

impl fmt::Display for RecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecordError::InvalidTimestep => {
                write!(f, "timestep must be finite and > 0")
            }
            RecordError::Track(e) => {
                write!(f, "recorded keys did not form a track: {e}")
            }
        }
    }
}

impl std::error::Error for RecordError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RecordError::InvalidTimestep => None,
            RecordError::Track(e) => Some(e),
        }
    }
}

impl From<TrackError> for RecordError {
    fn from(e: TrackError) -> Self {
        RecordError::Track(e)
    }
}

/// Step `world` `steps` times at fixed `dt`, recording every body's pose
/// into a [`Track`] — one per body, in input order.
///
/// Key `i` sits at `i as f64 * dt` (derived, never accumulated) and holds
/// that body's pose after `i` steps, so key 0 is the initial pose before
/// any stepping and `steps` steps yield `steps + 1` keys. The caller's
/// `bodies` are left untouched: the rollout runs on an internal copy, so
/// recording twice from the same state reproduces the same tracks.
///
/// `joints` is passed straight to [`World::step`]; pass `&[]` when there
/// are none.
///
/// The result is ordinary track data. Nothing downstream learns that a
/// simulation produced it.
///
/// # Errors
///
/// [`RecordError::InvalidTimestep`] if `dt` is not finite and positive;
/// [`RecordError::Track`] if the derived key times do not form a valid
/// track, which requires `steps as f64 * dt` to overflow.
pub fn record(
    world: &World,
    bodies: &[Body],
    joints: &[Joint],
    dt: f64,
    steps: usize,
) -> Result<Vec<Track>, RecordError> {
    if !(dt.is_finite() && dt > 0.0) {
        return Err(RecordError::InvalidTimestep);
    }
    let mut state = bodies.to_vec();
    let mut keys: Vec<Vec<(f64, Motor3)>> = state.iter().map(|b| vec![(0.0, b.pose())]).collect();
    for i in 1..=steps {
        world.step(&mut state, joints, dt);
        let t = i as f64 * dt; // derived, never accumulated
        for (k, b) in keys.iter_mut().zip(state.iter()) {
            k.push((t, b.pose()));
        }
    }
    keys.into_iter().map(|k| Ok(Track::keys(k)?)).collect()
}

#[cfg(test)]
mod tests {
    use super::{record, RecordError};
    use garust::physics::world::{Body, World};

    /// A rollout leaves the caller's bodies exactly as they were.
    #[test]
    fn caller_state_is_untouched() {
        let world = World::new();
        let bodies = [Body::ball(1.0, 0.5)];
        let before = bodies;
        record(&world, &bodies, &[], 1.0 / 60.0, 10).expect("valid rollout");
        assert_eq!(bodies, before);
    }

    /// A non-positive or non-finite timestep is rejected before stepping.
    #[test]
    fn bad_timestep_is_rejected() {
        let world = World::new();
        let bodies = [Body::ball(1.0, 0.5)];
        for dt in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(
                record(&world, &bodies, &[], dt, 1).unwrap_err(),
                RecordError::InvalidTimestep
            );
        }
    }
}
