# SPEC-0001 — `Track` + `Ease`: motor keyframe evaluation

- **Status:** Draft — architect review 2026-08-20 (REQUEST CHANGES) applied; awaiting owner acceptance
- **Realizes:** R-0001
- **Author:** Claude (main session) with owner
- **Created:** 2026-08-20
- **Depends on:** none
- **Module(s):** `crates/motoreel/src/ease.rs`, `crates/motoreel/src/track.rs`

## 1. Motivation

R-0001: motion as motor keyframe tracks — ordered `(time, Motor3)` keys,
eased geodesic-screw interpolation between them, total clamping evaluation,
typed construction errors, and no API distinction between sparse authored
keys and dense recorded rollouts (the `(time, Motor)` currency).

## 2. Design

Two small modules, no state beyond the data itself, everything `Clone` and
deterministic.

**`ease`** — a pure time-remap. `Ease` is a fieldless-plus-`fn`-pointer enum;
`apply(u)` first clamps `u` to `[0, 1]` (defensive: span-local parameters are
produced clamped already, but `Custom` outputs are also clamped so an
ill-behaved custom curve cannot push `slerp` outside the span — the path
invariant AC3 depends on it). **Normative `Custom` contract (F1):** a custom
curve must be *anchored* — `f(0) = 0` and `f(1) = 1`. The built-ins satisfy
this; an unanchored custom would return a non-key motor at interior key times
(violating AC1's "exactly") and break C⁰ continuity at span joins. This is a
documented contract in the same GIGO style SPEC-0002 uses for `focal`;
outputs are clamped regardless, with NaN clamping low to `0.0` (`f64::clamp`
propagates NaN; the low clamp matches `eval`'s NaN convention).

**`track`** — `Track` (`Clone + Debug`; deliberately no `PartialEq` — `Ease::Custom` fn-pointer address equality is codegen-dependent) owns `keys: Vec<(f64, Motor3)>` and
`eases: Vec<Ease>` with the invariant `eases.len() == keys.len() - 1`
(empty for a single-key track), established at construction and never
broken. Construction is the only fallible surface:

- `Track::keys(iter) -> Result<Track, TrackError>` — non-empty, strictly
  increasing times; every span `Ease::Linear`.
- `Track::hold(pose: Motor3) -> Track` — infallible single-key track (key at
  `t = 0.0`; clamping makes it a constant pose). The static-object case
  SPEC-0002 needs without an `expect` at its call sites.
- `TrackError` — `Empty` | `NonIncreasing { index }` | `NoSuchSpan { index }`
  | `InvalidSpin`. Key times must form a **finite, strictly increasing**
  sequence; `NonIncreasing { index }` reports the first key that breaks it —
  its own index for a non-finite time (so a single NaN/∞ key is caught), the
  second key of a non-increasing pair;
  implements `Display` + `std::error::Error`.
- `ease(self, e) -> Track` — whole-track convenience, sets every span (AC6).
- `ease_span(&mut self, span, e) -> Result<(), TrackError>` — per-span (AC6).
- `spin(rate, plane, duration) -> Result<Track, TrackError>` — convenience
  constructor for uniform rotation at `rate` rad/s about the given (unitized)
  plane bivector: **emits subdivided keys, at most a quarter turn per span**,
  poses `rotor(rate·tᵢ, plane)`. Rationale: `slerp` through a near-antipodal
  motor (rotation approaching a half turn in one span) is directionally
  ambiguous; subdivision keeps every span well inside the unambiguous range
  while the piecewise path remains the exact geodesic (a rotor path *is* its
  own geodesic — subdivision changes nothing but ambiguity). Key times are
  derived, not accumulated: `tᵢ = duration · i/n`. Validation (F5): `rate`
  and `duration` must be finite and `duration > 0`, else
  `Err(TrackError::InvalidSpin)`; the plane bivector is **normalized by
  `spin`** (callers pass any non-zero multiple). NaN key times can never
  reach `Track::keys` from here; if constructed directly they fail its
  finite-and-strictly-increasing check, which is documented on `keys`.
  `rate == 0` yields `Track::hold(identity)` — a total, unsurprising spin of
  nothing; a plane whose Euclidean part has zero norm (e.g. an ideal plane)
  is `InvalidSpin` — there is no rotation rate about it.

Evaluation (`eval(&self, t: f64) -> Motor3`) is total and allocation-free:

1. `t ≤ first.time` → first pose; `t ≥ last.time` → last pose; a NaN `t`
   clamps low to the first pose via an explicit branch (AC4 stays total —
   the comparison chain alone would underflow the span search) (AC4).
2. Binary search (`partition_point`) for the surrounding span — O(log n) so
   240+-key rollout tracks cost the same as authored ones (AC8).
3. `u = (t − t0) / (t1 − t0)`; `s = ease[span].apply(u)`;
   `m0.slerp(&m1, s)` (AC1–AC3).

**Key hygiene (audit-driven):** `Track` construction renormalizes every key
once (`Motor::renormalize`). Rationale: garust's `Multivector::log` — inside
`slerp` — `debug_assert!`s the unit-versor contract, so a slightly drifted
key (e.g. from a long physics rollout) would panic a debug-build render loop.
Normalizing at the single construction site keeps `eval` total with zero
per-frame cost.

**Span semantics (audit-driven):** garust's `slerp` interpolates the *short
way* and caps a single span at a half turn — at exactly τ the interpolation
collapses to identity, and near-τ rotations play backwards (verified against
`garust-core/src/transform.rs:319`, no upstream test coverage today). A
`Track` span therefore *means* the short-way screw between its keys; long
rotations are authored as subdivided keys, which `spin` does automatically.
This is documented on `eval`. Upstream work is in flight to make garust
turn-aware (`Motor::exp` + generator-cached spans); when it lands, eval can
cache each span's screw generator instead of recomputing `log`/`exp` per
call — an optimization, not a semantic change.

## 3. Code outline

```rust
// ease.rs — the schedule, never the path
#[derive(Clone, Copy, Debug)]
pub enum Ease {
    Linear,
    SmoothStep,
    SmootherStep,
    Custom(fn(f64) -> f64),
}

impl Ease {
    /// Remap a span-local parameter; input and output clamped to [0, 1].
    pub fn apply(self, u: f64) -> f64 {
        let u = u.clamp(0.0, 1.0);
        let s = match self {
            Ease::Linear => u,
            Ease::SmoothStep => u * u * (3.0 - 2.0 * u),
            Ease::SmootherStep => u * u * u * (u * (u * 6.0 - 15.0) + 10.0),
            Ease::Custom(f) => f(u),
        };
        s.clamp(0.0, 1.0)
    }
}

// track.rs — evaluation is the whole engine
pub fn eval(&self, t: f64) -> Motor3 {
    let (first, last) = (
        self.keys.first().expect("Track keys are non-empty by construction"),
        self.keys.last().expect("Track keys are non-empty by construction"),
    );
    if t <= first.0 { return first.1; } // Motor3 is Copy
    if t >= last.0  { return last.1; }
    let i = self.keys.partition_point(|(kt, _)| *kt <= t) - 1; // span index
    let ((t0, m0), (t1, m1)) = (&self.keys[i], &self.keys[i + 1]);
    let s = self.eases[i].apply((t - t0) / (t1 - t0));
    m0.slerp(m1, s)
}
```

(The `expect` message carries the §6-required justification in the code
itself: the non-empty invariant is established by the only constructors.
Mixed builder style is deliberate: `ease(self)` consumes for construction
chains; `ease_span(&mut self)` borrows for loop-driven per-span setup.)

## 4. Non-goals

- Motor splines / C¹ multi-key smoothing (garust RFC-013 upstream, later
  requirement), track blending, time-warping beyond per-span ease.
- No `Scene`/rendering knowledge — `track` depends only on garust.

## 5. Open questions

None — architect review 2026-08-20 adjudicated all findings (F1–F5);
changes applied above.

## 6. Acceptance criteria

Each maps to R-0001's AC and becomes a qa-owned test:

- [ ] AC1 — eval at every key time returns that key's motor exactly (unit +
  property over random valid tracks; the generator draws **anchored** eases
  only, per the `Custom` contract).
- [ ] AC2 — midpoint of pure-translation pair = half translation (exact for
  dyadic keys); of pure-rotation pair (≤ quarter turn) = half rotation (at
  tolerance ~1e-12 — `sin`/`cos` involved, matching garust's own tests).
- [ ] AC3 — property: for random ease/track/t, eased eval == linear eval at
  the remapped parameter.
- [ ] AC4 — property: out-of-range t clamps to end poses.
- [ ] AC5 — `Empty` and `NonIncreasing` construction errors (incl. equal
  times); `NoSuchSpan` from `ease_span`.
- [ ] AC6 — per-span ease honored; whole-track `ease()` sets all spans.
- [ ] AC7 — `Track: Clone`; two evals of a cloned track are bit-identical
  (compare motor coefficient bits).
- [ ] AC8 — 240-key uniform track: exact at keys, interpolating between,
  API-identical to sparse (shared test path with AC1).
- [ ] `spin`: pose at `t` equals `rotor(rate·t, plane)` for spot times
  including a full TAU turn (property within tolerance); span count is
  `ceil(|rate|·duration / (TAU/4))`.

## 7. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-08-20 | `eases` as parallel `Vec` with structural invariant, not per-key storage | A span property stored span-wise; invariant checked at the single construction site |
| 2026-08-20 | `spin` emits subdivided keys (≤ quarter turn/span) instead of new procedural-track machinery | Stays inside R-0001's pairwise-geodesic scope; avoids antipodal `slerp` ambiguity; piecewise rotor path is still the exact geodesic |
| 2026-08-20 | `Ease::apply` clamps input and output | Protects the AC3 path invariant from ill-behaved `Custom` curves |
| 2026-08-20 | Binary-search eval, allocation-free | AC8 makes dense rollout tracks first-class; O(log n) keeps 60 fps × many objects trivial |
| 2026-08-20 | Keys renormalized at construction | garust `log` debug-asserts the unit contract; drifted rollout keys must not panic a render loop |
| 2026-08-20 | A span means the short-way screw (≤ half turn) | garust slerp folds direction at τ/2 and collapses at τ (audit); subdivision — not new machinery — expresses long rotations |
| 2026-08-20 | `Track::hold` infallible constructor; `Track` derives `Debug` | SPEC-0002 reconciliation: static objects are common and must not carry a fallible path or an `expect` |
| 2026-08-20 | Architect F1–F5 applied: anchored-`Custom` contract; `expect` with justifying message; `Copy` not `clone()`; no `PartialEq` on `Track`/`Ease`; `spin` validation via `TrackError::InvalidSpin`, plane normalized | Architect review (REQUEST CHANGES → resolved); owner acceptance pending |
| 2026-08-20 | qa-gap decisions: `NonIncreasing.index` = first chain-breaking key (own index when its time is non-finite); key times validated finite at construction; `eval(NaN)` clamps to the first pose explicitly; `spin(0, …)` = identity hold; zero-Euclidean-norm plane = `InvalidSpin` | qa run surfaced three underspecified corners (loop step 3 doing its job); decisions keep evaluation total and the error enum minimal |
| 2026-08-20 | Known non-gating edge, deferred: `spin` span count saturates for finite rates near 1e308 (allocation abort). Hardening (cap → `InvalidSpin`) is a future loop decision | QA sign-off observation; unreachable in practice, recorded so it is not lost |

## Changelog

- 2026-08-20 — created.
