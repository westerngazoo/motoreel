//! Motor keyframe tracks: ordered `(time, Motor3)` keys, eased
//! geodesic-screw interpolation between them.
//!
//! A track does not care where its keys came from — sparse authored
//! keyframes and dense fixed-timestep samples recorded from a physics
//! rollout are the same data, evaluated by the same path (R-0001 AC8).

use core::fmt;
use std::f64::consts::TAU;

use garust::{Motor3, Pga3};

use crate::ease::Ease;

/// Why a [`Track`] could not be built or edited.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackError {
    /// [`Track::keys`] was given no keys.
    Empty,
    /// Key times must form a finite, strictly increasing sequence.
    /// `index` is the first key that breaks it: its own index for a
    /// non-finite time, the second key of a non-increasing pair.
    NonIncreasing {
        /// Index of the first chain-breaking key.
        index: usize,
    },
    /// [`Track::ease_span`] addressed a span past the last one.
    NoSuchSpan {
        /// The requested span index.
        index: usize,
    },
    /// [`Track::spin`] was given a non-finite rate, a non-finite or
    /// non-positive duration, or a plane whose Euclidean part has zero
    /// norm (there is no rotation rate about an ideal plane).
    InvalidSpin,
}

impl fmt::Display for TrackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrackError::Empty => write!(f, "track needs at least one key"),
            TrackError::NonIncreasing { index } => write!(
                f,
                "key time at index {index} is not finite or not greater \
                 than its predecessor"
            ),
            TrackError::NoSuchSpan { index } => {
                write!(f, "track has no span {index}")
            }
            TrackError::InvalidSpin => write!(
                f,
                "spin needs a finite rate, a finite positive duration, and \
                 a plane with a non-zero Euclidean part"
            ),
        }
    }
}

impl std::error::Error for TrackError {}

/// A motor keyframe track: `(time, Motor3)` keys with a per-span [`Ease`].
///
/// Evaluation is total ([`Track::eval`] clamps outside the keyed range)
/// and deterministic — identical inputs produce bit-identical motors.
/// Keys are renormalized once at construction, so a slightly drifted
/// motor (e.g. from a long physics rollout) can never trip garust's
/// unit-versor debug assertions inside a render loop.
///
/// Structural invariant, established by every constructor and never
/// broken: `keys` is non-empty with finite, strictly increasing times,
/// and `eases.len() == keys.len() - 1`.
///
/// Deliberately no `PartialEq`: [`Ease::Custom`] carries a `fn` pointer,
/// and function-pointer address equality is codegen-dependent.
#[derive(Clone, Debug)]
pub struct Track {
    keys: Vec<(f64, Motor3)>,
    eases: Vec<Ease>,
}

impl Track {
    /// Build a track from `(time, pose)` keys; every span starts
    /// [`Ease::Linear`].
    ///
    /// Key times must be finite and strictly increasing (NaN never
    /// compares as increasing and is reported by its own index). Keys are
    /// renormalized on ingest.
    pub fn keys(keys: impl IntoIterator<Item = (f64, Motor3)>) -> Result<Track, TrackError> {
        let keys: Vec<(f64, Motor3)> = keys
            .into_iter()
            .map(|(t, m)| (t, m.renormalize()))
            .collect();
        if keys.is_empty() {
            return Err(TrackError::Empty);
        }
        for (index, (t, _)) in keys.iter().enumerate() {
            if !t.is_finite() {
                return Err(TrackError::NonIncreasing { index });
            }
        }
        for index in 1..keys.len() {
            if keys[index - 1].0 >= keys[index].0 {
                return Err(TrackError::NonIncreasing { index });
            }
        }
        let eases = vec![Ease::Linear; keys.len() - 1];
        Ok(Track { keys, eases })
    }

    /// An infallible single-key track: a constant pose (clamping makes it
    /// hold everywhere).
    #[must_use]
    pub fn hold(pose: Motor3) -> Track {
        Track {
            keys: vec![(0.0, pose.renormalize())],
            eases: Vec::new(),
        }
    }

    /// Uniform rotation at `rate` radians/second about `plane`, for
    /// `duration` seconds.
    ///
    /// The plane bivector is normalized here — pass any non-zero multiple.
    /// Emits subdivided keys at most a quarter turn apart: a single slerp
    /// span caps at a half turn (garust folds to the short way, and a full
    /// turn collapses), while the piecewise rotor path is still the exact
    /// geodesic. Key times are derived (`tᵢ = duration · i/n`), never
    /// accumulated. `rate == 0` yields an identity hold.
    pub fn spin(rate: f64, plane: Pga3, duration: f64) -> Result<Track, TrackError> {
        if !rate.is_finite() || !duration.is_finite() || duration <= 0.0 {
            return Err(TrackError::InvalidSpin);
        }
        let norm2 = plane.norm_squared();
        if !norm2.is_finite() || norm2 <= 0.0 {
            return Err(TrackError::InvalidSpin);
        }
        let unit = plane * (1.0 / norm2.sqrt());
        if rate == 0.0 {
            return Ok(Track::hold(Motor3::identity()));
        }
        let spans = ((rate.abs() * duration) / (TAU / 4.0)).ceil() as usize;
        Track::keys((0..=spans).map(|i| {
            let t = duration * (i as f64 / spans as f64);
            (t, Motor3::rotor(rate * t, unit))
        }))
    }

    /// Set every span's ease — the whole-track convenience.
    #[must_use]
    pub fn ease(mut self, ease: Ease) -> Track {
        self.eases.fill(ease);
        self
    }

    /// Set one span's ease. Span `i` covers keys `i` and `i + 1`.
    pub fn ease_span(&mut self, span: usize, ease: Ease) -> Result<(), TrackError> {
        match self.eases.get_mut(span) {
            Some(e) => {
                *e = ease;
                Ok(())
            }
            None => Err(TrackError::NoSuchSpan { index: span }),
        }
    }

    /// Evaluate the pose at time `t`.
    ///
    /// Total and clamping: `t` at or before the first key returns the
    /// first pose, at or after the last key the last pose, and a NaN `t`
    /// clamps low to the first pose. In between, the surrounding span's
    /// ease remaps the local parameter and the pose is the geodesic screw
    /// `slerp` between the span's keys — the *short way*: a span expresses
    /// at most a half turn (author long rotations as subdivided keys;
    /// [`Track::spin`] does this automatically).
    ///
    /// Allocation-free, `O(log n)` in the key count — a dense recorded
    /// rollout costs the same as a sparse authored track.
    pub fn eval(&self, t: f64) -> Motor3 {
        let first = self
            .keys
            .first()
            .expect("Track keys are non-empty by construction");
        let last = self
            .keys
            .last()
            .expect("Track keys are non-empty by construction");
        if t.is_nan() || t <= first.0 {
            return first.1;
        }
        if t >= last.0 {
            return last.1;
        }
        let i = self.keys.partition_point(|(kt, _)| *kt <= t) - 1;
        let (t0, m0) = self.keys[i];
        let (t1, m1) = self.keys[i + 1];
        let s = self.eases[i].apply((t - t0) / (t1 - t0));
        m0.slerp(&m1, s)
    }
}

#[cfg(test)]
mod tests {
    use super::{Track, TrackError};
    use garust::Motor3;

    /// The span search lands on the correct pair right at an interior key.
    #[test]
    fn span_search_is_exact_at_interior_keys() {
        let m = Motor3::identity();
        let track = Track::keys([(0.0, m), (1.0, m), (2.0, m)]).expect("valid keys");
        // Interior key time: clamps run first, then span [1, 2] at u = 0.
        assert_eq!(track.eval(1.0), m);
    }

    /// A single non-finite key is caught even without a pair to compare.
    #[test]
    fn single_non_finite_key_is_rejected() {
        let m = Motor3::identity();
        for t in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(
                Track::keys([(t, m)]).unwrap_err(),
                TrackError::NonIncreasing { index: 0 }
            );
        }
    }
}
