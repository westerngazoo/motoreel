# R-0008 — Declarative scenes: a video without writing Rust

- **Status:** Accepted (2026-08-29) — surface decided; format decided
- **Milestone:** MC (the creator pipeline) — its final requirement
- **Owner:** Gustavo Delgadillo (westerngazoo)
- **Created:** 2026-08-27
- **Depends on:** R-0006 (`PpmSink`), R-0007 (labels) — both `Met`
- **Realized by:** SPEC-0008 *(not yet written)*
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

A creator must be able to describe a scene **as data** and get a video,
without writing Rust and without a Rust toolchain. The engine reads a
scene description, renders frames through the existing sinks, and the
documented ffmpeg line turns them into an mp4.

Everything the engine can already do — motor keyframes with easing, a
posed camera, the four stroke primitives, anchored labels, physics
rollouts recorded into tracks — must be reachable from that description.
A capability the engine has but the description cannot name is a gap this
requirement is meant to close, not a detail to defer.

## 2. Rationale

`project-specifics.md` §Audience (owner decision, 2026-08-27) names the
audience as "STEM creators who do not write Rust — teachers and science
communicators who want to describe a scene and get a video," and calls the
declarative path **essential rather than optional**.

There is now direct evidence for that claim, from inside this ecosystem
rather than from speculation about hypothetical users:

- **The project's own owner does not author in Rust for real work.** Nine
  published reels at `@fisicobuenfisico` are Python + PIL. motoreel was
  tried once — `out/reels/motoreel_test.mp4`, 25 Aug — and not used again.
  The tool that gets reached for is the one that can be written in the
  language its author already thinks in.
- **What that Python stack proves about shape.** Its reusable vocabulary
  is a set of small named pieces — an axis object, a trace, a label, a
  readout panel, a legend — composed differently per episode. Six charts
  share primitives and differ in arrangement. A declarative scene format
  should expect the same: a fixed vocabulary, freely arranged, not a
  fixed template with slots.
- **physics-lab already ships the house precedent**: `lesson.json`
  describes a lesson as data and one fixed runtime paints it. The same
  split applied to film is this requirement.

The gap this closes is not expressiveness — the engine is expressive
already. It is **reach**: today, using motoreel means writing a Rust
crate against a path dependency on garust.

## 3. Acceptance criteria

- **AC1 — a scene is data.** A scene file in the chosen format renders to
  frames with no Rust written by the creator, and the same file renders
  byte-identically on a second run.
- **AC2 — the vocabulary is complete.** Every capability R-0001 through
  R-0007 exposes is nameable in the format: keyed motor tracks with
  easing, both projections, a posed camera, `Point`/`Segment`/`Polyline`/
  `Edges`, styles, all three label anchors and the 3×3 screen grid, both
  sinks, and view/size selection.
- **AC3 — motion is authorable by a non-mathematician.** A creator can
  express "rotate a quarter turn about this axis over two seconds" and
  "move from here to there" without writing a motor by hand, while the
  motor form stays available for anyone who wants it.
- **AC4 — errors name the mistake and the place.** A malformed scene
  reports what is wrong and where, and writes no frames. A misspelled
  field is an error, never a silently ignored one.
- **AC5 — struck.** *(Was: "it runs without a Rust toolchain." Q4 records
  why this is not delivered here rather than letting it look delivered.)*
- **AC6 — the demo is a scene file.** At least one shipped example is
  authored in the format rather than in Rust, and its output is asserted
  against a golden.

## 4. Constraints & non-goals

- **Not a programming language.** No user-defined functions, no control
  flow, no arithmetic expressions beyond what a field needs. If a scene
  wants computation, that is the `wedge` expression language's territory
  (garust RFC-014), not this format's.
- **No new dependencies without an owner decision.** The crate's
  dependency graph is `garust` alone, asserted by a test in R-0003, R-0006
  and R-0007. A format needing a parser crate breaks that and must be
  decided explicitly (Q1).
- **Not a replacement for the Rust API**, which stays the substrate.

## 5. Open questions

- **Q1 — What is the format?** **Settled: a line-oriented format with a
  hand-written reader, no dependency.** The owner chose "a data file the
  engine reads" (2026-08-29), which leaves only *which* file. TOML and
  JSON both need a parser crate, and the crate's dependency graph is
  `garust` alone — asserted by a test in R-0003, R-0006 and R-0007, and
  changing it is an owner decision this requirement does not need to
  spend. The house has already answered this shape three times: the wasm
  C-ABI hand-written rather than `wasm-bindgen`, the SVG template pinned
  rather than generated, the R-0006 rasterizer written in `std`. A format
  we define is also a format we can make trivially parseable and
  precisely diagnosable, which AC4 needs. See SPEC-0008 §2 for the
  grammar.
- **Q2 — What drives it?** **Settled: both.** A library entry point does
  the work; a thin `motoreel` binary in the same crate calls it, so the
  CLI is a shell and a front end can skip it.
- **Q3 — Is there a Python front end?** **Deferred to its own
  requirement.** The owner's choice scopes R-0008 to the file. A Python
  package that emits it is additive, needs nothing from the engine, and
  should not gate this.
- **Q4 — How does a creator get the binary?** **Open — the honest gap.**
  "Without writing Rust" still means installing Rust to `cargo build`
  unless something is released. R-0008 does not close this, and AC5 is
  therefore struck rather than quietly claimed. Recorded so the shortfall
  is visible instead of implied.
- **Q5 — Does the format cover physics (R-0004 `record`)?** **Not in
  R-0008.** Naming a body, an inertia and an initial twist is a different
  vocabulary from naming keyframes, and R-0005 has not been specced. The
  grammar must leave room for a `body` verb without a breaking change —
  a constraint on the design, checked in SPEC-0008.
- **Q6 — Units and angles.** **Settled: turns.** τ is the house circle
  constant, so a quarter turn is `turn = 0.25`. `degrees` is accepted as
  an alternate spelling on the same field set, because a teacher writing
  `degrees = 90` should not have to know what τ is to get started. Both
  are exact for the dyadic cases the goldens use.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-08-27 | Drafted for discussion; §2's evidence gathered from the owner's own production repo rather than assumed | The audience decision was made on 2026-08-27 without data; there is now data, and it supports the decision more strongly than the reasoning that produced it |
| 2026-08-29 | **The surface is a data file the engine reads**, not a Python front end (owner) | The file is the engine's contract and is testable on its own; ergonomic front ends can be layered on it later without the engine growing a second authoring path to keep working |
| 2026-08-29 | Format is a line-oriented grammar with a hand-written reader; **no new dependency** | Keeps the garust-only graph three requirements already assert, so no dependency-policy decision is spent here. Matches the house's three existing hand-written-over-dependency calls, and a format we define is one we can make precisely diagnosable — which AC4 requires |
| 2026-08-29 | Angles are **turns**, with `degrees` as an accepted alternate spelling | τ is the house constant, so `turn = 0.25` is the native form; but a teacher should not need to know that to write their first scene, and both are exact on the dyadic cases |
| 2026-08-29 | AC5 struck rather than kept as aspirational (Q4) | A creator still needs a Rust toolchain to build the binary. Keeping the criterion would let the requirement claim a reach it does not deliver |

## Changelog

- 2026-08-27 — created, Draft, for discussion.
- 2026-08-29 — Accepted. Q1, Q2, Q3, Q5, Q6 settled; Q4 left open and AC5
  struck so the shortfall stays visible. History appended, not rewritten.

## Appendix — the proposed surface

Not normative; SPEC-0008 pins the grammar. Included here so the decision in
§5 Q1 can be judged on how it reads rather than on how it is described.
This is the shipped `labelled_lab` demo, which today is a Rust file.

```
# labelled_lab.scene — the three anchor kinds on one screw motion.
# Every line is `verb [name] key=value ...`. No nesting, no indentation
# rules, no quoting except around text. A bad line names itself.

scene   duration=4 fps=60 view=3.2,1.8 size=1920,1080
sink    ppm dir=out/lab
camera  pinhole at=0,0,6 focal=2

# ---- geometry -------------------------------------------------------
object  square shape=polyline stroke=#ffffff width=0.02
points  square  -0.5,-0.5  0.5,-0.5  0.5,0.5  -0.5,0.5  -0.5,-0.5

# One full turn about z while rising 2. Authored as quarter turns because
# a full turn in one key is antipodal — the engine says so if you try.
key     square t=0
key     square t=1 turn=0.25 axis=z rise=0.5
key     square t=2 turn=0.50 axis=z rise=1.0
key     square t=3 turn=0.75 axis=z rise=1.5
key     square t=4 turn=1.00 axis=z rise=2.0

# ---- labels ---------------------------------------------------------
# pinned to the frame
label   title  text="SCREW MOTION" screen=top-centre align=center \
        offset=0,-0.22 size=0.12 stroke=#ffffff

label   note   text="one turn about z while rising 2" screen=bottom-left \
        offset=0.06,0.10 size=0.07 stroke=#00b4d8

# pinned to the body's own model space — rides the motion, no bookkeeping
label   rider  text="body" on=square at=0,0.75 align=center \
        size=0.09 stroke=#ff4d00
```

Shape decisions visible in it, each of which SPEC-0008 must justify:

- **Every line is `verb [name] key=value ...`.** Flat: no nesting, no
  indentation significance, no block delimiters. A parse error can always
  name a line and a key, which is what AC4 asks for.
- **Geometry is a separate `points` line** rather than a field, because a
  vertex list is the one thing that does not fit on one line.
- **Angles are turns** (§5 Q6). `turn=0.25` is a quarter turn; `degrees=90`
  is the same thing for someone who has not met τ yet.
- **`turn`/`axis`/`rise` together are a screw**, which is the motion
  motoreel exists to make exact. `rise` is along `axis`, so the rotation
  and the translation commute and the key is exact.
- **The full-turn trap is surfaced, not hidden.** `turn=1` in a single span
  is antipodal; SPEC-0003 §2.7 already records this and the demo already
  works around it. The reader must reject it with that explanation rather
  than silently rendering the short way round.
- **Line continuation with a trailing backslash**, so a long label does not
  force a nested form on the other 95% of lines.
