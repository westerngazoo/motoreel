# R-0005 — Joints and the double pendulum: simulating sensitive dependence

- **Status:** Draft
- **Milestone:** M2
- **Owner:** Gustavo Delgadillo (westerngazoo)
- **Created:** 2026-08-23
- **Depends on:** R-0004
- **Realized by:** SPEC-0005
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

motoreel must record and render **constrained** rigid-body motion: bodies
linked by `garust-physics` joints, anchored to a fixed world point. The
recorder already forwards the `&[Joint]` slice, so this requirement adds no
recording machinery — it proves that constrained rollouts record and render
exactly like free ones, and it delivers M2's second explanatory video.

The demo is a **double pendulum, twinned**: two identical double pendulums
released from initial conditions differing by 1e-12, rendered in one scene.
They track together, then visibly separate. That is sensitive dependence on
initial conditions — chaos — shown rather than asserted, and it says
something sharp about determinism at the same time: identical inputs
produce bit-identical output, while inputs differing in the last few bits
produce entirely different futures. Both facts are true, and neither is
obvious until you watch it.

## 2. Rationale

M2's second half. A free-tumbling body (R-0004) exercises the integrator;
a jointed chain exercises the **constraint solver**, which is a different
machine with different failure modes — drift, sag, energy injection. This
requirement asserts against those directly.

## 3. Acceptance criteria

- **AC1.** A world containing joints records exactly as a free one does:
  `record` is called with a non-empty `&[Joint]` and returns one `Track`
  per body, with the key times and count R-0004 AC1 already fixes. No
  change to `record`'s signature or behaviour is required.
- **AC2.** Determinism, as R-0004 AC4: two recordings of the jointed world
  in one process are bit-identical, and frames rendered from them are
  byte-identical.
- **AC3.** **Sensitive dependence is demonstrated and asserted.** Two
  rollouts differing only by a perturbation of **ε = 1e-3 rad** in one
  initial angle produce tip positions that agree closely early and
  differ by more than a measured threshold by the end of the demo. Both
  the early agreement and the late divergence are asserted, at measured
  times. **Corrected from a first draft that specified 1e-12** — the
  probe measured λ ≈ 1.1/s, so separation from 1e-12 needs
  `ln(1/ε)/λ ≈ 25 s` to become visible and the demo is 6 s. At 1e-3 the
  arc is right: sub-millimetre through 2.5 s, 1.4 cm at 3 s, 7 cm at
  4 s, ~1 m at 5 s.
- **AC4.** **Energy stays in a measured band.** Total energy (translational
  + rotational kinetic + gravitational potential) stays within a bounded
  relative band of its initial value across the rollout, with no secular
  trend. The band is measured by probe before being written down, and is
  binding thereafter.
- **AC5.** **The constraints hold.** The world-space separation of each
  joint's two anchor points stays below a measured bound for the whole
  rollout — the demo does not visibly come apart.
- **AC6.** Planar release stays planar: with all initial positions and
  velocities in a plane, out-of-plane excursion stays below a measured
  bound. (See §4 — this is what makes the accepted hinge limitation
  invisible in the demo.)
- **AC7.** The demo renders at 60 fps to numbered frames, twice,
  bit-identically, with both pendulums and the pivot visible in frame for
  the whole rollout.

## 4. Constraints & non-goals

- **Accepted upstream limitation.** garust's v1 solver enforces only the
  *positional* part of `Hinge`: the axis is stored but relative rotation is
  not locked, so a hinge currently behaves as a ball joint. The demo works
  within this by releasing in-plane, where symmetry keeps the motion planar
  and the distinction invisible. This is documented, not hidden, and AC6
  measures rather than assumes it. Locking hinge rotation upstream is
  future garust work, out of scope here.
- No collisions, no ground plane, no contact between the two pendulums —
  they are independent chains sharing a scene.
- No new recording machinery, no new shape vocabulary (R-0004's
  `Shape::Edges` draws the links), no GA glyphs (M3).
- Energy and constraint bands are *empirical* properties of this solver at
  the chosen `dt`, not physical guarantees; they are regression bars, and
  changing `dt` or the parameters is a spec change.

## 5. Open questions

None. The three shaping decisions were settled with the owner on
2026-08-23, and the probe has since supplied every measured number
SPEC-0005 needs (see the decision log and §4): flip-free planarity to
bit-exact zero, the energy band at dt = 1/960, joint residuals, the
Lyapunov rate, and the bob-inertia prerequisite. SPEC-0005 is the next
step in the loop.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-08-23 | Accept the hinge-rotation gap; release in-plane and document it | A planar release stays planar by symmetry even on a ball constraint, so the demo reads correctly with zero upstream work; AC6 measures the claim rather than trusting it (owner) |
| 2026-08-23 | Demo is a **twinned double pendulum**, not a simple one | Chaos is the phenomenon worth a study-companion video, and twinning makes it *testable* — early agreement and late divergence are both assertable, and it dramatises determinism from the other side (owner) |
| 2026-08-24 | AC3's perturbation corrected 1e-12 → 1e-3 after the probe measured the Lyapunov rate | The original figure was written without doing the arithmetic: separation grows as ε·e^(λt), so 1e-12 needs ~25 s to surface and the demo is 6 s. The physics was never in question — only the constant (probe) |
| 2026-08-23 | Add binding energy-band and constraint-error criteria (AC4, AC5) | Determinism alone cannot notice a wrong-but-repeatable integrator; a constraint solver's characteristic failures are drift and energy injection, so assert against them directly. Bands measured by probe, never guessed (owner) |

## Changelog

- 2026-08-23 — created; discussed and shaped with the owner the same day.
