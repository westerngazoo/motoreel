# Roadmap

The single source of truth for what is being built and in what order — for the
project named in `project-specifics.md`. Milestones group requirements; each
requirement is realized by one or more specs. Nothing moves without passing the
requirement loop in [`CLAUDE.md`](CLAUDE.md) §4.

## Status legend

`Backlog` → `Discussing` → `Spec'd` → `In progress` → `In review` → `Done`

## Milestones

### M0 — Foundation  ·  *complete*

Adopt the methodology and prepare the repository.

| Item | Status |
|------|--------|
| Methodology files in place (`CLAUDE.md`, `requirements/`, `specs/`, agents) | Done |
| `project-specifics.md` filled in | Done |
| Founding design doc adopted (`docs/RFC-012-garust-anim.md`) | Done |
| Toolchain chosen and recorded (Rust workspace, garust path dep, gates green) | Done |
| First requirement discussed (R-0001 — and R-0002/R-0003 with it) | Done |

### M1 — Motion core & first light

Everything needed to render the first explanatory animation: motor keyframe
tracks, a scene with a motor-posed camera, and SVG frames ffmpeg can encode.
Maps to RFC-012 milestones A1–A3.

| Req | Capability | Spec | Status |
|-----|------------|------|--------|
| R-0001 | `Track` + `Ease` + evaluation: motor keyframes, eased `slerp` in-between; key times return key motors exactly | SPEC-0001 | Met — QA PASS; merge pending repo setup |
| R-0002 | `Scene`/`Object`/`Camera` + projection: motor-posed camera, pinhole & orthographic, `Prim2` output | SPEC-0002 | Met — QA PASS; merge pending repo setup |
| R-0003 | `SvgSink` + 240-frame screw-motion demo encodable by ffmpeg | SPEC-0003 | Met — QA PASS; merge pending repo setup |

### M2 — Physics playback

The differentiator from Manim: motion produced by a *real* GA physics engine.
A garust-physics `World` rollout is recorded at fixed dt into the same
`(time, Motor)` tracks keyframes use — deterministic, bit-for-bit.

| Req | Capability | Spec | Status |
|-----|------------|------|--------|
| R-0004 | `SimTrack`: record a garust-physics `World` rollout into motor tracks (fixed dt; determinism golden test) | SPEC-0004 | Backlog |
| R-0005 | First simulated videos: tumbling free body (Dzhanibekov-ready) and a pendulum, rendered through M1 | SPEC-0005 | Backlog |

### MC — The creator pipeline  ·  *promoted ahead of M3, 2026-08-27*

The project acquired an audience: STEM creators who want to show concepts
and do not write Rust. Two things block them today, and both outrank the
M3 glyph work.

| Req | Capability | Spec | Status |
|-----|------------|------|--------|
| R-0006 | `PpmSink`: P6 raster frames with a stroke rasterizer — video with **stock ffmpeg**, no librsvg, no external rasterizer | SPEC-0006 | **Met** — QA-signed-off 2026-08-27; encode proven against stock ffmpeg |
| R-0007 | Anchored text labels: strings pinned to a world point, a body's pose, or a screen corner; rendered by both sinks | SPEC-0007 | **Met** — QA-signed-off 2026-08-27; both sinks, 95-glyph embedded face |
| R-0008 | *(next)* Declarative scenes: describe a scene as data and render it without writing Rust — the physics-lab manifest pattern, applied to film | — | Backlog |

**Why the renumber.** The M3/M4 requirements below shift up by two; the
GA glyphs and incidence shapes keep their content and lose their old ids.
Recorded rather than silently renumbered: *old* R-0006 (glyphs) → R-0009,
*old* R-0007 (JoinLine/MeetPoint) → R-0010, *old* R-0008 (PpmSink) is
superseded by the new R-0006, *old* R-0009 (SIMD) → R-0011.

### M3 — GA mechanics visuals

The study-companion layer: draw the algebra itself, live from simulation
state — the same way derived shapes are computed live from geometry.

| Req | Capability | Spec | Status |
|-----|------------|------|--------|
| R-0009 | GA quantity glyphs: velocity/momentum bivectors as oriented plane elements, instantaneous screw axis of a motor | SPEC-0009 | Backlog |
| R-0010 | Derived incidence shapes: `JoinLine`/`MeetPoint` resolved from transformed geometry each frame (RFC-012 A5) | SPEC-0010 | Backlog |

### M4 — Throughput

*(The raster sink moved to MC/R-0006: it became a creator blocker, not a
performance nicety.)*

| Req | Capability | Spec | Status |
|-----|------------|------|--------|
| R-0011 | SIMD fast path in the render loop: ≥3× over scalar on a 10k-vertex scene (RFC-012 A6) | SPEC-0011 | Backlog |

## Sequencing rules

- A requirement enters `Discussing` only when every requirement it depends on is
  `Done`.
- Requirement and spec ids are 4-digit and shared in spirit: `R-0001` is
  realized by `SPEC-0001` unless a requirement needs several specs.
- This file is updated by the orchestrator whenever a requirement changes state.
- Capabilities missing from garust (physics readouts, spline forms, …) are
  RFC'd/PR'd **upstream in garust first**, then consumed here.

## Current focus

**M1 is complete (2026-08-21).** R-0001, R-0002, R-0003 all `Met`: every
requirement discussed → spec'd → architect-reviewed → red tests → green
implementation → independent QA sign-off. 67/67 workspace tests, all gates
raw exit 0, and `cargo run --example first_light` renders 240 real frames
that are bit-identical to the test suite's own render (two processes, two
binaries).

**Owner decisions outstanding — the only things blocking loop steps 6–8:**

1. **Git/repo formalities.** The tree has never been committed. Constitution
   §7 wants `R-NNNN-*` branches and one PR per requirement against a
   protected default branch, which needs the GitHub repo
   (`westerngazoo/motoreel`) to exist. Until then M1's three requirements
   are `Met` but unmerged.
2. **garust upstream branches**, complete and green locally, not pushed:
   `claude/slerp-turns` (slerp semantics docs/tests, `Motor::exp`,
   `screw_generator`/`slerp_from_generator`, `slerp_unwrapped`, RFC-012
   §3.5 demo fix) and `claude/physics-bivector-api` (`RigidBody::momentum`/
   `twist` bivectors, `twist_parts`, `ScrewAxis`, `Line`/`Plane` accessors,
   `chain` re-export, `Body::pose`). Note the worktrees lived in session
   tmp — `git worktree prune` in garust may be needed.
3. **Spec acceptance.** All three specs carry architect- and qa-driven
   decision rows marked "owner acceptance pending".

**M2 is under way.** R-0004 is `Met` (2026-08-21): `record` steps a
garust-physics world at fixed dt and returns ordinary motor tracks, the
shape vocabulary gained `Shape::Edges` for wireframe solids (amending
R-0002/R-0003 additively — R-0003's golden fixture is byte-unchanged), and
`cargo run --example tumbling_box` renders a freely tumbling box flipping
end-over-end via the intermediate axis theorem. 90/90 tests green. Nothing
in the renderer knows a simulation produced the motion, which was the
whole claim.

**Next: R-0005 — the pendulum**, adding joints. `World::step` already
takes the `&[Joint]` slice `record` forwards, so the recorder needs no
signature change.
