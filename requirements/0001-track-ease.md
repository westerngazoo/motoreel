# R-0001 — Motor keyframe tracks (`Track` + `Ease` + evaluation)

- **Status:** Met
- **Milestone:** M1
- **Owner:** Gustavo Delgadillo (westerngazoo)
- **Created:** 2026-08-20
- **Depends on:** none
- **Realized by:** SPEC-0001
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

motoreel must represent motion as **motor keyframe tracks**: an ordered
sequence of `(time, Motor3)` keys. Evaluating a track at any time `t` yields a
pose motor by locating the surrounding key pair, remapping the local parameter
through that span's ease function, and taking the geodesic screw between the
two key motors (`slerp`). The motor is the only motion representation.

A track does not care where its keys came from: sparse hand-authored keyframes
and dense fixed-timestep samples recorded from a physics rollout are the same
data, evaluated by the same path. This is the project's differentiator — the
`(time, Motor)` currency shared by authored animation and simulated mechanics.

## 2. Rationale

Every later capability (scenes, sinks, physics playback, GA-quantity glyphs)
consumes poses through track evaluation. Getting the semantics right —
geodesic path, ease-as-schedule-only, total evaluation, typed construction
errors — is the foundation the rest of M1–M3 stands on. (RFC-012 §3.2, A1.)

## 3. Acceptance criteria

- **AC1.** Evaluating at a key's exact time returns that key's motor exactly.
- **AC2.** The midpoint of a pure-translation key pair is the half
  translation; the midpoint of a pure-rotation pair is the half rotation
  (constant-speed screw geodesic).
- **AC3.** Easing changes the schedule, never the path: for every ease `e`
  and time `t` inside a span with local parameter `u`, the eased evaluation
  equals the linear evaluation at `e(u)`. (Property-tested.)
- **AC4.** Evaluation is total and clamps: `t` at or before the first key
  returns the first motor; at or after the last key, the last motor.
- **AC5.** Construction validates: keys must be non-empty with strictly
  increasing times; violations return a typed error (no panic, no silent
  reordering).
- **AC6.** Easing is **per span**: each key-to-key segment carries its own
  `Ease`; a whole-track convenience applies one ease to every span.
- **AC7.** `Ease` provides `Linear`, `SmoothStep`, `SmootherStep`, and
  `Custom(fn(f64) -> f64)`. Tracks are `Clone`, and evaluation is
  deterministic: identical inputs produce bit-identical motors.
- **AC8.** Dense sampled tracks work identically to sparse ones: a track
  built from ≥240 uniformly spaced keys (a recorded rollout) evaluates
  exactly at key times and interpolates between them, with no API distinction
  from an authored track.

## 4. Constraints & non-goals

- Pairwise geodesic interpolation only — no motor splines (garust RFC-013
  offers them upstream when a requirement wants C¹ paths), no track blending,
  no time-warping beyond per-span easing.
- `std` + garust only; zero further dependencies.
- Angles are radians against TAU (garust convention).

## 5. Open questions

None — settled 2026-08-20 (see decision log).

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-08-20 | `Ease::Custom` is a `fn(f64) -> f64` pointer, not a boxed closure | Zero-dep; keeps `Track: Clone` and bit-for-bit determinism (RFC-012 §6 Q1, owner) |
| 2026-08-20 | Out-of-range evaluation clamps/holds at end poses | Standard animation hold; total function, no error path in the render loop (owner) |
| 2026-08-20 | Invalid keys are a typed construction error | Constitution §6: no unchecked failures, typed errors (owner) |
| 2026-08-20 | Easing is per span, with a whole-track convenience | Resolves RFC-012's internal contradiction toward its stated intent (owner) |
| 2026-08-20 | Dense physics-rollout samples are first-class track keys (AC8) | The `(time, Motor)` differentiator: simulated and authored motion share one currency (owner directive) |

## Changelog

- 2026-08-20 — created; discussed and accepted same day.
- 2026-08-20 — implemented (SPEC-0001) and QA-signed-off: PASS, every AC
  mapped to passing tests, all gates green. Merge formalities pending repo
  setup (owner). QA non-gating note: `spin` with an absurd finite rate
  (~1e308) saturates the span count — future hardening decision.
