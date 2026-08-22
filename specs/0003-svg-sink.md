# SPEC-0003 — `FrameSink` + `SvgSink`: the render walk and first light

- **Status:** Draft — architect review 2026-08-20 (REQUEST CHANGES) applied; awaiting owner acceptance
- **Realizes:** R-0003
- **Author:** Claude (main session) with owner
- **Created:** 2026-08-20
- **Depends on:** SPEC-0001; SPEC-0002 (Draft — assumptions reconciled in §2.8)
- **Module(s):** `crates/motoreel/src/sink.rs`, `crates/motoreel/src/svg.rs`
  (plus `crates/motoreel/examples/first_light/` and `crates/motoreel/tests/`)

## 1. Motivation

R-0003: frames on disk are the product. This spec adds the `FrameSink`
abstraction and the `Scene::render` frame walk (AC1), an `SvgSink` writing
well-formed, zero-padded, **byte-deterministic** SVG frames (AC2), a
byte-for-byte golden-file test (AC3), the RFC-012 §3.5 screw demo as first
light — 240 frames, rendered twice, bit-identical (AC4) — and the documented
`ffmpeg` invocation, kept outside the crate and outside CI (AC5).

Everything here is `std` + garust; zero new dependencies (AC5, RFC-012 §2).

## 2. Design

### 2.1 Module layout and public surface

```
crates/motoreel/src/
  sink.rs                      — `FrameSink` trait + `impl Scene { render }`
  svg.rs                       — `SvgSink`
crates/motoreel/examples/first_light/
  main.rs                      — renders the demo to `out/`; documents ffmpeg
  scene.rs                     — the demo scene builder (shared with tests)
crates/motoreel/tests/
  render_walk.rs               — AC1
  svg_sink.rs                  — AC2
  golden.rs                    — AC3
  first_light.rs               — AC4 (reuses examples/first_light/scene.rs)
  golden/frame_00000.svg       — the checked-in golden fixture (small)
```

`lib.rs` gains `pub mod sink; pub mod svg;` and re-exports
`sink::FrameSink`, `svg::SvgSink`. Dependencies point inward: `svg` →
`sink` → `scene` (SPEC-0002) → `track` (SPEC-0001) → garust. `scene.rs` is
not touched — the render walk lives sink-side, so the scene stays
sink-agnostic.

### 2.2 `FrameSink` — RFC-012 §3.3, verbatim

```rust
/// Per-frame consumer of projected primitives (RFC-012 §3.3).
pub trait FrameSink {
    /// Consume frame `index`'s primitives; any write failure propagates.
    fn frame(&mut self, index: usize, prims: &[Prim2]) -> io::Result<()>;
}
```

No deviation from the RFC signature, and deliberately no `finish()`/flush
hook: every frame is an independent file, fully written and closed inside
`frame`. A sink that one day needs end-of-render finalization motivates a
spec revision, not a speculative method today.

### 2.3 `Scene::render` — the frame walk (AC1)

```rust
impl Scene {
    /// Walk `ceil(duration · fps)` frames at `t_i = i / fps`, feeding each
    /// evaluated frame to `sink`; stops at the first sink error.
    pub fn render<S: FrameSink + ?Sized>(&self, fps: f64, sink: &mut S) -> io::Result<()>;
}
```

- **Frame count:** `n = (duration * fps).ceil() as usize`. `4.0 s × 60 fps`
  is exactly `240.0` → 240 frames; a fractional product gets its partial
  second a frame (`1.01 s × 60` → 61).
- **Timing is derived, never accumulated** (R-0003 §4): `t_i = i as f64 / fps`.
  No running sum, so no float drift at frame 100 000. Every `t_i` lies in
  `[0, duration)`; the pose *at* `duration` is never sampled (the last frame
  displays for its full `1/fps`, standard frame semantics).
- **Per frame:** `sink.frame(i, &self.eval(t_i))?` — evaluation is entirely
  SPEC-0002's (`Scene::eval`, §2.8 item 1); the walk adds nothing else.
- **Validation:** non-finite or `≤ 0` fps, and a `duration` failing
  `duration.is_finite() && duration >= 0.0`, return
  `io::Error::new(io::ErrorKind::InvalidInput, …)` before any sink call.
  Duration is validated *here* because SPEC-0002 deliberately validates
  nothing (its §2.8: the field carries a documented contract only) —
  unchecked, a `+∞` duration would make `(duration * fps).ceil() as usize`
  saturate to `usize::MAX` (an unbounded render) and a NaN would silently
  truncate to 0 frames. `io::Error` is the pipe's error currency and
  `ErrorKind` keeps it typed (constitution §6) without deviating from the
  RFC §3.4 call shape.
- **Zero-duration scene:** 0 frames, `Ok(())`, sink never called
  (`duration = −0.0` passes the `>= 0.0` predicate identically — IEEE
  `−0.0 >= 0.0` is true; harmless and documented).
- **Errors:** the first sink `Err` propagates immediately; later frames are
  not attempted. The walk itself performs no I/O.

The bound is `S: FrameSink + ?Sized` so both `&mut SvgSink` and
`&mut dyn FrameSink` work; the RFC §3.4 call site compiles as written.

### 2.4 `SvgSink` — construction and directory policy

```rust
/// Writes one `frame_%05d.svg` per frame into a directory (RFC-012 §3.3).
pub struct SvgSink {
    dir: PathBuf,
    size: (u32, u32),   // raster hint: width/height attributes, px
    view: (f64, f64),   // image-space view window (w, h), centred on origin
    buf: String,        // reused per frame (capacity cache only)
}

impl SvgSink {
    /// Sink into `dir` (created now, parents included): 1920×1080 raster
    /// over the default 3.2 × 1.8 centred view window.
    pub fn new(dir: impl AsRef<Path>) -> io::Result<Self>;

    /// Sink into `dir` with explicit raster size (px) and centred
    /// image-space view window (width, height).
    pub fn with_view(dir: impl AsRef<Path>, size: (u32, u32), view: (f64, f64))
        -> io::Result<Self>;
}
```

- **Directory policy:** `fs::create_dir_all(dir)` at construction —
  idempotent, creates parents, and surfaces permission/path failures *before*
  a long render, matching the RFC's `SvgSink::new("out/")?`. `frame()` never
  re-creates the directory; if it vanishes mid-render the `fs::write` error
  propagates.
- **Validation:** zero raster dimensions or a non-finite/non-positive view
  → `ErrorKind::InvalidInput` (same policy as §2.3). Validation runs
  **before** `create_dir_all` — invalid input has no side effects, matching
  `render`'s sink-untouched rule (qa-run decision 2026-08-20).
- **Existing files are overwritten** (`fs::write` truncates); the sink never
  deletes. A shorter re-render over a stale directory leaves higher-numbered
  frames behind, which `ffmpeg %05d` would happily encode — the example
  documents "encode from a clean directory".
- **Filenames:** `format!("frame_{index:05}.svg")` — zero-padded to five
  digits, `frame_00000.svg` onward. `{:05}` is *minimum* width: index
  100 000+ widens naturally (> 27 min at 60 fps; the documented ffmpeg
  pattern would then need adjusting — noted, out of scope).
- **Write path:** the whole frame is composed in `buf` (cleared, capacity
  retained), then a single `fs::write`. One code path, one syscall-ish write;
  on failure at most one partial file remains and the error propagates — no
  cleanup attempted (documented).

### 2.5 The SVG byte grammar (AC2, AC3)

The writer is a fixed template — same input bits, same output bytes, on
every platform. Exact rules:

**Float formatting (pinned).** Every `f64` is written with Rust's default
`Display` (`write!(buf, "{}", x)`): the shortest decimal string that parses
back to exactly `x`. Deterministic given the bits, never scientific
notation, `1` not `1.0`, `0.5`, `-1.6`; `-0` is possible and legal SVG.
Never `{:.N}` precision (a lossy knob to bikeshed), never `{:?}`. Colors are
`#rrggbb` lowercase hex, always six digits
(`format!("#{:02x}{:02x}{:02x}", …)`). Raster `size` is `u32` — integers,
no float formatting in the header.

**File shape (exact bytes, in order).** `\n` after every line including the
last; no indentation; UTF-8 (ASCII in practice):

```
<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="{-vw/2} {-vh/2} {vw} {vh}">
<g transform="scale(1 -1)">
{one line per primitive, in slice order}
</g>
</svg>
```

- **viewBox maps image space** (AC2): the view window is centred on the
  image-space origin — `viewBox="-1.6 -0.9 3.2 1.8"` by default — and
  `width`/`height` give the raster size hint (1920×1080). The four viewBox
  numbers are f64 via the pinned rule (`vw/2` is exact: exponent halving).
- **y-flip:** image space is y-up (§2.8 item 5); SVG is y-down. The
  single static `<g transform="scale(1 -1)">` performs the flip, so emitted
  numbers *are* the image coordinates — one literal attribute instead of
  per-vertex negation arithmetic. Stroke widths are unaffected
  (`|det| = 1`).
- **Element order = slice order = scene object order** (§2.8 item 3).
  The sink never sorts, groups, or dedups.

**One stroke element per primitive** (AC2), attribute order pinned by these
literal templates:

```
Segment  → <line x1="{ax}" y1="{ay}" x2="{bx}" y2="{by}" stroke="{#hex}" stroke-width="{w}" stroke-opacity="{a}" stroke-linecap="round"/>
Polyline → <polyline points="{x0},{y0} {x1},{y1} …" fill="none" stroke="{#hex}" stroke-width="{w}" stroke-opacity="{a}" stroke-linecap="round" stroke-linejoin="round"/>
Point    → <circle cx="{x}" cy="{y}" r="{width/2}" fill="{#hex}" fill-opacity="{a}"/>
```

- Style attributes are **always emitted**, including `stroke-opacity="1"` —
  a uniform template with no value-dependent branching keeps the byte shape
  independent of the data.
- A point renders as a **filled** circle of radius `width / 2`: a *stroked*
  circle would draw a ring; the fill carries the stroke color and alpha, so
  stroke/width/alpha are all honoured as a dot. (Zero-length round-cap lines
  render inconsistently across rasterizers — rejected.)
- `points` pairs are `x,y` (comma inside a pair), single spaces between
  pairs, no trailing space.
- `stroke-width` and point radius are in **image units** (viewBox units):
  width `0.01` in the default 1.8-tall window ≈ 6 px at 1080p — documented
  on `Style` usage in the sink docs.
- **No escaping machinery:** every emitted value is a number or `#hex` —
  the metacharacters `< & " '` cannot occur. No text content exists (RFC
  non-goal).

**Determinism hygiene:** the write path reads no clock, no environment, no
randomness; iterates no `HashMap`; emits no comments, generator tags, or
timestamps. Buffer reuse affects capacity only, never bytes.

### 2.6 Determinism analysis — what "byte-for-byte" rests on

1. Upstream bits: R-0001 AC7 and R-0002 AC6 guarantee identical `Prim2`
   bits for identical scene + `t`; IEEE-754 f64 arithmetic is deterministic.
2. This spec's text layer (§2.5) is a pure function of those bits.
3. **Platform caveat (stated, not hidden):** `Motor::rotor`/`slerp` reach
   `sin`/`cos` in the platform libm, which may differ in the last ulp
   *across* platforms. Therefore:
   - **AC4** claims bit-identity across two renders in the *same*
     environment — trig is fine there, and the test renders twice in one
     process.
   - **AC3**'s golden scene is **transcendental-free by construction**:
     single-key tracks (clamp returns the key motor exactly — R-0001
     AC1/AC4, no `slerp`), poses built from `Motor::identity()` and
     `Motor::translator` only (no `rotor`), orthographic camera (no
     divide). Its bytes are platform-portable; CI on any host agrees.
   Cross-platform bit-identity of trig-bearing scenes is explicitly *not*
   claimed by R-0003 and not promised here.

### 2.7 First light — the §3.5 screw demo (AC4, AC5)

`examples/first_light/scene.rs` builds the scene; `main.rs` renders it:
4 s at 60 fps → exactly 240 frames in `out/`. Join-line omitted (M3,
R-0007) per R-0003 AC4.

**Full-turn caution (normative).** The RFC §3.5 snippet authors the screw
as *two* keys, identity → `translator(0,0,2) · rotor(TAU, e₁₂)`. A full-TAU
rotor is antipodal (versor scalar part `cos(TAU/2) = −1`); garust's
`Motor::slerp` folds the versor sign and takes the short way, so a single
two-key slerp through it is directionally ill-defined — the turn would
collapse rather than sweep (SPEC-0001 §2 "Span semantics" documents the
audited half-turn span cap). The demo therefore authors the screw as
**subdivided keys, a quarter turn per span**, each key placed exactly on the
intended one-parameter screw (z-rotation and z-translation commute, so the
subdivided path *is* the RFC's screw, identically):

```
key k ∈ 0..=4, s = k/4:  ( 4·s ,  translator(0, 0, 2·s) · rotor(TAU·s, e₁₂) )
```

Each span is a TAU/4 slerp — well inside the unambiguous range. The orbiter
uses SPEC-0001's `Track::spin(TAU/4, e₁₂, 4.0)` verbatim from the RFC,
which already emits quarter-turn-subdivided keys for exactly this reason.
**No new interpolation machinery is introduced by this spec.**

**Easing.** The RFC snippet's whole-track `SmootherStep` is dropped: R-0001
easing is per span, so applied across subdivided keys it would ease *each
quarter* (a pulsing screw), and re-easing the whole turn is time-warping —
out of R-0001 scope. Spans stay `Linear`: a constant-speed screw, which is
the project's story anyway. (Adjudicated 2026-08-20: Linear stays. If the
RFC's eased look is ever wanted, it is a future small *whole-track
time-warp* requirement — warp scene time before span lookup — never sink
machinery; the decision log records that promotion path.)

**Composition (structure normative, numbers demo taste — SPEC-0002's camera
landed, §2.8 items 6–8 confirmed):** unit square (side 1, closed 5-vertex
polyline, repeated last vertex) centred at origin in z = 0 riding the screw;
orbit point at `(2, 0, 0)`; pinhole camera posed by `translator(0, 0, 6)`
looking down −z — the square rises toward the camera and grows, reading
clearly as a screw. Light strokes (`#e0e0e0`, `#ff4d00`) on the transparent
(≈ dark, once encoded) background.

**AC4 test sharing:** `tests/first_light.rs` includes the *same* scene
source via `#[path = "../examples/first_light/scene.rs"] mod scene;` — the
multi-file example layout exists precisely so the shipped demo and the
determinism test cannot drift apart. It renders twice into
`env!("CARGO_TARGET_TMPDIR")` subdirectories `a/` and `b/`, asserts exactly
240 files each, and byte-compares every pair. (Adjudicated 2026-08-20:
`#[path]` approved, with the constraint that the shared `scene.rs` names
motoreel's own items through `motoreel::` and garust's through `garust::` —
never `crate::`/`super::`, which are the spellings that differ between the
example crate and the test crate. Cargo exposes the package's
`[dependencies]` to both, so `garust::` resolves identically in each.)

**Encoding (AC5) — outside the crate, outside CI.** Recorded in
`main.rs`'s doc header and here:

```bash
cargo run --example first_light
ffmpeg -framerate 60 -i out/frame_%05d.svg -pix_fmt yuv420p first_light.mp4
```

Note for the demo docs: SVG demuxing needs an ffmpeg built with librsvg;
otherwise pre-rasterize (`rsvg-convert`/Inkscape) and encode the PNGs. CI
never invokes ffmpeg; `cargo test` compiles (does not run) the example, and
`clippy --all-targets` lints it — the demo cannot rot silently. `out/` is
added to `.gitignore` at implementation.

### 2.8 SPEC-0002 touchpoints — reconciled and confirmed

SPEC-0002 has landed (Draft, reviewed the same day). The assumptions this
spec was drafted against, reconciled against its actuals:

1. **`Scene::eval(&self, t: f64) -> Vec<Prim2>`** — confirmed verbatim
   (SPEC-0002 §2.6).
2. **`duration`** — a public field (`Scene { duration: f64 }`) carrying a
   documented contract — finite, `≥ 0` — that SPEC-0002 *deliberately does
   not validate* (its §2.8: plain data, no error type); `render` therefore
   validates it alongside fps (§2.3) before walking.
3. **Order and determinism:** confirmed — `eval` output order is scene
   object insertion order, bit-deterministic (SPEC-0002 §2.6–§2.7, R-0002
   AC6); the sink writes slice order verbatim.
4. **`Prim2` vocabulary** — confirmed: `Point { at }` / `Segment { a, b }`
   / `Polyline { points }`, coordinates as `Pt2 { x, y }`, every variant
   carrying `style: Style` (SPEC-0002 §2.2); the §3 outline codes against
   the landed shape below.
5. **Image space is y-up, origin on the optical axis** — confirmed
   (SPEC-0002 §2.1: x right, y up); the `<g transform="scale(1 -1)">` flip
   stands as designed.
6. **`Style { stroke: Rgb { r, g, b }, width, alpha }`** with `u8` RGB —
   confirmed exactly as the sink wants it (each channel is two hex digits;
   no rounding rule needed; SPEC-0002 §2.2).
7. **Culling contract:** confirmed — SPEC-0002 §2.5 makes never-non-finite
   structural ("no non-finite number can leave `Scene::eval`"). The sink
   still `debug_assert!`s finiteness (test-time tripwire on the contract;
   justified per constitution §6 as a documented sibling-spec invariant)
   and release builds write whatever arrives, deterministically.
8. **Constructors used by the demo/golden** — confirmed (SPEC-0002
   §2.3–§2.4, §2.6): `Scene::new(duration)` (installs `Camera::default()`),
   `Scene::add`, `Object::point/segment/polyline` with `with_style` /
   `with_track` / `at(pose)`, `Camera::pinhole(pose, focal)` and
   `Camera::orthographic(pose)`.

SPEC-0001 touchpoints are factual (spec exists): `Track::keys` (Linear
spans), `Track::spin` (subdivided ≤ quarter turn), `TrackError`; the demo
uses `.expect(…)` on statically valid keys (example code, not library code).

### 2.9 Error handling summary

| Site | Failure | Behaviour |
|------|---------|-----------|
| `SvgSink::new` / `with_view` | dir creation fails | `io::Error` propagates (fail before rendering) |
| `SvgSink::new` / `with_view` | zero size / bad view | `ErrorKind::InvalidInput` |
| `SvgSink::frame` | `fs::write` fails | `io::Error` propagates; ≤ 1 partial file, no cleanup |
| `Scene::render` | bad fps / bad duration | `ErrorKind::InvalidInput`, sink untouched |
| `Scene::render` | sink error at frame *k* | that error returned; frames *k+1…* not attempted |

No panics on any library path; the only `debug_assert` is §2.8 item 7.

## 3. Code outline

```rust
// sink.rs — the trait and the walk (representative, not final)
use std::io;
use crate::prim::Prim2;
use crate::scene::Scene;

pub trait FrameSink {
    /// Consume frame `index`'s primitives; any write failure propagates.
    fn frame(&mut self, index: usize, prims: &[Prim2]) -> io::Result<()>;
}

impl Scene {
    /// Walk `ceil(duration · fps)` frames at `t_i = i / fps` into `sink`.
    pub fn render<S: FrameSink + ?Sized>(&self, fps: f64, sink: &mut S) -> io::Result<()> {
        if !(fps.is_finite() && fps > 0.0) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "fps must be finite and > 0"));
        }
        if !(self.duration.is_finite() && self.duration >= 0.0) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "duration must be finite and >= 0"));
        }
        let frames = (self.duration * fps).ceil() as usize;
        for index in 0..frames {
            let t = index as f64 / fps; // derived, never accumulated (R-0003 §4)
            sink.frame(index, &self.eval(t))?;
        }
        Ok(())
    }
}
```

```rust
// svg.rs — the byte-pinned writer (representative, not final)
impl FrameSink for SvgSink {
    fn frame(&mut self, index: usize, prims: &[Prim2]) -> io::Result<()> {
        self.buf.clear();
        self.header();                       // <?xml…>, <svg…>, <g …> per §2.5
        for p in prims {
            self.prim(p);                    // one pinned template line each
        }
        self.buf.push_str("</g>\n</svg>\n");
        std::fs::write(self.dir.join(format!("frame_{index:05}.svg")), &self.buf)
    }
}

// SPEC-0002's landed shape the outline codes against (§2.8, items 4/6):
// pub struct Pt2 { pub x: f64, pub y: f64 }
// pub enum Prim2 {
//     Point    { at: Pt2, style: Style },
//     Segment  { a: Pt2, b: Pt2, style: Style },
//     Polyline { points: Vec<Pt2>, style: Style },
// }
// pub struct Rgb { pub r: u8, pub g: u8, pub b: u8 }
// pub struct Style { pub stroke: Rgb, pub width: f64, pub alpha: f64 }
```

```rust
// examples/first_light/scene.rs — shared by tests/first_light.rs via
// #[path]; motoreel items via `motoreel::`, garust's via `garust::`,
// never crate::/super:: (decision log).
use std::f64::consts::TAU;

/// The RFC-012 §3.5 screw demo, join-line omitted (M3): a unit square rides
/// a full screw (one turn about z while rising 2), a point orbits it.
pub fn scene() -> Scene {
    // Full-turn caution (§2.7): rotor(TAU) is antipodal — author the screw
    // as quarter-turn keys on the exact one-parameter screw, Linear spans.
    let screw = Track::keys((0..=4).map(|k| {
        let s = k as f64 / 4.0; // derived, not accumulated
        (4.0 * s, Motor::translator(0.0, 0.0, 2.0 * s)
                * Motor::rotor(TAU * s, Pga3::basis(0b0011)))
    }))
    .expect("static demo keys are strictly increasing and non-empty");

    let orbit = Track::spin(TAU / 4.0, Pga3::basis(0b0011), 4.0)
        .expect("static demo spin parameters are valid");

    // Scene/Object/Camera construction per SPEC-0002 (§2.8, item 8); demo
    // numbers: pinhole camera posed at translator(0, 0, 6), unit square
    // as closed 5-vertex polyline at z = 0, orbiter point at (2, 0, 0).
    …
}
```

Golden fixture, illustrative (exact bytes are fixed at implementation from
the pinned grammar, reviewed by eye, then frozen — §6 AC3): a static
trig-free scene — one segment, one point, one triangle polyline, every
vertex authored in the **z = 0 plane**, default view window — viewed by
`Camera::default()` (the orthographic camera at `translator(0, 0, 5)` that
`Scene::new` installs: translator-only, dyadic, still transcendental-free;
every vertex sits at view depth 5). The stand-off matters: SPEC-0002 culls
behind-or-at-plane vertices under *both* projections, so an identity-posed
orthographic camera over z = 0 content would cull every primitive and
freeze an **empty** golden. It costs nothing in the bytes: orthographic
projection ignores depth, so the image coordinates below are the authored
x/y unchanged. Rendered by `scene.render(1.0, sink)` (duration 1.0 →
exactly one frame, `t = 0`):

```
<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="1920" height="1080" viewBox="-1.6 -0.9 3.2 1.8">
<g transform="scale(1 -1)">
<line x1="-1" y1="0" x2="1" y2="0" stroke="#e0e0e0" stroke-width="0.02" stroke-opacity="1" stroke-linecap="round"/>
<circle cx="0.5" cy="0.5" r="0.03" fill="#ff4d00" fill-opacity="1"/>
<polyline points="-0.5,-0.5 0.5,-0.5 0,0.25 -0.5,-0.5" fill="none" stroke="#00b4d8" stroke-width="0.02" stroke-opacity="0.5" stroke-linecap="round" stroke-linejoin="round"/>
</g>
</svg>
```

## 4. Non-goals

- No rasterization (`PpmSink` is R-0008/M4), no in-crate encoding, no
  real-time preview (RFC-012 §2).
- No derived shapes (`JoinLine`/`MeetPoint` — R-0007/M3); the demo omits the
  RFC's join-line by requirement.
- No SVG animation (SMIL/CSS), text, fills beyond the point-dot, gradients,
  clipping, or depth sorting; no XML parser (not even in tests).
- No background rect in M1 (adjudicated — decision log: transparent
  composites to black in typical encodes; revisit with M3 visuals), no
  frame-directory cleanup, no parallel frame writing (determinism first;
  throughput is R-0009).
- No new interpolation machinery — the demo's full turn is handled entirely
  by key subdivision (SPEC-0001).

## 5. Open questions

None — architect review 2026-08-20 adjudicated everything held open here:
the 1920×1080 / 3.2 × 1.8 defaults, no M1 background, `#[path]` sharing
(with the `motoreel::`-paths constraint), round caps/joins frozen into the
golden, and §2.8 reconciled against the landed SPEC-0002 — plus the §2.7
easing note and the error-type promotion trigger. Each is recorded in the
decision log as architect-recommended, owner acceptance pending.

## 6. Acceptance criteria

Each maps to an R-0003 AC; the qa agent derives the binding tests from
these (tests first, red, then implementation):

- [ ] **AC1** (`tests/render_walk.rs`): counting sink records `(index, len)`
  — frame count is `ceil(duration·fps)` for integral and fractional cases
  (4.0×60 → 240; 1.01×60 → 61; 0-duration → 0 calls); indices are `0..n` in
  order, exactly one call each. Timing law: a unit-rate translator track on
  a scene of duration **4.0** — pinned to a power of two so the span
  parameter `t / 4.0` stays exact (the dyadic discipline) — makes the
  emitted x-coordinate equal `i as f64 / fps` bit-exactly for `i ≥ 1` and
  value-equal at `i = 0` (real garust emits `−0.0` there — the zero-sign
  pin lives in the golden bytes, qa-run decision 2026-08-20);
  garust's translator `log`/`exp` path is exact for it (architect-verified:
  the parabolic `log` branch scales by `1/c` with `c = 1`, the null `exp`
  is `1 + self`). A sink failing at frame *k* stops the walk with that
  error after exactly *k+1* calls. `fps ∈ {0, −1, NaN, ∞}` and a scene
  `duration ∈ {−1.0, NaN, ∞}` → `ErrorKind::InvalidInput`, sink never
  called.
- [ ] **AC2** (`tests/svg_sink.rs`, driving the trait directly): filenames
  `frame_00000.svg` / `frame_00123.svg` for indices 0 / 123; nested output
  dir is created by `new`; header + viewBox exactly per §2.5 for a custom
  `with_view`; each primitive kind emits its pinned template (string
  assertions — no XML parser dep); style attrs always present incl.
  `stroke-opacity="1"`; empty `prims` yields the well-formed empty document.
- [ ] **AC3** (`tests/golden.rs`): the §3 trig-free scene rendered into
  `env!("CARGO_TARGET_TMPDIR")` equals
  `include_bytes!("golden/frame_00000.svg")` byte-for-byte. Fixture is
  regenerated only via `MOTOREEL_BLESS=1 cargo test --test golden`, and its
  diff is reviewed like code.
- [ ] **AC4** (`tests/first_light.rs`): the shared demo scene renders twice
  (same process, `CARGO_TARGET_TMPDIR/{a,b}`) at 60 fps / 4 s → exactly 240
  files each side, every corresponding pair bit-identical.
- [ ] **AC5**: `Cargo.toml` diff adds no dependencies (architect/PR gate);
  the ffmpeg invocation is recorded in `examples/first_light/main.rs` and
  §2.7; nothing in tests or CI shells out to ffmpeg.

## 7. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-08-20 | Float text = Rust `f64` `Display` (shortest round-trip), pinned | Deterministic given the bits, minimal bytes, no precision knob to drift; `{:.N}` and `{:?}` forbidden in the writer |
| 2026-08-20 | Golden scene is transcendental-free (identity/translator poses, single-key tracks, orthographic camera) | Platform libm `sin`/`cos` varies in the last ulp; AC3's bytes must be portable, AC4 is same-environment |
| 2026-08-20 | Fixed element templates, style attrs always emitted, no indentation, `\n` everywhere | One writer path; byte shape independent of values; diffable frames |
| 2026-08-20 | y-flip as single static `<g transform="scale(1 -1)">`; viewBox centred on origin | Emitted numbers are the image coordinates — mapping visible in one literal, no per-vertex arithmetic |
| 2026-08-20 | Point = filled circle, `r = width/2` | A stroked circle is a ring; fill carries stroke colour/alpha as a dot; zero-length capped lines render inconsistently |
| 2026-08-20 | Demo screw authored as quarter-turn keys, Linear spans; orbiter via SPEC-0001 `spin` | Antipodal full-TAU two-key slerp is ill-defined (garust folds versor sign); per-span SmootherStep over subdivided keys would pulse; no new interpolation machinery |
| 2026-08-20 | Demo in `examples/first_light/{main,scene}.rs`; AC4 test `#[path]`-includes `scene.rs` | Shipped demo and determinism test share one source; examples stay compile-checked by `cargo test` / clippy without rendering in CI |
| 2026-08-20 | Bad fps/view → `io::Error` of `ErrorKind::InvalidInput` | Keeps the RFC signatures; typed via `ErrorKind` (constitution §6) |
| 2026-08-20 | `create_dir_all` at construction only; whole-frame `fs::write`; overwrite, never delete | Fail before long renders; single write path; deterministic re-renders over existing dirs, stale-frame caveat documented |
| 2026-08-20 | Test infra: `env!("CARGO_TARGET_TMPDIR")` + `include_bytes!` + `MOTOREEL_BLESS=1` regen | Golden and demo tests with zero added dependencies |
| 2026-08-20 | Architect F8–F11 applied: golden rebuilt on `Camera::default()` (identity-ortho would cull z = 0 content — empty golden; ortho ignores depth, so the fixture's image coordinates are unchanged); `render` validates `duration` alongside fps (`+∞` saturates the frame count to `usize::MAX`, NaN truncates to 0); outline vocabulary aligned to SPEC-0002's landed `Pt2`/`Rgb`/`Prim2`/`Style`; AC1 timing-law duration pinned to dyadic 4.0 | Architect review (REQUEST CHANGES → resolved); owner acceptance pending |
| 2026-08-20 | Defaults approved: 1920×1080 raster over the 3.2 × 1.8 centred view window | Any other pair is one `with_view` call away — architect-recommended, owner acceptance pending |
| 2026-08-20 | No background element in M1; transparent composites to black in typical encodes | RFC §6 Q3 minimalism; a solid `<rect>` first child is an additive later change — revisit with M3 visuals; architect-recommended, owner acceptance pending |
| 2026-08-20 | Demo/test sharing via `#[path]` approved, constrained: the shared `scene.rs` uses only `motoreel::` paths | Single source of truth for demo and determinism test; `motoreel::`-rooted names are the one spelling valid from both including crates — architect-recommended, owner acceptance pending |
| 2026-08-20 | Round `stroke-linecap`/`stroke-linejoin` stay baked into the golden bytes | Aesthetic default; a later veto costs exactly one reviewed `MOTOREEL_BLESS` regeneration — architect-recommended, owner acceptance pending |
| 2026-08-20 | Demo easing stays Linear; the RFC's eased look is a future whole-track time-warp *requirement* (warp scene time before span lookup), never sink machinery | Per-span ease over subdivided keys would pulse; recording the promotion path keeps timing knobs out of the sinks — architect-recommended, owner acceptance pending |
| 2026-08-20 | `io::ErrorKind::InvalidInput` kept for fps *and* duration validation; promotion trigger recorded | The day a caller must programmatically distinguish validation from OS I/O errors, introduce a `RenderError` enum wrapping `io::Error`; until then the RFC signature stands — architect-recommended, owner acceptance pending |

| 2026-08-20 | Golden fixture freezes the real bytes incl. `-0,0.25` (garust zero-sign asymmetry: x = 0 → −0.0 under the z-translator camera); no writer zero-normalization | qa run: §2.5 blesses `-0` and forbids value-dependent branching; normalizing signs would reintroduce it (owner may re-bless to `+0` at the cost of one reviewed regen) |
| 2026-08-20 | Tests consolidated into one `tests/r0003_svg_sink.rs` (house convention, matching r0001/r0002) + `tests/golden/frame_00000.svg`; bless command `MOTOREEL_BLESS=1 cargo test -p motoreel --test r0003_svg_sink` | qa run: §2.1's four-file listing was illustrative; one file per requirement is the established repo shape |
| 2026-08-20 | AC1 timing law scoped: bit-exact `i ≥ 1`, value-equal `i = 0`; `with_view` validates before dir creation; `duration = −0.0` documented as 0-frame Ok | qa-run findings 1/3/5 decided (architect-recommended pattern, owner acceptance pending) |

| 2026-08-21 | `mem::take` header dance removed in favour of a direct disjoint-field borrow | qa sign-off finding 3: unclear per §2 and a latent trap — the header field sits empty across the copy, so any future fallible insertion there would blank subsequent frames |
| 2026-08-21 | Shared-`scene.rs` path constraint reworded: motoreel items via `motoreel::`, garust's via `garust::`, never `crate::`/`super::` | qa sign-off finding A: the original "motoreel:: only" wording was literally unsatisfiable — motoreel re-exports no garust types |
| 2026-08-21 | Demo helper renamed `motoreel_point` → `flat_point` | qa sign-off finding B: the old name pointed at the wrong crate (it returns a `garust::pga::Point`); §2 wants names to carry intent |

## Changelog

- 2026-08-20 — created.
