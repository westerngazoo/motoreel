# R-0004 — Physics playback: record a simulated rollout into motor tracks

- **Status:** Met
- **Milestone:** M2
- **Owner:** Gustavo Delgadillo (westerngazoo)
- **Created:** 2026-08-21
- **Depends on:** R-0001, R-0002, R-0003
- **Realized by:** SPEC-0004
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

motoreel must be able to take a `garust-physics` world — bodies, joints,
gravity — step it at a **fixed timestep**, and record each body's pose into
a motor [`Track`]: `(t, body.rigid.pose())` per step. The result is
ordinary track data. Nothing downstream learns that a simulation produced
it; a recorded track goes into an `Object` exactly like an authored one and
renders through the same path.

This is the project's differentiator made real. Manim animates what you
describe; motoreel can also animate what the mechanics *do*.

To render a solid body, the shape vocabulary gains **`Shape::Edges`** — a
wireframe edge list drawn as one primitive with one style. (A cube cannot
be traced as a single polyline: every vertex has odd degree, so no Euler
path exists and edges would visibly retrace.) This extends R-0002's
`Shape`/`Prim2` vocabulary and R-0003's SVG grammar; both are amended, not
superseded, and their existing golden bytes are unaffected.

The demo is a **freely tumbling box**: three distinct principal moments,
zero gravity, spun about its intermediate axis. Such a body flips
end-over-end periodically — the intermediate axis theorem, the
Dzhanibekov effect — which is exactly the kind of mechanics that is hard
to believe from equations and obvious in motion.

## 2. Rationale

M2's purpose. It also proves the `(time, Motor)` currency claim end to end:
R-0001 AC8 established that a dense recorded track is API-identical to an
authored one, but nothing has yet produced one from real dynamics.

## 3. Acceptance criteria

- **AC1.** Recording `n` steps of a world at fixed `dt` produces, per body,
  a `Track` of `n + 1` keys at times `t_i = i · dt` (derived, not
  accumulated), whose pose at key `i` is that body's pose after `i` steps.
  Key 0 is the initial pose, before any step.
- **AC2.** A recorded track is ordinary track data: it is accepted by
  `Object::with_track` and renders through `Scene::eval`/`render` with no
  simulation-aware code path anywhere in the renderer.
- **AC3.** The recorder does not mutate the caller's inputs — recording
  twice from the same initial state reproduces the same rollout.
- **AC4.** Determinism: recording the same world twice in one process
  yields **bit-identical** tracks, and rendering them yields bit-identical
  frames. (Same-machine — see §4.)
- **AC5.** `Shape::Edges` renders as a single styled primitive carrying all
  its segments; the whole-primitive cull rule (R-0002 §4) and the
  never-non-finite coordinate invariant hold for it exactly as for the
  other shapes.
- **AC6.** The SVG grammar gains one pinned template for the edge
  primitive, byte-deterministic like the rest; frames containing no edge
  primitive are byte-unchanged from R-0003 (its golden fixture still
  passes untouched).
- **AC7.** First simulated light: the tumbling-box demo renders 4 s at
  60 fps to 240 frames, shows at least two visible flips, and is
  bit-identical across two renders in one process.

## 4. Constraints & non-goals

- **Determinism is per-machine, not cross-platform.** A rollout is
  trig-heavy and f64 transcendentals may differ in the last ulp across
  platforms/libm versions. R-0003's checked-in golden stays trig-free and
  portable; physics determinism is asserted by same-process re-recording.
  `project-specifics.md` is corrected accordingly.
- No joints/pendulum (R-0005), no collision demos, no live stepping during
  render — simulation runs to completion first, then frames sample the
  recording.
- No GA-quantity glyphs (momentum bivectors, screw axes): M3. The garust
  accessors exist but stay unused here.
- No adaptive timestep, no sub-stepping policy beyond the caller's `dt`.

## 5. Open questions

None — the four shaping decisions were settled with the owner on
2026-08-21 (see decision log).

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-08-21 | Add `Shape::Edges` (and its `Prim2` counterpart) rather than cloning a track across N segment objects or retracing a polyline | A cube has no Euler path, so a single polyline visibly retraces edges; N objects would evaluate the same rollout track N× per frame and scatter one solid across N scene entries (owner) |
| 2026-08-21 | Record **every physics step**, not at render rate | The rollout is ground truth: a dense track re-renders at any fps including slow motion without re-simulating, and R-0001 AC8 already made dense tracks first-class. 4 s at dt = 1/240 is 960 keys ≈ 123 KB/body (owner) |
| 2026-08-21 | Scope R-0004 to the recorder + tumbling box; the pendulum and joints become R-0005 | Smaller reviewable diffs per §7; the pendulum pulls in joint setup and constraint-solver behaviour orthogonal to recording (owner) |
| 2026-08-21 | Determinism tested as same-machine byte-exactness, mirroring R-0003 AC4 | Honest about the platform-libm caveat the garust audit surfaced, while keeping the strongest testable claim; the portable golden stays trig-free (owner) |

## Changelog

- 2026-08-21 — created; discussed and shaped with the owner the same day.
- 2026-08-21 — implemented (SPEC-0004) and QA-signed-off: PASS, 90/90
  workspace tests, all gates raw exit 0, QA-owned suite byte-unmoved
  through red→green, R-0003's golden fixture byte-unchanged. QA confirmed
  the flips independently from the rendered SVG bytes (sign reversals at
  frames 41/123/205 — exactly the measured crossings), touching neither
  the track nor simulation state.
