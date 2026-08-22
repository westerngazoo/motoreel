# SPEC-0002 — Scene, objects, motor-posed camera, projection

- **Status:** Draft — architect review 2026-08-20 (REQUEST CHANGES) applied; awaiting owner acceptance
- **Realizes:** R-0002
- **Author:** Claude (engineer session)
- **Created:** 2026-08-20
- **Depends on:** SPEC-0001 (Draft — assumptions reconciled in §2.9)
- **Module(s):** `crates/motoreel/src/prim.rs`, `crates/motoreel/src/object.rs`,
  `crates/motoreel/src/camera.rs`, `crates/motoreel/src/scene.rs`

## 1. Motivation

R-0002 is the bridge from motor mathematics to drawable frames: a `Scene` of
`Object`s (typed PGA geometry + stroke style + motor track) and a `Camera`
whose pose is itself a `Motor3`, evaluated at a time `t` into a flat list of
2D primitives (`Prim2`) that a frame sink can consume without knowing 3D ever
existed. (RFC-012 §3.2, §3.4, milestone A2.)

The no-matrices promise is honored with room to spare: **this design contains
no matrix at all**. The view transform is a motor applied by sandwich; the
projection itself is two multiplications and one divide per coordinate.
`Motor::to_matrix` remains an M4 throughput option *inside* the render loop's
bulk path (RFC-012 A6) — nothing here forecloses it, nothing here needs it.

## 2. Design

### 2.1 Frames and conventions

All scalars are `f64` (`Motor3`, `garust::pga::Point` defaults); `f32`/SIMD
is M4. One pipeline of spaces, each step a motor except the last:

```
model ──(object pose: track.eval(t))──▶ world
world ──(camera.pose.inverse())──────▶ view
view  ──(projection rule)────────────▶ image      ← the only non-motor step
```

- **View frame:** right-handed, `+x` right, `+y` up, the camera looks along
  **−z** (the GL/math convention). The **view depth** of a point with view
  coordinates `(x, y, z)` is `d = −z`: positive in front of the camera,
  negative behind, zero on the camera plane.
- **Image space:** origin on the optical axis, `x` right, `y` up. Units are
  focal-scaled view units (pinhole) or view units (orthographic). Mapping
  image space to pixels/viewBox — including the y-flip SVG wants — is
  SPEC-0003's business.
- `focal` is a **length in world units** (center of projection to image
  plane), not an angle; no angles appear in this spec. A camera at
  `Motor3::translator(0.0, 0.0, 5.0)` looks at the origin and sees the
  xy-plane upright, x right, y up, with no flips.

### 2.2 The 2D output vocabulary — `prim`

`prim.rs` is the sink-facing boundary: `Pt2`, `Rgb`, `Style`, `Prim2`. It
depends on `std` alone — no garust type crosses it, which is the concrete
form of "the SVG sink never learns 3D existed".

`Prim2` is an enum mirroring `Shape` (`Point` / `Segment` / `Polyline`),
each variant carrying its own `Style`. An enum rather than a length-encoded
point list because the sink must not guess whether one point means a dot or
a degenerate polyline. `Style` is the minimal vocabulary R-0002 decided:
`stroke: Rgb` (8-bit sRGB), `width: f64`, `alpha: f64`.

**Invariant (load-bearing):** every geometry coordinate (`Pt2` value)
inside an emitted `Prim2` is finite. `Scene::eval` upholds it structurally
(§2.5); sinks and golden tests may rely on it, and it is what makes
`PartialEq` on emitted geometry a meaningful equality (no NaN ever
compares). **Style fields are outside the invariant** (qa-run decision,
2026-08-20): AC4 promises passthrough *unchanged*, so an out-of-contract
author style (a NaN `width`) flows through verbatim rather than being
silently sanitized — style honors its documented §2.8 contract, coordinates
are guaranteed unconditionally.

### 2.3 Scene content — `object`

`Shape` holds model-space geometry as typed `garust::pga::Point`s:
`Point`, `Segment`, `Polyline` — exactly R-0002's list; derived incidence
shapes are M3 (R-0007). An `Object` is `{ shape, style, track }`, all fields
public (plain data, any combination is valid). Constructors
`Object::point/segment/polyline` give the default style and a **single-key
identity track at `t = 0`** — the static object of AC5 is not a special
case, just a one-key track (valid per R-0001 AC5, clamping per AC4).
Builders `with_style`, `with_track`, and `at(pose)` (hold a fixed pose)
complete the authoring surface. The single-key construction is SPEC-0001's
infallible `Track::hold(pose)` — hoisted upstream at this spec's request
(SPEC-0001's decision log records the reconciliation) — so no fallible path
and no `expect` appears here.

A `Polyline` with fewer than two points passes through unvalidated: the
pipeline is total, and drawing nothing (or a dot) is the sink's honest
rendering of empty geometry. Closed shapes are authored by repeating the
first point.

### 2.4 Camera and projection — `camera`

Per R-0002's decision log, one camera type, two divide rules:

```rust
pub struct Camera { pub pose: Motor3, pub projection: Projection }
pub enum Projection { Pinhole { focal: f64 }, Orthographic }
```

`pose` is world-from-camera; the view transform is `pose.inverse()`. Camera
moves are therefore authored in the same currency as object motion — a
motor — though attaching a `Track` to the camera is a later requirement
(§4). Projection of a **view-space** point `(x, y, z)` with `d = −z`:

- **Pinhole:** `(focal · x / d, focal · y / d)`. The image plane sits *in
  front of* the center of projection (graphics convention), so there is no
  image inversion.
- **Orthographic:** `(x, y)`. The depth is used only by the cull test.

`Projection::project` is `pub` (adjudicated — decision log): it is AC3's
direct test point, it lets users project annotation anchors, and its doc
names the argument a **view-space** point.

`focal` carries a documented contract — finite and `> 0` — with no runtime
validation: the fields are public per the decided plain-data shape, a
non-positive focal is not a *failure mode* but malformed input (GIGO, as
with `std` float functions), and the finite-guard in §2.5 still keeps the
output NaN-free even then. Convenience constructors `Camera::pinhole(pose,
focal)` and `Camera::orthographic(pose)` exist; `Camera::default()` is
orthographic at `translator(0, 0, 5)` — the zero-surprise math-diagram
default: a unit square in the xy-plane projects to a unit square, and
default content sits comfortably in front of the default eye.

### 2.5 Culling policy — cull whole primitives, never NaN (R-0002 §4)

**The rule.** A vertex *passes* iff its view depth satisfies
`d.is_finite() && d >= Projection::NEAR` **and** its projected image
coordinates are finite. A primitive is emitted iff **every** vertex passes;
otherwise the **whole primitive is dropped**. `NEAR = 1e-9`. The same rule
applies to both projection models. Culling therefore happens per primitive,
decided by per-vertex tests, inside `Scene::eval` — there is no clipping,
no partial polyline, no substituted sentinel point.

**Why whole-primitive drop (and not the alternatives):**

- *Near-plane segment clipping* would manufacture new interpolated vertices
  — more floating-point paths, a far larger test surface, and visible
  policy choices (clip color? cap style?) — to serve a case (the camera
  plane slicing through an object mid-shot) that explanatory scenes do not
  stage. If a fly-through ever needs it, clipping is an additive later
  spec; nothing here blocks it.
- *Dropping only the offending vertices* of a polyline would silently
  reshape geometry — a polyline missing interior points draws a different
  figure, which is worse than drawing nothing.
- Whole-drop is total, branch-simple, conservative, and obviously correct:
  a primitive is either exactly the projection of its shape or absent.

**Why the rule is uniform across both models:** a camera keeps a facing
direction under parallel projection too; one rule means one code path, one
documented policy, and camera/world duality (AC2) that is literally the
same test for both models. The cost is that orthographic content must sit
in front of the camera — the contract of every renderer, and the default
camera already stands back 5 units. (Alternative — orthographic projects
everything, cull is pinhole-only — weighed at review and declined:
uniformity stays, and the orthographic constructor documents the in-front
contract; decision log, 2026-08-20.)

**Why `NEAR` is a strictly positive constant, not zero:** with `d > 0` but
subnormal, `focal · x / d` overflows to `±inf` — not NaN, but poison for a
sink all the same. `1e-9` is far below any authored scene scale (units ~1)
and far above f64 noise at that scale (~1e-16). The trailing
`is_finite()` guard on the projected coordinates backstops everything else
(astronomical inputs, ideal points whose `to_euclidean` weight-divide
produced `inf`/NaN upstream), making the §2.2 invariant unconditional: a
NaN depth fails `is_finite`, a NaN coordinate fails the guard — **no
non-finite number can leave `Scene::eval`**, even for garbage input.

### 2.6 Scene evaluation — `scene`

`Scene { objects: Vec<Object>, camera: Camera, duration: f64 }`, plain and
public. `Scene::new(duration)` starts empty with the default camera;
`add(object) -> ObjectId` appends (draw order = insertion order; later
objects paint over earlier ones in the sink). `ObjectId` is an opaque
insertion-index newtype — inert until R-0007's derived shapes, included now
because it is the RFC's target API and costs one line. `duration` is
consumed by SPEC-0003's frame walk, not by `eval`.

```
eval(t):
  view = camera.pose.inverse()                     # once per eval
  for obj in objects (in order):                   # Vec order — no maps
      to_view = view ∘ obj.track.eval(t)           # one composed motor
      project every vertex of obj.shape through to_view, then projection
      all vertices pass  → push Prim2 (obj.style copied verbatim)   # AC4
      any vertex fails   → push nothing                             # §2.5
```

The per-object motor is composed **once** (`view.compose(&pose)` — pose
applies first, then view, matching garust's `a * b` = "b first") and each
vertex is transformed by **one** sandwich via the typed
`pga::Point::transform`. This is RFC §3.4's loop with one algebraic
simplification: transform-to-world and transform-to-view fuse into a single
motor application, halving the sandwiches and the accumulated rounding.
Scalar per-vertex transforms only in M1; `apply_each`/SIMD/`to_matrix`
bulk paths are M4 (RFC A6).

### 2.7 Determinism (AC6) and the duality property (AC2)

`eval` is a pure function of `(scene, t)`:

- **No maps:** the only collections are `Vec`s iterated in insertion order.
- **No accumulation:** nothing persists between calls — every quantity is
  recomputed from `t` (frame times themselves are derived `i / fps` in
  R-0003, never `t += dt`).
- **One code path:** IEEE-754 f64 arithmetic is deterministic for a fixed
  expression order; the compose-once/apply-once structure fixes that order.
- Track evaluation is bit-deterministic upstream (R-0001 AC7, fn-pointer
  `Ease::Custom`).

Hence AC6 is bit-exact: two evals of the same scene at the same `t` produce
`Vec<Prim2>` equal down to `f64::to_bits`.

AC2 (camera/world duality) is **mathematically exact, floating-point
approximate**: with camera pose `C` moved by `M`, the composed view motor is
`(M·C)⁻¹·Q = C⁻¹·M⁻¹·Q`, identical to leaving the camera at `C` and
premultiplying every object pose by `M⁻¹` — but the two sides round
differently (the user composes on different ends), so the property test
compares within tolerance, not bit-for-bit. Duality also survives *animated*
tracks: garust's screw geodesic is left-invariant
(`slerp(M·a, M·b, s) = M·slerp(a, b, s)`, since the delta motor conjugates
and `log`/`exp` carry the conjugation), so static-key generators lose no
generality.

### 2.8 Errors

**This spec adds no error type.** All construction is plain data with no
invalid state to reject; evaluation is total — the same decision R-0001
recorded ("no error path in the render loop"). Numeric fields carry
documented contracts (`focal` finite `> 0`; `width` finite `≥ 0`; `alpha`
in `[0, 1]`) enforced by the never-NaN cull guard rather than by erroring.
The only fallible surface in the whole pipeline remains `Track`
construction (`TrackError`, SPEC-0001), which callers resolve before
`Object::with_track`. Constitution §6 is satisfied: nothing panics, nothing
is stringly-typed, and — single-key tracks coming from SPEC-0001's
infallible `Track::hold` — not even a justified `expect` exists in this
spec.

### 2.9 SPEC-0001 touchpoints

Drafted against SPEC-0001 (Draft, same day). What this spec consumes, and
what must stay true when either spec is revised:

1. `Track::eval(&self, t: f64) -> Motor3` — total, clamping at both ends
   (R-0001 AC4). Confirmed in SPEC-0001 §2.
2. Evaluation at a key's exact time returns that key's motor **exactly**
   (R-0001 AC1) — the golden test (§6 AC1) evaluates its animated object at
   its last key time and relies on this, not on `slerp` bit-behavior.
3. Single-key tracks are valid (`eases` empty) and evaluate to that key for
   every `t` — AC5's static object. Confirmed in SPEC-0001 §2.
4. `Track::hold(pose: Motor3) -> Track` — the infallible single-key
   constructor (key at `t = 0`), hoisted into SPEC-0001 at this spec's
   request; `Object` constructors and `at(pose)` consume it directly,
   deleting the planned private helper and its `expect`. `Track::keys` (the
   fallible multi-key surface) remains what callers resolve before
   `with_track`.
5. `Track: Clone` (R-0001 AC7 — confirmed) and `Track: Debug` (confirmed —
   SPEC-0001 derives it) so `Object`/`Scene` can derive both. `Track` has
   no `PartialEq` (confirmed: fn-pointer ease equality is
   codegen-dependent).
6. Track keys are renormalized at construction (SPEC-0001 key hygiene) —
   so posed geometry keeps unit weight and `to_euclidean` stays stable; no
   re-normalization is needed here.
7. Ease semantics are opaque to this spec: `eval(t)` already contains them.
8. `lib.rs` re-export style: SPEC-0001 items at crate root
   (`motoreel::{Track, TrackError, Ease}`) alongside this spec's (§3).

### 2.10 garust surface used

`Motor3` (= `garust_geo::Motor<f64>`): `identity`, `translator`, `inverse`,
`compose`/`*`; `slerp` only indirectly via `Track`. Typed
`garust::pga::Point`: `new`, `transform(&Motor3)`, `to_euclidean`
(weight-dividing, scale-independent). Not used, deliberately: `to_matrix`,
`apply_each`, `apply_each_simd`, `apply_point_fast` (M4 throughput, RFC
A6); no `simd` feature, no `wide` (per project-specifics, an M4 decision).
Zero new dependencies: `std` + garust in the library; `proptest` (the same
major garust dev-locks, `"1"`) as a **dev-dependency only** for §6.

## 3. Code outline

```
crates/motoreel/src/
├── lib.rs        # + mod/pub use lines below (SPEC-0001 adds ease, track)
├── ease.rs       # SPEC-0001
├── track.rs      # SPEC-0001
├── prim.rs       # 2D output vocabulary — depends on std only
├── object.rs     # Shape, Object — depends on prim, track, garust
├── camera.rs     # Camera, Projection — depends on prim, garust
└── scene.rs      # Scene, ObjectId, eval — depends on object, camera, prim
```

Dependencies point inward: `scene → {object, camera} → {prim, track} →
std/garust`. No cycles; `track` never learns of scenes (SPEC-0001 non-goal
honored); `prim` never learns of 3D.

```rust
// lib.rs — additions
mod camera;
mod object;
mod prim;
mod scene;
pub use camera::{Camera, Projection};
pub use object::{Object, Shape};
pub use prim::{Prim2, Pt2, Rgb, Style};
pub use scene::{ObjectId, Scene};
```

```rust
// prim.rs — the sink-facing boundary; no garust types

/// A point in image space: `x` right, `y` up, origin on the optical axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pt2 {
    /// Rightward image coordinate.
    pub x: f64,
    /// Upward image coordinate (sinks flip for SVG's y-down).
    pub y: f64,
}

/// An sRGB stroke color, 8 bits per channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
}

impl Rgb {
    /// Pure white — the default stroke for the assumed dark background.
    pub const WHITE: Rgb = Rgb { r: 255, g: 255, b: 255 };
    /// Pure black.
    pub const BLACK: Rgb = Rgb { r: 0, g: 0, b: 0 };
}

/// Stroke style — R-0002's minimal vocabulary: color, width, alpha.
/// Contracts: `width` finite and ≥ 0 (image units); `alpha` in [0, 1].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    /// Stroke color.
    pub stroke: Rgb,
    /// Stroke width, in image units.
    pub width: f64,
    /// Stroke opacity, 0 transparent to 1 opaque.
    pub alpha: f64,
}

/// White, width 0.01 (≈ 6 px at 1080p in SPEC-0003's default view), opaque.
impl Default for Style { /* Style { stroke: Rgb::WHITE, width: 0.01, alpha: 1.0 } */ }

/// A flat 2D drawing primitive — everything a sink needs, no trace of 3D.
/// Invariant upheld by `Scene::eval`: every geometry coordinate is finite
/// (styles are author data, carried verbatim — §2.2).
#[derive(Clone, Debug, PartialEq)]
pub enum Prim2 {
    /// A dot.
    Point { at: Pt2, style: Style },
    /// A straight stroke from `a` to `b`.
    Segment { a: Pt2, b: Pt2, style: Style },
    /// An open polyline through `points` in order (< 2 points draws
    /// nothing or a dot — the sink's call).
    Polyline { points: Vec<Pt2>, style: Style },
}
```

```rust
// object.rs — what a scene contains

use garust::{pga, Motor3};
use crate::prim::Style;
use crate::track::Track;

/// Drawable model-space geometry as typed PGA points. Derived incidence
/// shapes (`JoinLine`, `MeetPoint`) arrive with M3 (R-0007).
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// A single point.
    Point(pga::Point),
    /// A straight stroke between two points.
    Segment(pga::Point, pga::Point),
    /// An open polyline through the points in order; close a figure by
    /// repeating its first point.
    Polyline(Vec<pga::Point>),
}

/// A scene entry: geometry, stroke style, and the motor track posing it.
#[derive(Clone, Debug)]
pub struct Object {
    /// Model-space geometry, posed by `track` at evaluation time.
    pub shape: Shape,
    /// Stroke style, carried to the emitted primitive unchanged (AC4).
    pub style: Style,
    /// Pose over time (SPEC-0001); a single key means a static object.
    pub track: Track,
}

impl Object {
    /// A point object with default style, holding the identity pose.
    pub fn point(p: pga::Point) -> Self;
    /// A segment object with default style, holding the identity pose.
    pub fn segment(a: pga::Point, b: pga::Point) -> Self;
    /// A polyline object with default style, holding the identity pose.
    pub fn polyline(points: Vec<pga::Point>) -> Self;
    /// Replace the style (builder).
    pub fn with_style(self, style: Style) -> Self;
    /// Replace the track (builder).
    pub fn with_track(self, track: Track) -> Self;
    /// Hold a fixed pose: replaces the track with `Track::hold(pose)`.
    pub fn at(self, pose: Motor3) -> Self;
}

// Constructors and `at` take their single-key track from SPEC-0001's
// infallible `Track::hold(pose)` — no private helper, no `expect`.
```

```rust
// camera.rs — the eye and the only non-motor step

use garust::{pga, Motor3};
use crate::prim::Pt2;

/// How view-space geometry maps to the image plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Projection {
    /// Perspective through the camera center onto a plane `focal` ahead:
    /// image = `(focal·x/d, focal·y/d)`, `d = −z`. Contract: `focal`
    /// finite and > 0.
    Pinhole { focal: f64 },
    /// Parallel projection: image = view `(x, y)`; depth only culls.
    Orthographic,
}

impl Projection {
    /// Near-plane distance in view units: a vertex with depth below this
    /// (behind, at plane, or non-finite) culls its whole primitive.
    pub const NEAR: f64 = 1e-9;

    /// Project one view-space point to image space; `None` means culled.
    /// Never returns a non-finite coordinate (§2.5).
    pub fn project(&self, view_point: &pga::Point) -> Option<Pt2> {
        let (x, y, z) = view_point.to_euclidean();
        let d = -z; // the camera looks along −z (§2.1)
        if !d.is_finite() || d < Self::NEAR {
            return None; // behind / at plane / degenerate — cull, never NaN
        }
        let p = match *self {
            Projection::Pinhole { focal } => Pt2 { x: focal * x / d, y: focal * y / d },
            Projection::Orthographic => Pt2 { x, y },
        };
        (p.x.is_finite() && p.y.is_finite()).then_some(p)
    }
}

/// The eye: a motor pose plus a projection rule. The camera looks along
/// its local −z with +y up; the view transform is `pose.inverse()`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// World-from-camera rigid pose — a motor, like every other motion.
    pub pose: Motor3,
    /// Pinhole or orthographic image formation.
    pub projection: Projection,
}

impl Camera {
    /// A pinhole camera at `pose` with the given focal length.
    pub fn pinhole(pose: Motor3, focal: f64) -> Self;
    /// An orthographic camera at `pose` — the classic math-diagram look.
    /// Content must sit in front of the camera: the uniform cull rule
    /// (§2.5) applies to orthographic views too.
    pub fn orthographic(pose: Motor3) -> Self;
}

/// Orthographic, standing 5 units back on +z, looking at the origin.
impl Default for Camera { /* orthographic(Motor3::translator(0.0, 0.0, 5.0)) */ }
```

```rust
// scene.rs — composition and the evaluation loop

use garust::{pga, Motor3};
use crate::camera::{Camera, Projection};
use crate::object::{Object, Shape};
use crate::prim::{Prim2, Pt2, Style};

/// Opaque handle to an object in a scene (stable insertion index) — the
/// seam R-0007's derived shapes will reference; inert until then.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjectId(usize); // field private to the crate

/// A renderable description: objects in draw order, one camera, and a
/// duration in seconds (consumed by SPEC-0003's frame walk, not by eval).
#[derive(Clone, Debug)]
pub struct Scene {
    /// Draw order = insertion order; later objects paint over earlier.
    pub objects: Vec<Object>,
    /// The eye (§2.4).
    pub camera: Camera,
    /// Scene length in seconds. Contract: finite and ≥ 0.
    pub duration: f64,
}

impl Scene {
    /// An empty scene of the given duration with the default camera.
    pub fn new(duration: f64) -> Self;

    /// Append an object; returns its stable id (draw order = insertion).
    pub fn add(&mut self, object: Object) -> ObjectId;

    /// Evaluate at time `t`: every track becomes a pose, geometry rides
    /// one composed motor into view space, surviving primitives are
    /// emitted in draw order. Total and deterministic; culled primitives
    /// are simply absent (§2.5).
    pub fn eval(&self, t: f64) -> Vec<Prim2> {
        let view = self.camera.pose.inverse();
        let mut prims = Vec::with_capacity(self.objects.len());
        for obj in &self.objects {
            let to_view = view.compose(&obj.track.eval(t)); // pose, then view
            if let Some(p) =
                project_shape(&obj.shape, &to_view, self.camera.projection, obj.style)
            {
                prims.push(p);
            }
        }
        prims
    }
}

/// One shape through the camera; `None` culls the whole primitive (§2.5).
fn project_shape(
    shape: &Shape,
    to_view: &Motor3,
    projection: Projection,
    style: Style,
) -> Option<Prim2> {
    let pt = |p: &pga::Point| projection.project(&p.transform(to_view));
    Some(match shape {
        Shape::Point(p) => Prim2::Point { at: pt(p)?, style },
        Shape::Segment(a, b) => Prim2::Segment { a: pt(a)?, b: pt(b)?, style },
        Shape::Polyline(ps) => {
            let points = ps.iter().map(pt).collect::<Option<Vec<Pt2>>>()?;
            Prim2::Polyline { points, style }
        }
    })
}
```

## 4. Non-goals

- **Derived shapes** (`JoinLine`, `MeetPoint`) — M3, R-0007. `ObjectId` is
  their inert seam; nothing else anticipates them.
- **Camera tracks / animated focal.** The pose being a motor makes a later
  "camera track" requirement a natural extension; here the camera is fixed
  data per evaluation.
- **Fills, gradients, text, clipping regions, depth sorting.** Draw order
  is insertion order, full stop (R-0002 §4).
- **Near-plane clipping / partial primitives** — culling is whole-primitive
  by decision (§2.5); clipping would be an additive later spec.
- **Viewport/pixel/viewBox mapping, background, frame walking, sinks** —
  SPEC-0003.
- **Throughput** (`to_matrix` handoff, `apply_each[_simd]`, `f32`, the
  `wide` dependency question) — M4, RFC A6.
- **Validation of numeric style/camera/duration contracts** beyond the
  documented ranges and the structural never-NaN guarantee (§2.8).
- **Scene editing API** beyond `add` (no removal, reordering, lookup;
  fields are public).

## 5. Open questions

None — architect review 2026-08-20 adjudicated all four: Q1
`Projection::project` is `pub`, documented as taking a view-space point;
Q2 the uniform cull rule stays; Q3 defaults approved, with
`Style::default()` width 0.01 (F6); Q4 resolved by SPEC-0001's
`Track::hold` (F7). Each is recorded in the decision log as
architect-recommended, owner acceptance pending.

## 6. Acceptance criteria

Each maps to R-0002's AC and becomes a qa-owned test. Module-level unit
tests live in `#[cfg(test)]` blocks; requirement-level tests in
`crates/motoreel/tests/r0002_scene_camera.rs`. Property tests use
`proptest` (dev-dependency only — the same tool and major version garust
dev-locks; the library stays zero-dep).

- [ ] **AC1 — golden frame (unit/integration, exact).** The scene below is
  evaluated at `t = 4.0` and compared with `assert_eq!` against a
  hand-constructed `Vec<Prim2>`. Camera: `pinhole(translator(0,0,4), 2.0)`.
  Every input is a dyadic rational and every depth a power of two, so all
  f64 arithmetic (translator coefficients, sandwich, weight divide by 1,
  `focal·x/d`) is rounding-free and the comparison is bit-exact; the
  animated object is evaluated at its **last key time**, exact by R-0001
  AC1/AC4 (§2.9), independent of `slerp` rounding.

  | # | object (style) | track | world at t=4 | view z | d | image |
  |---|---|---|---|---|---|---|
  | 1 | Segment (1,0,0)–(0,1,0) (s1) | identity | unchanged | −4 | 4 | (0.5,0)–(0,0.5) |
  | 2 | Polyline unit square (s2) | at translator(1,0,0) | x+1 | −4 | 4 | (0.5,0),(1,0),(1,0.5),(0.5,0.5) |
  | 3 | Point (0,0,0) (s3) | keys (0,id),(4,translator(0,0,2)) | (0,0,2) | −2 | 2 | (0,0) |
  | 4 | Point (0,0,6) | identity | unchanged | 2 | −2 | **culled** (behind) |
  | 5 | Point (0,0,4) | identity | unchanged | 0 | 0 | **culled** (at plane) |

  Expected: `[Segment{(0.5,0),(0,0.5),s1}, Polyline{[(0.5,0),(1,0),(1,0.5),(0.5,0.5)],s2}, Point{(0,0),s3}]`
  with s1/s2/s3 distinct dyadic styles — the golden also witnesses AC4 and
  the cull policy. (Fallback if the architect prefers a recorded artifact:
  compare a checked-in `format!("{:#?}")` dump byte-for-byte, R-0003
  style.)
- [ ] **AC2 — camera/world duality (property).** Generator: motors as
  `translator(±0.8 box) ∘ rotor(θ ∈ [−TAU/4, TAU/4], basis plane)` (for
  both `M` and object poses); 1–4 objects with ≤ 8 vertices in `[−2,2]³`,
  single-key generated poses and styles; camera pose `translator(0,0,8) ∘
  small motor (translations ≤ 0.25)`; both projection variants. The boxes
  are sized so every generated depth stays O(1)-far from `NEAR` — wider
  translation boxes would let geometry approach the camera plane, where the
  pinhole divide amplifies rounding past any fixed tolerance (qa-run
  decision, 2026-08-20). Compare `eval` of (camera `M·C`, poses `Q`) against (camera
  `C`, poses `M⁻¹·Q`) at `t = 0`: equal length, pairwise same variant,
  bit-equal styles, coordinates within `1e-6` absolute (generator keeps
  image coordinates O(10) and depths O(1)-far from `NEAR`; a measure-zero
  near-threshold coincidence cannot be generated by continuous
  strategies). Static keys suffice by left-invariance (§2.7).
- [ ] **AC3 — projection models (unit, hand-computed).**
  `Pinhole{focal:2}.project(Point::new(1, 2, -4))` = `(0.5, 1.0)`;
  `Orthographic.project` of the same point = `(1.0, 2.0)`; both `None` for
  behind `(0,0,1)`, at-plane `(0,0,0)`, near-plane `(0, 0, -1e-12)` (d
  below `NEAR`), and an ideal point via `Point::from_multivector` (weight
  0 → non-finite → culled). Both models on the one `Camera` type via the
  two constructors.
- [ ] **AC4 — style passthrough (unit).** Objects with distinct styles
  (u8 colors, width 3.5, alpha 0.25) emit primitives whose `Style`
  compares equal bit-for-bit under both projections; also witnessed by the
  golden (AC1) and asserted pairwise in the AC2 harness.
- [ ] **AC5 — track evaluation at scene time (unit).** A single-key object
  at pose P renders identically (bit-compare) for `t ∈ {−1, 0, 0.5, 7}`
  (clamp semantics, R-0001 AC4). A two-key object renders exactly at its
  key times; at mid-span `t = 1` of keys `(0, id), (4, translator(0,0,2))`
  it matches the hand expectation (quarter translation) within `1e-12` —
  tolerance decouples this spec from slerp bit-behavior.
- [ ] **AC6 — determinism (unit + property rider).** The golden scene is
  evaluated twice, and once more on a `clone()`; all coordinate f64s
  compare equal by `to_bits` (stricter than `==`, which admits ±0
  crossings). The AC2 harness re-evaluates one side and asserts
  bit-identity on arbitrary generated scenes.
- [ ] **Invariant — never non-finite (property, supports R-0002 §4).**
  Hostile generator: coordinates to ±1e9, points at/behind the plane,
  ideal points, focal in `(1e-6, 1e3]`, both models — every geometry
  coordinate in every emitted primitive is finite (styles are drawn within
  contract: they are author data carried verbatim, §2.2).

## 7. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-08-20 | View frame: right-handed, x right, y up, camera looks along −z; depth d = −z | GL/math convention; a +z camera sees the xy-plane upright with no flips |
| 2026-08-20 | One culling rule, both models: whole primitive dropped iff any vertex has non-finite or < NEAR depth, or projects non-finite | Simplest total rule; no clip vertices, no reshaped polylines; makes never-NaN structural (R-0002 §4) |
| 2026-08-20 | Orthographic also culls behind/at-plane | A camera keeps a facing under parallel projection; one rule, one code path, duality uniform across models (§5 Q2 to confirm) |
| 2026-08-20 | `NEAR = 1e-9`, strictly positive associated const | d > 0 subnormal would overflow the pinhole divide to inf; 1e-9 is far below scene scale, far above f64 noise |
| 2026-08-20 | Per-object motor composed once (view ∘ pose), one sandwich per vertex | Fewer roundings, fixed expression order (AC6); RFC §3.4 with the two transforms fused |
| 2026-08-20 | No matrix anywhere — projection is component arithmetic | Stronger than the "matrices only inside projection" ceiling; `to_matrix` stays an M4 bulk option |
| 2026-08-20 | Golden AC1 is a hand-constructed dyadic-exact list, `assert_eq!` | Auditable arithmetic, not a recorded blob; dyadic inputs make f64 evaluation rounding-free; animated object pinned to a key time to avoid slerp bit-dependence |
| 2026-08-20 | Camera/Projection are plain data (pub fields); focal doc-contracted (finite, > 0), unvalidated | R-0002's decided shape; a bad focal is malformed input, not a reachable failure — and the finite-guard still keeps output NaN-free |
| 2026-08-20 | No new error types; `Scene::eval` total | Mirrors R-0001's "no error path in the render loop"; only Track construction is fallible |
| 2026-08-20 | `Prim2` is an enum (Point/Segment/Polyline), each variant carrying `Style` | Self-describing for the sink; no length-encoded ambiguity between a dot and a degenerate polyline |
| 2026-08-20 | Degenerate polylines (< 2 points) pass through unvalidated | Total pipeline; drawing nothing is the sink's honest rendering of empty geometry |
| 2026-08-20 | `ObjectId` returned by `Scene::add` now, opaque insertion index | RFC target API and the seam R-0007 needs; costs one newtype, no behavior |
| 2026-08-20 | Builders named `with_style`/`with_track`/`at` (RFC sketch had `.track()`) | Method-shadows-field reads badly with public fields; `with_*` is house-idiomatic |
| 2026-08-20 | Defaults: `Camera::default()` orthographic at translator(0,0,5); `Style::default()` white/2.0/1.0 | Zero-surprise math-diagram default (unit square → unit square) with content in front of the eye; dark-theme stroke (§5 Q3, owner taste) |
| 2026-08-20 | Architect F6: `Style::default()` width 2.0 → **0.01** image units | Unit coherence: width is in image units and 2.0 exceeds the whole default 3.2 × 1.8 view window; 0.01 ≈ 6 px at 1080p (SPEC-0003's own reference). Owner may adjust taste later |
| 2026-08-20 | §5 Q1 adjudicated: `Projection::project` is `pub`, documented as taking a view-space point | AC3's direct test point; users can project annotation anchors — architect-recommended, owner acceptance pending |
| 2026-08-20 | §5 Q2 adjudicated: the uniform cull rule stays (both projections cull behind-camera); the orthographic constructor doc states content must sit in front | One rule, one code path, uniform duality — architect-recommended, owner acceptance pending |
| 2026-08-20 | §5 Q3 adjudicated: defaults approved — `Camera::default()` orthographic at translator(0,0,5); `Style::default()` white, alpha 1.0, width 0.01 per F6 | Zero-surprise math-diagram default — architect-recommended, owner acceptance pending |
| 2026-08-20 | §5 Q4 / architect F7: single-key tracks come from SPEC-0001's public `Track::hold`; the private-helper-plus-`expect` plan is deleted | SPEC-0001 landed `hold` at this spec's request; no `expect` remains in this spec — architect-recommended, owner acceptance pending |

| 2026-08-20 | Never-NaN invariant scoped to geometry coordinates; styles pass through verbatim under their documented contracts | qa run: §2.2's original wording contradicted AC4 passthrough for out-of-contract styles; sanitizing would silently mutate author data (architect-recommended pattern, owner acceptance pending) |
| 2026-08-20 | AC2 generator boxes tightened (±0.8 pose/M translations, ≤0.25 camera nudge) | qa run: the original ±2 boxes contradicted the governing depths-far-from-NEAR guarantee (~9% flake per 256 cases); tightened boxes provably keep depths ≥ ~0.9 |

## Changelog

- 2026-08-20 — created (Draft); submitted for architect review.
