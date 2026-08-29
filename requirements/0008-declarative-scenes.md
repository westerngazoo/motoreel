# R-0008 — Declarative scenes: a video without writing Rust

- **Status:** Draft — for discussion
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

*(Draft — to be settled in discussion, then made testable.)*

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
- **AC5 — it runs without a Rust toolchain.** *(Contingent on Q4.)*
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

These are the discussion. Nothing below is decided.

- **Q1 — What is the format?** TOML reads best for humans and needs a
  parser dependency. JSON matches physics-lab's `lesson.json` precedent
  and also needs one (or a hand-written reader, as the wasm C-ABI was
  hand-written to avoid `wasm-bindgen`). A bespoke line format needs no
  dependency and no ecosystem either.
- **Q2 — What drives it?** A `motoreel render scene.toml` CLI in this
  crate, or a library entry point that a front end calls?
- **Q3 — Is there a Python front end?** The evidence in §2 points at
  Python as the language the audience — and the owner — actually writes.
  A thin Python package that emits the scene file would meet people where
  they are without putting Python in the engine. In scope here, or its
  own requirement?
- **Q4 — How does a creator get the binary?** "Without writing Rust"
  still means installing Rust to `cargo build` unless something is
  released. Tagged release binaries, or is a toolchain an acceptable
  prerequisite for now? This is the difference between a real answer to
  the audience decision and a partial one.
- **Q5 — Does the format cover physics (R-0004 `record`)?** Naming a
  rigid body, an initial twist and a duration is a different vocabulary
  from naming keyframes. In scope, or M2's business?
- **Q6 — Units and angles.** τ is the house circle constant. Does a scene
  file say `turns = 0.25`, `tau = 0.25`, or `degrees = 90`?

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-08-27 | Drafted for discussion; §2's evidence gathered from the owner's own production repo rather than assumed | The audience decision was made on 2026-08-27 without data; there is now data, and it supports the decision more strongly than the reasoning that produced it |

## Changelog

- 2026-08-27 — created, Draft, for discussion.
