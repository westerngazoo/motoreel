# SPEC-0004 — Rollout recording, wireframe edges, and the tumbling box

- **Status:** Draft — architect review 2026-08-21 (REQUEST CHANGES) applied; awaiting owner acceptance
- **Realizes:** R-0004
- **Author:** Claude (main session) with owner
- **Created:** 2026-08-21
- **Depends on:** SPEC-0001, SPEC-0002, SPEC-0003
- **Module(s):** `crates/motoreel/src/record.rs`; amendments to `prim.rs`,
  `object.rs`, `scene.rs`, `svg.rs`; `examples/tumbling_box/`

## 1. Motivation

R-0004: step a `garust-physics` world at fixed `dt`, record each body's
pose into an ordinary `Track`, and render it with no simulation-aware code
anywhere downstream. Plus the wireframe vocabulary a solid body needs, and
the demo that proves the whole chain — a freely tumbling box.

## 2. Design

### 2.0 Module and test layout

Mirroring SPEC-0003 §2.1's conventions, which AC7 structurally reuses:

```
crates/motoreel/src/
  record.rs                    — `record` + `RecordError`
crates/motoreel/examples/tumbling_box/
  main.rs                      — renders to `out/tumbling/`; documents ffmpeg
  scene.rs                     — records the rollout, builds the Scene (shared)
crates/motoreel/tests/
  r0004_physics_playback.rs    — every AC (one file per requirement, house
                                 convention since SPEC-0003)
```

`lib.rs` gains `mod record;` and `pub use record::{record, RecordError};`
— the private-module-plus-re-export pattern of `ease`/`track`/`prim`/
`object`/`camera`/`scene`. As in R-0003, the demo's `scene.rs` is included
by the determinism test via `#[path = "../examples/tumbling_box/scene.rs"]
mod scene;`, so the shipped demo and the test cannot drift apart; it names
motoreel items via `motoreel::` and garust's via `garust::`, never
`crate::`/`super::`.

**No manifest change:** `crates/motoreel/Cargo.toml` already depends on
garust with `features = ["physics"]`, so R-0003 AC5's zero-new-dependency
test passes untouched.

### 2.1 The recorder — `record.rs`

One free function; no state, no builder, nothing to configure.

```rust
pub fn record(
    world: &World,
    bodies: &[Body],
    joints: &[Joint],
    dt: f64,
    steps: usize,
) -> Result<Vec<Track>, RecordError>
```

Returns one `Track` per body, in input order. The caller's `bodies` are
borrowed and copied internally (`Body: Copy`), so the initial state is
untouched and re-recording reproduces the rollout (AC3).

`joints` is taken even though R-0004 ships no joint demo (§4): `World::step`
requires the slice, and hardcoding `&[]` here would force a signature break
in R-0005. Callers with no joints pass `&[]`.

Times are **derived, never accumulated**: key `i` is at `i as f64 * dt`,
matching the discipline R-0003 uses for frame times. Key 0 is the initial
pose *before* any step, so `steps` steps yield `steps + 1` keys (AC1).

```
record:
  state = bodies.to_vec()                     # caller untouched
  keys[b] = [(0.0, state[b].pose())]         # key 0 = initial pose
  for i in 1..=steps:
      world.step(&mut state, joints, dt)
      for b: keys[b].push((i as f64 * dt, state[b].pose()))   # Body::pose
  keys.map(Track::keys)                       # per-body tracks
```

`RecordError` is typed per constitution §6:

- `InvalidTimestep` — `dt` not finite or `<= 0`.
- `Track(TrackError)` — a track could not be built from the recorded keys.
  Reachable only when `steps as f64 * dt` overflows to infinity (an absurd
  `dt`), but surfaced rather than unwrapped; `From<TrackError>` is
  implemented so the constructor's error propagates with `?`. Verified
  reachable: `dt = 1e308, steps = 2` returns
  `Track(NonIncreasing { index: 2 })` with no panic.

`RecordError` implements `Display` and `std::error::Error` like its sibling
`TrackError`, with `source()` returning the inner `TrackError` for the
`Track` arm — constitution §6, and the demo's `main` will want it in a
`Box<dyn Error>`.

`steps = 0` is valid and yields single-key tracks — a hold at the initial
pose, consistent with `Track::hold`. An empty `bodies` slice yields an
empty `Vec`; there is nothing to record and nothing to fail.

**Boundary (enforced, not asserted).** Within `src/`, `record.rs` is the
only module that names `garust::physics`; it imports `World`, `Body`,
`Joint` and calls exactly `step` and `Body::pose()` (which exists upstream
precisely to save the `.rigid` hop for renderers). Examples necessarily
build a `World` and a `Body`, so the rule scopes to the library. AC2 makes
this checkable the way R-0003 AC5 checks dependencies — by asserting over
source text — rather than leaving it aspirational.

### 2.2 Wireframe geometry — amending SPEC-0002's vocabulary

`Shape` gains a variant, and `Prim2` gains its mirror:

```rust
Shape::Edges(Vec<(pga::Point, pga::Point)>)     // model space
Prim2::Edges { segments: Vec<(Pt2, Pt2)>, style: Style }   // image space
```

One object, one track, one style, one primitive — the 1:1
object→primitive shape of `Scene::eval` is preserved, and with it the
whole-primitive cull rule: if **any** endpoint of **any** edge fails,
the entire wireframe is dropped (AC5). A partially drawn solid would
misrepresent the geometry exactly the way a partially drawn polyline
would.

`Object::edges(Vec<(Point, Point)>)` joins the existing constructors.

### 2.3 The SVG template — amending SPEC-0003's grammar

One pinned template, one element, following §2.5's rules exactly (default
`Display` floats, fixed attribute order, style attributes always emitted):

```
Edges → <path d="M {x0},{y0} L {x1},{y1} M {x2},{y2} L {x3},{y3}" fill="none" stroke="{#hex}" stroke-width="{w}" stroke-opacity="{a}" stroke-linecap="round"/>
```

A `<path>` with repeated `M`/`L` pairs rather than N `<line>` elements:
one element per primitive is R-0003 AC2's rule, and one element carries
one style. Pair separator is a single space, `x,y` uses a comma — the same
convention as `polyline`'s `points`. `stroke-linejoin` is deliberately
absent where `polyline` carries it: `M`/`L` subpairs are disjoint segments
with no joins, so the attribute would be inert — not an inconsistency to
"fix" later.

An empty edge list emits `d=""`, which renders nothing and stays
well-formed (SVG 1.1 makes the command group optional; SVG 2 says an empty
`d` simply does not render). This follows the precedent already shipped for
an empty `Polyline` → `points=""`: handling it any other way would add a
length-dependent branch to `eval` and break the 1:1 object→primitive
invariant AC5 protects.

**R-0003's golden fixture contains no edges, so its bytes are unchanged**
(AC6) — the existing byte-exact test must keep passing untouched, which is
the cheapest possible proof that this amendment is additive.

### 2.4 The demo — `examples/tumbling_box/`

A free rigid body with three distinct principal moments, spun about its
**intermediate** axis with a small perturbation, flips end-over-end
periodically: the intermediate axis theorem (Dzhanibekov effect). Every
number below was measured empirically against this integrator, not
derived on paper (probe, 2026-08-21):

| parameter | value | note |
|---|---|---|
| box | 1.60 × 1.00 × 0.20 | half-extents 0.80 / 0.50 / 0.10 |
| mass | 1.0 | |
| `Ix` | `(m/12)(dy² + dz²)` ≈ 0.0866666666666667 | min |
| `Iy` | `(m/12)(dx² + dz²)` ≈ 0.2166666666666667 | **intermediate** |
| `Iz` | `(m/12)(dx² + dy²)` ≈ 0.2966666666666667 | max |

**The formulas are normative, not the decimals** (qa-run finding 2: a
literal and its own formula disagreed by 1 ulp). The demo computes the
moments from the box dimensions, so no inertia constant is hand-written;
flip timings are insensitive to the spelling anyway (four spellings,
identical crossings).
| `ω₀` | `p[0]·0.5 + p[1]·10.0` rad/s | `p = Inertia::principal_planes()`; the 0.5 is the perturbation, 2 % of the y spin |
| `Π₀` | `RigidBody::spinning(Motor3::identity(), &inertia, ω₀)` | upstream applies `Π = I·ω` itself — no hand-computed momentum literals |
| `dt` | 1/240 | 960 steps over 4 s; four per rendered frame |
| gravity | `[0, 0, 0]`, no ground | free rotation, no collisions |

Measured behaviour: **3 flips** at t ≈ 0.688 / 2.058 / 3.425 s (period
≈ 1.37 s), each transition ≈ 0.46 s ≈ 28 frames — readable, not a snap.
Peak |ω| = 11.86 rad/s = 11.3° per frame at 60 fps, so no strobing. Over
4 s, |L| drifts −8.6e−13 %, its direction by less than the print
resolution, and rotational energy stays in a bounded ±7e−4 % band with no
secular drift (7.4e−4 % full band).

**The perturbation must be explicit.** With ε exactly zero the body never
flips, in this integrator, forever — roundoff does not seed the
instability. (Verified, along with the control that the same nudge about
the min- or max-moment axis never flips.)

**Camera.** `L̂` is fixed at ≈ world `+y` for the whole rollout, so the
camera must look *perpendicular* to it or the flip foreshortens into
near-invisibility. Pinhole at `translator(0, 0, 3.5)`, focal `2.5`, up
`+y`. The centre of mass never moves, so every corner stays exactly
`‖(0.8, 0.5, 0.1)‖ = 0.9487` from the origin; the worst-case image
coordinate over that whole sphere is `0.704`, inside the default view's
`0.9` half-height with ≈ 22 % margin.

**Sampling note (honest).** Recorded key times are `i · dt` and frame
times are `j / 60`; these agree mathematically but not necessarily to the
last bit, so a frame may sample a hair off a key and slerp with a
parameter of order **1e−14** (a 5.6e−17 time offset over a 4.17e−3 span;
19 of the 241 frame times differ by exactly 1 ulp). The resulting pose
offset is ≈7e−16 rad — immaterial visually and fully deterministic, so
byte-identity is untouched. The spec does not claim exact key landing.

### 2.5 Determinism

Recording is a pure function of `(world, bodies, joints, dt, steps)`:
`World::step` carries no randomness, no map iteration, no clock, and no
threads (garust audit), and the recorder adds only `Vec` pushes in slice
order. Two recordings in one process are therefore bit-identical, and so
are the frames rendered from them (AC4, AC7) — **on the same machine**;
the rollout is trig-heavy and R-0004 §4 declines the cross-platform claim.

### 2.6 garust surface used

`garust::physics::{World, Body, Joint}`, `World::step`,
`RigidBody::pose()`; `Inertia::principal` and `principal_planes` in the
demo only. Deliberately unused: `RigidBody::twist`/`momentum`/`ScrewAxis`
— they exist upstream but glyphs are M3.

**Convention, empirically pinned (do not hand-write blades).**
`Inertia::principal([Ix, Iy, Iz])` indexes rotation about body x/y/z, and
`principal_planes()` returns the matching unit bivectors `[e23, e31,
e12]`. Slot 1 is stored as `e13` and `e31 = −e13`, so
`Pga3::basis(0b0101)` carries the **wrong sign** for the y axis. Write
angular momentum as `p[0]·Lx + p[1]·Ly + p[2]·Lz`; read it back with
`twist_parts().1`.

## 3. Code outline

```rust
// record.rs — the only place motoreel touches physics
use garust::physics::world::{Body, Joint, World};
use garust::Motor3;
use crate::track::{Track, TrackError};

/// Why a rollout could not be recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]   // + Display, Error, From<TrackError>
pub enum RecordError {
    /// `dt` was not finite, or was not greater than zero.
    InvalidTimestep,
    /// The recorded keys did not form a valid track.
    Track(TrackError),
}

pub fn record(
    world: &World, bodies: &[Body], joints: &[Joint], dt: f64, steps: usize,
) -> Result<Vec<Track>, RecordError> {
    if !(dt.is_finite() && dt > 0.0) {
        return Err(RecordError::InvalidTimestep);
    }
    let mut state = bodies.to_vec();                       // caller untouched
    let mut keys: Vec<Vec<(f64, Motor3)>> =
        state.iter().map(|b| vec![(0.0, b.pose())]).collect();
    for i in 1..=steps {
        world.step(&mut state, joints, dt);
        let t = i as f64 * dt;                             // derived, not summed
        for (k, b) in keys.iter_mut().zip(state.iter()) {
            k.push((t, b.pose()));
        }
    }
    keys.into_iter().map(|k| Ok(Track::keys(k)?)).collect()
}
```

```rust
// svg.rs — the added arm (byte grammar per SPEC-0003 §2.5)
Prim2::Edges { segments, style } => {
    self.buf.push_str("<path d=\"");
    for (i, (a, b)) in segments.iter().enumerate() {
        if i > 0 { self.buf.push(' '); }
        let _ = write!(self.buf, "M {},{} L {},{}", a.x, a.y, b.x, b.y);
    }
    let _ = writeln!(self.buf, "\" fill=\"none\" stroke=\"{}\" \
        stroke-width=\"{}\" stroke-opacity=\"{}\" \
        stroke-linecap=\"round\"/>", hex(style), style.width, style.alpha);
}
```

## 4. Non-goals

- No joints or pendulum (R-0005), no collisions, no ground plane, no live
  stepping during rendering.
- No shape library: the demo builds its own 12 edges from half-extents.
  A `box_wireframe` helper waits until a second caller wants it.
- No GA glyphs (M3), no `Body` construction helpers upstream, no
  sub-stepping policy, no adaptive `dt`.

## 5. Open questions

None — the four shaping decisions are in R-0004's log; the demo's
parameters are measured, not chosen.

## 6. Acceptance criteria

- [ ] **AC1** — `record` of `n` steps yields keys at exactly
  `i as f64 * dt`, key 0 being the pre-step pose; per body, in input order.
  **Observed through `Track::eval`**, since `Track` exposes no key
  iterator, no `len`, and deliberately no `PartialEq`: evaluating at each
  derived key time returns that key's motor bit-exactly (R-0001 AC1, the
  mechanism `ac8_dense_rollout_track_is_first_class` already relies on).
  Key *count* is proxied by clamping: evaluating past
  `steps as f64 * dt` returns the same motor as evaluating at it.
- [ ] **AC2** — a recorded track drives an `Object` through
  `Scene::eval`/`render` unchanged; and no `src/` module other than
  `record.rs` names `garust::physics`, asserted over source text the way
  R-0003 AC5 asserts the dependency set.
- [ ] **AC3** — the caller's `bodies` slice is unmodified after recording;
  two recordings from the same inputs agree.
- [ ] **AC4** — two recordings in one process are bit-identical —
  `to_bits` over all 16 coefficients of `eval` at every derived key time
  (the observable form of "every key motor", per AC1) — and frames
  rendered from them are byte-identical.
- [ ] **AC5** — `Shape::Edges` emits one `Prim2::Edges`; one failing
  endpoint culls the whole wireframe; no non-finite coordinate escapes.
- [ ] **AC6** — the edge template matches its pinned bytes, and R-0003's
  golden fixture still passes **untouched**.
- [ ] **AC7** — the demo renders 240 frames showing ≥ 2 flips, twice,
  bit-identically. The flip count is asserted **from the recorded track's
  poses**, not from pixels and not from simulation state the recorder
  discards: transform the body's `+y` axis by the pose at each key and
  count sign reversals of its world y-component
  (`pga::Point::new(0.0, 1.0, 0.0).transform(&pose).to_euclidean().1`).
  This is a large-amplitude signal — it swings between ±1 — with measured
  crossings at t ≈ 0.6875 / 2.0542 / 3.425 s.
- [ ] `RecordError`: `InvalidTimestep` for `dt ∈ {0, −1, NaN, ∞}`;
  `steps = 0` gives single-key holds; empty `bodies` gives an empty `Vec`.

## 7. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-08-21 | `record` is a free function returning `Vec<Track>`, not a builder or a `Rollout` type | Nothing to configure and nothing to keep; the output is ordinary track data, which is the entire point of the requirement |
| 2026-08-21 | Caller's bodies borrowed and copied internally | AC3 makes non-mutation testable; `Body: Copy` makes it free |
| 2026-08-21 | `Prim2::Edges` mirrors `Shape::Edges`; rendered as one `<path>` | Preserves the 1:1 object→primitive invariant and the whole-primitive cull rule; one element carries one style, per R-0003 AC2 |
| 2026-08-21 | Demo parameters taken from empirical measurement, incl. the explicit perturbation | ε = 0 never flips in this integrator; paper values would have produced a still box and a silent failure |
| 2026-08-21 | No `box_wireframe` helper yet | One caller; three similar lines beat a premature abstraction (§2) |
| 2026-08-21 | qa-run decisions: inertia **formulas** are normative (demo computes moments from dimensions); AC2's boundary check asserts the literal `garust::physics` and accepts that garust's root re-exports are a documented loophole (a broader token list false-positives on doc prose); AC7 keeps the tight ±0.05 s crossing assertion, which deliberately pins ε — changing the perturbation is a spec change; AC7 checks "exactly one twelve-edge wireframe is present" so a future decorative object cannot fail QA; `main.rs` carries the ffmpeg line by convention without its own AC | qa run findings 1–6; each was reported rather than invented, per the loop |
| 2026-08-21 | Architect findings 1–12 applied: ACs restated in terms the public API can actually observe; `RecordError` gains `Display`/`Error`/`source`; Π₀ built by `RigidBody::spinning` instead of hand-computed literals; physics boundary scoped to `src/` and made testable; layout section added | Architect review (REQUEST CHANGES → resolved). Findings 1–2 were blocking: AC1/AC4/AC7 described observations `Track` and the recorder's return type do not permit, and would have stalled the qa run |

## Changelog

- 2026-08-21 — created.
