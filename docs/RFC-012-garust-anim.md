> **Provenance:** copied from `garust/rfcs/RFC-012-garust-anim.md`
> (garust commit `d31fba3`, merged as PR #50). Founding design document for
> **motoreel**. §4 (placement) is superseded: the owner chose a standalone
> project; garust gains capabilities upstream as needed. §2 non-goals are
> amended by `project-specifics.md`: physics-driven motion is IN scope.

# RFC 012: `garust-anim` — Motor-Native Mathematical Animation

**Author:** garust maintainers
**Status:** Draft / Request for Comments
**Target:** `garust` (Geometric Algebra in Rust)
**Builds on:** RFC-009 (PGA kernel `Cl(3,0,1)`), RFC-001 (performance)
**Numbering note:** RFC-012 sits outside the reserved R-0008→R-0011 program
block; R-0010/R-0011 remain separate flows and are not affected.

## 1. Context and motivation

Nothing Manim-like exists on top of garust — or, as far as a survey of
public code shows, on top of *any* GA library with motors as the animation
primitive. Yet the library already contains, tested and benchmarked, exactly
the mathematical layer such a tool needs:

- **Keyframing** — a pose is one `Motor`; the in-between is
  `Motor::slerp`, a constant-speed screw. No separate position lerp +
  quaternion slerp, no gimbal handling, no renormalization drift
  (`Motor::renormalize` exists if a long chain ever needs it).
- **Easing** — remap `t` before `slerp`; the path is unchanged, only the
  schedule. Screw geometry and timing decouple cleanly.
- **Group choreography** — `Motor::frechet_mean` gives the pivot of a
  formation; `log`/`exp` give blend spaces for morphing between motions.
- **Scene geometry** — typed `pga::{Point, Line, Plane}` with `join`/`meet`
  mean incidence relations (the line through two animated points, the
  intersection of two moving planes) are *computed live each frame*, not
  authored — the core Manim trick of constraint-driven drawings, for free.
- **Throughput** — `apply_each_simd` (3.2× on `f32x8`),
  `apply_point_fast` (19× single-point), and `Motor::to_matrix` for a
  once-per-frame matrix handoff put thousands of animated points per frame
  well inside budget (RFC-001 Appendix E/G).

The purpose of the crate is the same as Manim's: *explanatory* video —
render a deterministic scene description to frames, offline. It is not a
game engine and not real-time; those constraints (below) keep it small.

## 2. Goals and non-goals

**Goals**

1. A `Scene` → frames pipeline: describe objects and motor tracks, get
   numbered frame files a video encoder can consume.
2. Zero dependencies in the core, like every other garust crate: frame
   sinks that need no external crates (SVG per frame; PPM raster), with
   encoding delegated to the user's `ffmpeg` invocation.
3. Motors as the *only* motion representation. No Euler angles, no
   matrices in the public animation API (matrices appear once, inside the
   camera projection).
4. Deterministic output: same scene, same frames, bit-for-bit.

**Non-goals**

- Real-time playback, windowing, input (a `wgpu` backend can be a later
  RFC if wanted).
- Text layout / LaTeX. Frame sinks emit geometry and stroke styles; labels
  can be overlaid downstream.
- Audio, timeline UI, interactive editing.

## 3. Design

### 3.1 Crate layout

A new workspace member `crates/garust-anim`, re-exported by the umbrella
crate behind an off-by-default `anim` feature — the `garust-physics`
precedent exactly:

```toml
[features]
anim = ["dep:garust-anim"]
```

`garust-anim` depends only on `garust-core` and `garust-geo`. `std` is
required (file I/O); there is no `no_std` story for a frame writer.

### 3.2 Core types

```rust
/// A drawable: typed PGA geometry plus stroke style.
pub enum Shape {
    Point(pga::Point),
    Segment(pga::Point, pga::Point),
    Polyline(Vec<pga::Point>),
    // Derived each frame from live geometry:
    JoinLine(ObjectId, ObjectId),      // line through two objects' anchors
    MeetPoint(ObjectId, ObjectId),     // intersection of two planes
}

pub struct Object { shape: Shape, style: Style, track: Track }

/// A motor keyframe track: (time, pose) pairs + easing per span.
pub struct Track { keys: Vec<(f64, Motor3)>, ease: Ease }

pub enum Ease { Linear, SmoothStep, SmootherStep, Custom(fn(f64) -> f64) }

pub struct Camera { pose: Motor3, focal: f64 }   // pose is a motor too

pub struct Scene { objects: Vec<Object>, camera: Camera, duration: f64 }
```

Track evaluation is the whole animation engine, and it is five lines:
find the surrounding keys `(t0, m0)`, `(t1, m1)`, ease the local
parameter, and return `m0.slerp(&m1, s)`. Everything else — arcs,
spirals, screws — falls out of *which motors* the keys hold, because the
interpolant is always the geodesic screw between them.

### 3.3 Frame sinks

```rust
pub trait FrameSink {
    fn frame(&mut self, index: usize, prims: &[Prim2]) -> io::Result<()>;
}
```

- **`SvgSink`** (default): one `frame_%05d.svg` per frame. Pure text,
  zero deps, resolution-independent, diffable in review.
- **`PpmSink`**: binary P6 PPM with a software rasterizer for strokes.
  Zero deps; `ffmpeg -i frame_%05d.ppm` encodes it directly.

Encoding stays outside the crate:

```bash
ffmpeg -framerate 60 -i out/frame_%05d.svg -pix_fmt yuv420p demo.mp4
```

### 3.4 Render loop

```rust
scene.render(60.0, &mut SvgSink::new("out/")?)?;
```

Per frame: evaluate every track at `t` → motors; apply to shape geometry
(`apply_each_simd` for polylines past a length threshold, otherwise
`apply_point_fast` per vertex); resolve derived `JoinLine`/`MeetPoint`
shapes from the *transformed* geometry; project through the camera
(`camera.pose.inverse()` then pinhole divide); hand `Prim2` list to the
sink. All angles in the API are radians measured against **TAU**.

### 3.5 Worked example (target API)

```rust
use garust::{anim::*, pga, Motor, Pga3};
use std::f64::consts::TAU;

let mut scene = Scene::new(4.0);                    // 4 seconds

// A square that travels a full screw: one turn about z while rising.
let screw_end = Motor::translator(0.0, 0.0, 2.0)
              * Motor::rotor(TAU, Pga3::basis(0b0011));
scene.add(
    Object::polyline(square(1.0))
        .track(Track::keys([(0.0, Motor::identity()), (4.0, screw_end)])
        .ease(Ease::SmootherStep)),
);

// A point orbiting it — and the *live* line joining the two, recomputed
// from incidence each frame rather than animated by hand.
let orbiter = scene.add(Object::point(pga::Point::new(2.0, 0.0, 0.0))
    .track(Track::spin(TAU / 4.0, Pga3::basis(0b0011), 4.0)));
scene.add(Object::join_line(orbiter, scene.object(0)));

scene.render(60.0, &mut SvgSink::new("out/")?)?;    // 240 SVG frames
```

## 4. Placement: workspace member vs. separate repo

Recommendation: **workspace member**, for the same reasons
`garust-physics` is one — it exercises the public API of `garust-geo`
from the consumer side, versions in lockstep with the kernel it
showcases, and keeps the zero-dep discipline enforceable in one CI run.
The counterargument (repo bloat, different release cadence) becomes real
only if a `wgpu`/GPU backend lands; that backend, if ever, should be the
thing that moves out, not the core.

## 5. Milestones

| # | Deliverable | Acceptance |
|---|---|---|
| A1 | `Track` + `Ease` + evaluation | proptest: track eval at key times returns the key motors; midpoint of a pure-translation pair is the half translation |
| A2 | `Scene`/`Object`/`Camera`, projection | golden-file test: one frame of a known scene matches checked-in SVG byte-for-byte |
| A3 | `SvgSink` | 240-frame demo renders; frames encode with ffmpeg |
| A4 | `PpmSink` + stroke rasterizer | PPM golden test; visual parity spot-check vs SVG |
| A5 | Derived shapes (`JoinLine`, `MeetPoint`) | incidence proptest: derived line passes through both transformed anchors every frame |
| A6 | SIMD fast path in the render loop | bench: 10k-vertex scene per-frame cost, target ≥3× over scalar loop |

## 6. Open questions

1. Should `Ease::Custom` take `fn` (zero-dep, `Eq`-friendly) or a boxed
   closure (ergonomic, but breaks `Scene: Clone` determinism guarantees)?
   Draft says `fn`.
2. Camera model: pinhole only, or also orthographic for the classic
   "math diagram" look? Draft says both — it is one `enum` with two
   divide rules.
3. Color/style vocabulary: minimal (`stroke`, `width`, `alpha`) now;
   defer gradients and fills until a scene needs them.
