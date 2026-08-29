# Project Specifics

This is the **single per-project file**. Every other document in this
methodology is generic and identical across all projects — only this file
changes. Fill it in when the project starts; keep it current as these facts
change.

`CLAUDE.md` imports this file, so its contents are always in context.

## Identity

- **Project name:** motoreel
- **One-line description:** A Manim-style mathematical animation engine where
  every motion is a PGA motor — and, unlike Manim, motion can come from a
  *real physics engine*: scenes are keyframed **or simulated** (garust-physics
  rigid-body dynamics), rendered as deterministic numbered frames (SVG/PPM)
  for ffmpeg to encode. A visual companion for studying mechanics through the
  lens of GA. Built on [garust](https://github.com/westerngazoo/garust).
- **Owner / final decision authority:** Gustavo Delgadillo (westerngazoo)
- **Audience (owner decision, 2026-08-27):** STEM creators who do not write
  Rust — teachers and science communicators who want to describe a scene and
  get a video. This reframes the project from a personal tool into one with
  users, and makes the eventual declarative (non-Rust) authoring path
  essential rather than optional; see ROADMAP milestone MC.
- **Repository URL:** https://github.com/westerngazoo/motoreel *(to be created)*

## Language & toolchain

The concrete commands referenced by `CLAUDE.md` §6 and by the `architect` and
`qa` agents as merge gates.

- **Primary language / version:** Rust (stable, currently 1.95)
- **Build command:** `cargo build --workspace`
- **Test command:** `cargo test --workspace`
- **Lint command:** `cargo clippy --workspace --all-targets -- -D warnings`
- **Format-check command:** `cargo fmt -p motoreel --check` *(scoped per package —
  `--all` follows the garust path dependency and formats it under the wrong
  config; add new workspace members to this command as they appear)*
- **Run the gate under CI's toolchain, not just the default one.** CI uses
  `dtolnay/rust-toolchain@stable`. A machine sitting a few releases behind
  cannot see the lints stable has since added, so a locally green gate can
  still fail the merge — which happened on the gate's own first run
  (`chunks_exact_to_as_chunks` and `manual_slice_fill`, neither visible on
  1.95). rustfmt drifts the same way, in both directions. Prefix each
  command to check the way CI will:

  ```bash
  rustup toolchain install stable --component clippy --profile minimal
  rustup run stable cargo test --workspace
  rustup run stable cargo clippy --workspace --all-targets -- -D warnings
  rustup run stable cargo fmt -p motoreel --check
  ```

  Deliberately not pinned via `rust-toolchain.toml`: a pin makes local and
  CI agree but lets lint debt accumulate silently until someone bumps it,
  which is how a sibling repo arrived at 370 hunks of formatting drift
  against its own pinned toolchain.

## Domain notes

The founding design document is
[`docs/RFC-012-garust-anim.md`](docs/RFC-012-garust-anim.md), drafted inside
garust. **Placement decision (supersedes RFC §4):** the owner chose a
standalone project kicked off from the wizzielyn methodology, not a garust
workspace member. Capabilities the engine needs but garust lacks are added
*to garust* through garust's own RFC/PR process, then consumed here —
"add to garust as needed."

The non-obvious domain facts:

- **Motors are the only motion representation.** A pose is one PGA
  `Motor3`; the in-between of two keyframes is `Motor::slerp`, a
  constant-speed screw. No Euler angles, no position-lerp + quaternion-slerp
  pairing, no gimbal handling. Matrices appear exactly once, inside the
  camera projection (and via `Motor::to_matrix` as a bulk-transform
  fast path).
- **Easing never changes the path, only the schedule.** Ease functions remap
  the local parameter `t` before `slerp`; screw geometry and timing decouple
  cleanly.
- **Incidence is computed, not authored.** Derived shapes (`JoinLine` through
  two objects, `MeetPoint` of two planes) are resolved from the *transformed*
  geometry every frame via PGA `join`/`meet` — the Manim constraint-driven
  drawing trick, for free.
- **Deterministic offline rendering, not a game engine.** Same scene → same
  frames, bit-for-bit. No real-time playback, windowing, input, audio, or
  timeline UI. Video encoding stays outside the crate.
- **Anchored text labels are IN scope (owner decision, 2026-08-27).**
  Previously an explicit non-goal. Reversed because the project now has an
  audience: an explanatory video that cannot name an axis, a quantity, or a
  body is not an explainer. Scope is deliberately narrow — plain strings
  anchored to world geometry (a point, a body's pose, or a screen corner),
  so a label can ride a moving object. **No layout engine, no LaTeX, no
  rich text**; those remain out of scope and their absence is documented,
  not hidden.
- **The encode path must work with stock ffmpeg (owner decision,
  2026-08-27).** The documented SVG command was found broken in practice:
  ffmpeg ships an `svg_pipe` demuxer but no SVG *decoder* unless built with
  librsvg, so `-i frame_%05d.svg` fails on an ordinary install. The raster
  sink (P6 PPM) is the answer — verified to encode with stock ffmpeg and no
  external rasterizer — and is promoted out of M4 accordingly.
- **Zero dependencies in the core** — the garust discipline. Frame sinks are
  pure text SVG and binary P6 PPM. `std` is required (file I/O).
- **Angles are radians measured against TAU**, matching garust convention.
- **Physics is a motion source, not a separate system.** garust-physics
  (garust RFC-010) integrates on the motor group: a body's pose *is* a
  `Motor`, its velocity and momentum are single bivectors, contacts are PGA
  incidence. A simulation rollout therefore produces exactly what a keyframe
  track holds — `(time, Motor)` samples. Keyframed and simulated tracks are
  the same currency; the render loop never knows the difference.
- **Simulation must preserve determinism.** Physics runs offline at a fixed
  timestep before rendering; frames sample the recorded rollout. Same scene,
  same frames — bit-for-bit **on the same machine**, or the tests fail.
  Cross-platform bit-exactness is *not* claimed for trig-bearing content:
  `sin`/`cos` may differ in the last ulp between platforms and libm
  versions (garust capability audit, 2026-08-20), and a rollout is
  trig-heavy. Checked-in golden fixtures are therefore trig-free by
  construction, so CI agrees on any host (R-0003 §2.6, R-0004 §4).
- **GA quantities are first-class drawables.** Where Manim explains by
  construction, motoreel also explains by *dynamics*: velocity/momentum
  bivectors as oriented plane glyphs, the instantaneous screw axis of a
  motor, forque lines, energy exchange — rendered live from the simulation
  state, the way `JoinLine`/`MeetPoint` are computed live from geometry.
- **garust dependency:** path dependency on `../garust` during development
  (version fallback for release). Hot paths to reach for: `apply_point_fast`
  (19× single point), `apply_each_simd` (3.2× on `f32x8`, `simd` feature),
  `Motor::to_matrix` for a once-per-frame matrix handoff (garust RFC-001
  App. E/G). Motor splines (`Motor::bezier`) are reachable
  upstream; kinematic chains (garust RFC-013) exist but `garust::chain` is
  not yet re-exported by the umbrella crate (upstream fix queued). The `simd`
  feature pulls the `wide` crate — whether motoreel accepts that dependency
  is an M4 decision, deliberately deferred; M1 uses the scalar paths.

## Milestone themes

Mirrored into `ROADMAP.md`:

- **M1 — Motion core & first light:** `Track`/`Ease` evaluation,
  `Scene`/`Object`/`Camera` with projection, `SvgSink` — a 240-frame demo
  that ffmpeg encodes.
- **M2 — Physics playback:** record a garust-physics `World` rollout into
  motor tracks (fixed dt, deterministic) and render it — a tumbling body
  (R-0004, with `Shape::Edges` for wireframe solids) and a pendulum
  (R-0005, joints) as the first *simulated* explanatory videos.
- **M3 — GA mechanics visuals:** the study-companion layer — bivector and
  screw-axis glyphs, momentum/velocity overlays, plus derived incidence
  shapes (`JoinLine`, `MeetPoint`) computed live each frame.
- **M4 — Raster & throughput:** `PpmSink` with a stroke rasterizer; SIMD
  fast path in the render loop (≥3× on a 10k-vertex scene).
- **Later (explicitly out of scope until re-decided):** `wgpu` real-time
  backend, text/LaTeX overlay, audio.
