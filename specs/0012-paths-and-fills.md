# SPEC-0012 — Paths and fills

- **Status:** Draft (architect review pending)
- **Realizes:** R-0012
- **Author:** Claude (main session) with owner
- **Created:** 2026-09-27
- **Depends on:** SPEC-0002 (`Shape`, `Prim2`, projection, cull),
  SPEC-0003 (`SvgSink`, number formatting, golden scheme), SPEC-0006
  (`PpmSink`, coverage ramp, determinism argument §2.7)
- **Module(s):**
  - new `crates/motoreel/src/path.rs`: path types, flattening, subdivision
  - new `crates/motoreel/src/shapes.rs`: shape generators
  - `crates/motoreel/src/prim.rs`: `Fill`, `Style::fill`, `Prim2::Path`
  - `crates/motoreel/src/object.rs`: `Shape::Path`
  - `crates/motoreel/src/scene.rs`: path projection
  - `crates/motoreel/src/ppm.rs`: fill rasterizer and path strokes
  - `crates/motoreel/src/svg.rs`: `<path>` emission
  - `lib.rs`: exports
  - new `examples/card/`
  - new `tests/r0012_paths_fills.rs`
  - new goldens under `tests/golden/`

## 1. Motivation

R-0012 (Accepted 2026-09-27) asks the engine to draw curved outlines and
filled regions. This spec builds it on the machinery SPEC-0006 already
proved:

- **Strokes of curves.** A path is flattened to chords within 0.1 px and
  handed to the existing round-capped-segment stroke rasterizer, unchanged.
- **Fills.** A fill uses the same one-pixel ramp, applied to the signed
  distance to the outline. The sign comes from the nonzero winding number.
  Coverage stays a closed-form function of each pixel, so the determinism
  argument of SPEC-0006 §2.7 survives **without amendment**.
- **Shapes.** Circles and rounded rectangles are built from quarter-circle
  cubics with the constant `k = 4(√2 − 1)/3`. They contain no
  transcendental function, so the golden fixtures stay bit-portable.

The four owner decisions on R-0012 (OQ-1 to OQ-4) fix the shape of the
design. §2 realizes them; it does not revisit them.

## 2. Design

### 2.1 Path types (`path.rs`, std-only)

`path.rs` depends on `std` alone, like `prim.rs`. It is generic over the
point type, so one definition serves model space (`pga::Point`) and image
space (`Pt2`).

```rust
/// One piece of a subpath, ending at its last point.
pub enum Seg<P> {
    Line(P),              // straight to P
    Cubic(P, P, P),       // handles h1, h2, then the end point
}

/// A connected outline: a start point and the segments that follow it.
pub struct Subpath<P> {
    pub start: P,
    pub segs: Vec<Seg<P>>,
    /// Closed subpaths stroke their closing segment. For **fill**, every
    /// subpath is treated as closed, which is SVG's behaviour (R-0012 AC3).
    pub closed: bool,
}
```

- **A path is `Vec<Subpath<P>>`.** There is no wrapper type; a wrapper with
  no invariant would be ceremony.
- **Builder:** `Subpath::new(start)`, `.line_to(p)`, `.cubic_to(h1, h2, p)`,
  `.close()`, each returning `Self` by value.
- **`Subpath::reversed()`** reverses orientation, which is how an author
  makes a hole under the nonzero rule.
- **`Subpath::map(f)`** is the one way a subpath changes point type. Eval
  uses it to project control points (§2.4).

### 2.2 Fill paint (`prim.rs`)

```rust
/// A flat fill: colour and opacity. Gradients are a later requirement.
pub struct Fill { pub colour: Rgb, pub alpha: f64 }

pub struct Style {
    pub stroke: Rgb,
    pub width: f64,
    pub alpha: f64,
    /// Interior paint (R-0012, OQ-3). `None` is today's behaviour.
    pub fill: Option<Fill>,
}
```

- `Style::default()` keeps the adjudicated stroke defaults and adds
  `fill: None`.
- `Style` stays `Copy`.
- **Which variants honour `fill`:** only `Prim2::Path` in this spec.
  - `Point`, `Segment` and `Edges` have zero area.
  - `Text` paints glyph coverage in `stroke`.
  - `Polyline` keeps SVG's `fill="none"`.
  - Each of these documents that it ignores `fill`, and a unit test pins
    that a `Some` fill leaves their bytes unchanged.
- The R-0012 decision-log rationale for OQ-3 ("any primitive can be
  filled") is sharpened to match (§7). Filling a polyline later is an
  additive change.
- **Contract, as `Style` documents today:** `fill.alpha ∈ [0, 1]` is
  documented, not validated. It is read through the total `unit` helper,
  so a NaN alpha paints nothing.

### 2.3 `Shape::Path` and `Prim2::Path`

```rust
// object.rs
pub enum Shape { …, Path(Vec<Subpath<pga::Point>>) }
impl Object {
    pub fn path(subpaths: Vec<Subpath<pga::Point>>) -> Self;
    /// Planar convenience: (x, y) in the model's z = 0 plane.
    pub fn planar(subpaths: Vec<Subpath<Pt2>>) -> Self;
}

// prim.rs
pub enum Prim2 { …, Path { subpaths: Vec<Subpath<Pt2>>, style: Style } }
```

- `Object::planar` lifts each control point `(x, y)` to
  `pga::Point::new(x, y, 0.0)`.
- `Pt2` is documented as "a point in a plane", used for image space and for
  the planar model coordinates the generators emit. That widens its doc
  comment but not its type.
- **Invariants that `Scene::eval` upholds for `Prim2::Path`:**
  - every coordinate is finite;
  - there is at least one subpath;
  - no subpath is empty of segments. Empty subpaths are dropped at eval,
    and a path with nothing left is not emitted.

### 2.4 Projection (`scene.rs`), realizing OQ-1

`project_shape` gains one arm. Control points are first carried into view
space by `to_view` (a motor, so rigid and affine). Then:

1. **Cull (R-0012 AC4).** If any control point fails the existing `pt`
   projection (behind the camera, at the camera plane, or non-finite), the
   whole primitive is culled.

   This is *sufficient*, not only necessary. A cubic lies in the convex hull
   of its four control points. If every control point has depth `d ≥ NEAR`,
   every point of the curve does too, because depth is an affine function
   and so its minimum over a convex hull is attained at a vertex. The cull
   test therefore never passes a curve that dips behind the camera.

2. **Orthographic.** Every control point is projected with `pt`. The image
   of a cubic under an affine map is the cubic of the mapped control points,
   **exactly**. No subdivision is needed and the SVG keeps one `C` per
   authored cubic.

3. **Pinhole.** Each view-space cubic is subdivided adaptively, then each
   piece's control points are projected:
   - For a piece `Q` with image-space cubic `q = project(ctrl(Q))`, the
     error estimate is
     `e(Q) = max over t ∈ {¼, ½, ¾} of ‖ project(Q(t)) − q(t) ‖`.
   - If `e(Q) ≤ τ` the piece is emitted as `Seg::Cubic`. Otherwise it is
     split at `t = ½` by de Casteljau (exact up to rounding: averages
     only) and both halves recurse.
   - **Tolerance:** `τ = PINHOLE_REL_TOL × min(view_w, view_h)` with
     `PINHOLE_REL_TOL = 5e-5`, using the scene's `view` (image units).
     Mapped to pixels by the sink's `scale = min(W/vw, H/vh)`, this is
     `≤ 0.05 × (short side in px)/1000` px, which is 0.054 px at 1080 ×
     1920 and under 0.1 px for any frame whose short side is ≤ 2000 px.
     The bound scales with resolution, and that scaling is documented.
   - **Why same-parameter comparison is conservative.** The distance
     between the points at equal parameter bounds the distance from
     `project(Q(t))` to the curve `q` from above. The estimate therefore
     never under-reports deviation *at the probes*. Between probes it is a
     heuristic, which is why AC4's test samples densely rather than
     trusting the probes.
   - **Termination:** a hard depth limit `PINHOLE_MAX_DEPTH = 12`, i.e.
     at most 4096 pieces per authored cubic. A piece still over tolerance
     at the limit is emitted anyway. This is reachable only for curves
     within about 1/4096 of the near plane, and it is documented.
   - Only `+ − × ÷` enter subdivision and projection. Pinhole projection
     already divides by depth, so determinism is unchanged.
   - `Seg::Line` pieces project their end points. A projective map sends
     lines to lines, so they are exact.

### 2.5 Flattening for the raster sink (`path.rs`)

The PPM sink maps control points to pixel space with `to_pixel`, which is
affine, so this is exact. It then flattens each cubic into `n` chords with
`n` chosen from a bound, never from a search.

**Bound.** Let `L = max(‖P0 − 2P1 + P2‖, ‖P1 − 2P2 + P3‖)` in px. Then
`‖B''(t)‖ ≤ 6L` on `[0, 1]`. Uniform chords of parameter width `1/n`
deviate from the curve by at most `(1/8)(1/n²)·max‖B''‖ ≤ 6L/(8n²)`.
Requiring `≤ ε`:

```
n = clamp( ceil( sqrt( 3L / (4ε) ) ), 1, FLATTEN_MAX )   with ε = 0.1 px
  = clamp( ceil( sqrt(7.5 · L) ), 1, 1024 )
```

- The chord points are `B(i/n)`, evaluated in Bernstein form with plain
  `+ − ×`. `mul_add` is forbidden, as in SPEC-0006 §2.7.
- `n` is a pure function of the pixel-space control points, which is
  R-0012 AC6.
- **Cost:** a 300 px-radius quarter circle has `L ≈ 138`, so `n = 33` and
  a full circle is about 132 chords.
- **`FLATTEN_MAX = 1024`** bounds memory against absurd off-canvas
  geometry. It is reached only for `L > 1.4 × 10⁵` px, which is documented
  as outside the tolerance claim.
- The output is one polyline per subpath, in pixel space, plus a `closed`
  flag.

### 2.6 The fill rasterizer (`ppm.rs`), realizing OQ-2

For one `Prim2::Path` with `fill = Some(f)`:

**Edges.** Flatten every subpath (§2.5). For **fill**, every subpath
contributes all of its chords plus its closing chord from its last point
to `start`, whether or not `closed` is set. That is SVG's implicit close.
Edges with a non-finite endpoint are dropped, with the same guard as
`draw_stroke`.

**Winding number** `w(p)`, using the half-open crossing rule of Sunday's
algorithm. For each edge `(a, b)`:

```
left = (b.x − a.x)(p.y − a.y) − (p.x − a.x)(b.y − a.y)
if a.y ≤ p.y < b.y and left > 0   → w += 1      // upward crossing, p on the left
if b.y ≤ p.y < a.y and left < 0   → w −= 1      // downward crossing, p on the right
```

It uses integers and exact sign tests on basic floating-point arithmetic,
so the result does not depend on edge order. `inside(p) ⇔ w(p) ≠ 0`.

**Boundary edges.** An edge is part of the *boundary* at a point only if
the inside test differs on its two sides. In a pentagram, the chords that
cross the central pentagon separate winding 2 from winding 1. That is
inside on both sides, so such chords must **not** draw a ramp, or the fill
would show seams (R-0012 AC3). The classification of edge `e` at its foot
point `f`, the point of `e` nearest the pixel centre, is:

```
n̂ = unit normal of e                       (one sqrt)
side⁺ = inside(f + δ·n̂),  side⁻ = inside(f − δ·n̂),   δ = 1/64 px
boundary ⇔ side⁺ ≠ side⁻ ;  the inside half-plane is the side that is inside
```

- `δ = 1/64` is a power of two, so the probe offsets introduce no extra
  rounding in the scaling.
- If `f` falls exactly on a crossing of two edges, a probe can land on the
  other edge. The half-open rule still assigns it deterministically, and
  the error is confined to pixels within `δ` of a self-intersection
  vertex. This is documented.

**Coverage**, per pixel centre `c` in the tile. The tile is the bounding box
of the edges, grown by 0.5 px and clamped:

```
among edges e with d(c, e) < 0.5 that are boundary at their foot point:
    take the one with least d; break ties by (d, s) lexicographically, where
    s = +d if c lies in e's inside half-plane, else −d
if such an edge exists:  cov = unit(0.5 + s)             // the SPEC-0006 ramp
else:                    cov = if inside(c) { 1 } else { 0 }
```

- **It is the stroke ramp.** With `r = 0` on each side, `unit(0.5 + s)` is
  `coverage(d, r)` from SPEC-0006 §2.5 extended through zero by sign.
  Where a path is both filled and stroked, the two agree on where the
  boundary is to within rounding.
- **Closed form per pixel.** A pixel's coverage depends only on its centre
  and the edge set. The minimum and the lexicographic tie-break do not
  depend on order. Nothing is carried from one pixel to the next, so
  SPEC-0006 §2.7 point 3 holds as written.
- **Cost-bounded evaluation.**
  - *Band pass.* For each edge, visit only the pixels of its own band (its
    bounding box grown by 0.5 px), as `draw_stroke` does. Compute `d`,
    classify when `d < 0.5`, and keep the best `(d, s)` in a scratch tile
    initialised to `+∞`.
  - *Interior pass.* Each remaining pixel needs only `inside(c)`. It is
    computed against a **per-row edge list**: the edges whose half-open
    y-range contains that row's centre line, built once per primitive. An
    edge not in the list contributes exactly 0 to `w`, so the result
    equals the full sum. This is an optimisation, not an approximation.
  - Probe points have arbitrary `y` and use the full edge list. They occur
    only in bands.
- **Compositing:** one `src_over(pixel, f.colour, cov · unit(f.alpha))` per
  pixel with `cov > 0`, in row-major order. That is exactly one composite
  per pixel for the fill, as SPEC-0006 §2.5 requires.

**Exact identities, which are the AC5 tests:**

| Case | Why it is exact |
|---|---|
| An axis-aligned rectangle with edges on pixel boundaries | Every pixel centre is at `d ≥ 0.5` from every edge, so it takes the `inside` branch and gets exactly 1 or 0. No fringe. |
| An edge through a column of pixel centres | `d = 0` and the edge is a boundary, so `cov = unit(0.5) = 0.5`, which composites to byte `round(127.5) = 128` for white on black. This matches the stroke table in SPEC-0006 §2.5. |
| An axis-aligned edge at any sub-pixel offset, away from corners | `cov = 0.5 ± d` equals the box-filtered area of the covered half-plane exactly. |
| Corners | Not exact. A Monte-Carlo sweep (3000 random offsets and sizes of axis-aligned rectangles, 2026-09-27) measured `|Σcov − area| ≤ 0.888 px²` for four corners and a per-pixel error of at most 0.249. **Pinned tolerance: `|Σcov − w·h| ≤ 0.25 px²` per convex right-angle corner.** QA verifies it on an exhaustive 1/16-px grid of offsets. |

### 2.7 Strokes of paths

- `push_segments` gains a `Prim2::Path` arm: flatten (§2.5) and push each
  chord, plus the closing chord **only** where `closed` is set.
- `draw_stroke` is otherwise unchanged. Round caps and joins come from the
  same union by `max`, so no new stroke code is written.
- **Order within one path primitive:** fill first (§2.6), then stroke. Each
  is one src-over per pixel. This matches SVG's `paint-order: normal`.
- The existing stroke guards still apply. A path with `width = 0` or
  `alpha = 0` paints no stroke, which is how a fill-only path is authored.
  `draw` routes `Prim2::Path` to a `draw_path` that runs `fill` (if `Some`)
  and then `draw_stroke`.

### 2.8 `SvgSink`

`Prim2::Path` emits one element:

```
<path d="M x,y L x,y C x1,y1 x2,y2 x,y Z M …" fill="#rrggbb" fill-opacity="a"
      fill-rule="nonzero" stroke="#rrggbb" stroke-width="w" stroke-opacity="a"
      stroke-linecap="round" stroke-linejoin="round"/>
```

- Absolute commands only, in image coordinates. The existing
  `scale(1 -1)` group handles y-down.
- Numbers use Rust's `{}` for `f64` (shortest round-trip), which is
  SPEC-0003's rule, so output is byte-deterministic.
- `Z` is written only where `closed` is set. Unclosed subpaths are still
  filled by the SVG renderer's implicit close, which matches §2.6.
- `fill = None` writes `fill="none"` and omits `fill-opacity` and
  `fill-rule`.
- **Sink agreement (R-0012 AC8)** follows from both sinks using the same
  image-space outline and the same pinned `to_pixel` mapping (SPEC-0006
  §2.3). The ±1 px claim is scoped as in R-0006 AC2.

### 2.9 Shape generators (`shapes.rs`, std-only)

Every generator returns `Vec<Subpath<Pt2>>` in planar model coordinates.
Orientation is **counter-clockwise** in y-up coordinates, stated once and
tested, so `reversed()` reliably makes a hole.

| fn | output | trig? |
|---|---|---|
| `circle(c, r)` | 1 closed subpath, 4 cubics starting at `(c.x + r, c.y)`, handle `K·r` with `const K: f64 = 4.0 * (SQRT_2 − 1.0) / 3.0` | **none** |
| `arc(c, r, start, sweep)` | open, `m = ceil(|sweep| / (τ/4))` cubics, each with handle `4/3 · tan(θ/4) · r` | `cos`, `sin`, `tan` |
| `sector(c, r, start, sweep)` | closed: `M c`, `L` to the arc start, the arc's cubics, `Z` | same as `arc` |
| `rounded_rect(c, w, h, rad)` | closed; `rad` clamped to `[0, min(w, h)/2]`; corners are quarter-circle cubics with handle `K·rad`; `rad = 0` gives 4 lines | **none** |
| `polygon(pts)` | closed lines | none |

**Degenerate inputs** never panic (R-0012 AC7):

- non-finite arguments, `r ≤ 0`, `sweep = 0`, `w ≤ 0` or `h ≤ 0`, or
  `pts.len() < 3` return an empty `Vec`;
- `|sweep| > τ` is clamped to `τ`.

`K` is a `const`, so it is evaluated at compile time from the exactly
rounded constant `SQRT_2` and pinned by a golden-constant test at
`0.5522847498307936`. The radial deviation `≤ 3 × 10⁻⁴ R` is measured
(`2.725 × 10⁻⁴` at 10⁵ samples, 2026-09-27).

### 2.10 Determinism (R-0012 AC9)

SPEC-0006 §2.7 carries over point by point:

- **Upstream bits.** Eval stays pure in `t`. Pinhole subdivision is a pure
  function of view-space control points.
- **Fixed order.** Subpaths, then segments, then chords, then edges, then
  pixels row-major.
- **No cross-pixel accumulation.** §2.6 shows coverage is closed-form per
  pixel.
- **No transcendental in the raster path.** Flattening and filling use
  `+ − × ÷`, `sqrt`, `floor`, `ceil`, `round`, `min`/`max` and
  comparisons. `mul_add` stays forbidden.
- **Trig lives in the scene.** `arc` and `sector` call `cos`, `sin` and
  `tan` at scene construction, the same caveat R-0003 carries. The golden
  scene uses only `circle`, `rounded_rect` and `polygon`, which are
  trig-free.

### 2.11 Compatibility (R-0012 AC10, AC11)

- **No existing byte moves.** Every existing `Prim2` variant takes an
  unchanged code path, and the new `match` arms are additive with no
  wildcard. R-0003, R-0006 and R-0009 goldens must stay byte-identical,
  and the suite asserts it.
- **`Style` literals.**
  - 36 sites in this repo gain `fill: None`, mostly in tests. The PR lists
    them.
  - **guion-video-creator** builds against `../motoreel` by path, so its 5
    literal sites break the moment this merges. A companion PR there adds
    `fill: None` and is merged in the same window. This is the one
    cross-repo coupling.
- **Dependencies.** Zero new ones. `path.rs` and `shapes.rs` are std-only.
- **Kernel build.** `--no-default-features` builds, lints and tests,
  because nothing here touches the `text` feature.

### 2.12 The demo (R-0012 AC12)

`examples/card/main.rs` renders a 1080 × 1920 clip at 30 fps, 3 s, to PPM
with the documented ffmpeg command. It shows:

- **Panel:** a `rounded_rect` filled `#141417` with a 1 px `#ffffff` stroke
  at alpha 0.10.
- **Friction circle:** a `circle` filled gold at alpha 0.16 and stroked
  steel, with a red `circle` dot orbiting on the rim. Its motion is a
  `Track` of motor keys, not a new mechanism.
- **Area under a curve:** a `polygon` of `y = k/x` samples closed down to
  the axis, filled gold-dim, with the curve stroked on top.

The colours are the Goosethropic tokens, **passed in by the example**. The
engine contains no brand.

## 3. Code outline

```rust
// path.rs
pub enum Seg<P> { Line(P), Cubic(P, P, P) }
pub struct Subpath<P> { pub start: P, pub segs: Vec<Seg<P>>, pub closed: bool }

impl<P: Copy> Subpath<P> {
    pub fn new(start: P) -> Self { Subpath { start, segs: Vec::new(), closed: false } }
    pub fn line_to(mut self, p: P) -> Self { self.segs.push(Seg::Line(p)); self }
    pub fn cubic_to(mut self, h1: P, h2: P, p: P) -> Self { self.segs.push(Seg::Cubic(h1, h2, p)); self }
    pub fn close(mut self) -> Self { self.closed = true; self }
    pub fn map<Q>(&self, f: impl Fn(&P) -> Q) -> Subpath<Q> { /* … */ }
    pub fn reversed(&self) -> Self { /* walk segs backwards, swap handles */ }
}

/// Chord count for ε = 0.1 px (§2.5): pure in the control points.
pub(crate) fn chords(p0: Pt2, p1: Pt2, p2: Pt2, p3: Pt2) -> u32 {
    let l = second_diff(p0, p1, p2).max(second_diff(p1, p2, p3));
    ((7.5 * l).sqrt().ceil() as u32).clamp(1, FLATTEN_MAX)
}

// ppm.rs
fn draw(&mut self, prim: &Prim2) {
    match prim {
        Prim2::Text { .. } => …,
        Prim2::Path { subpaths, style } => self.draw_path(subpaths, *style),
        Prim2::Point { .. } | Prim2::Segment { .. }
        | Prim2::Polyline { .. } | Prim2::Edges { .. } => self.draw_stroke(prim),
    }
}

fn draw_path(&mut self, subpaths: &[Subpath<Pt2>], style: Style) {
    if let Some(fill) = style.fill {
        self.draw_fill(subpaths, fill);          // §2.6 — one composite
    }
    self.draw_stroke_path(subpaths, style);      // §2.7 — reuses the segment core
}
```

Test layout: `tests/r0012_paths_fills.rs` holds one `mod` per AC. Goldens
are `tests/golden/r0012_paths.ppm` and `r0012_paths.svg`, blessed with the
existing `BLESS=1` scheme from SPEC-0003.

## 4. Non-goals

These carry over from R-0012 §4: gradients, even-odd, reveal and morph
animations, text as outlines, miter or bevel joins, GPU, and layout. In
addition:

- **Filling `Polyline`.** It is additive later, and `fill` is ignored
  there today (§2.2).
- **Stroke-to-outline conversion** (offset curves). Strokes stay a union of
  capsules.
- **Exact area coverage at corners.** It is bounded (§2.6), not eliminated.

## 5. Open questions

- **OQ-A (for the architect).** Should `path.rs` and `shapes.rs` be public
  modules (`motoreel::path::Subpath`) or re-exported at the root like
  `Pt2`? The spec proposes a public `path` module with `Seg` and `Subpath`
  re-exported at the root, and a public `shapes` module, since generators
  read best qualified (`shapes::circle`).
- **OQ-B.** Is `PINHOLE_REL_TOL = 5e-5` the right default, or should the
  scene expose it? The spec proposes a crate constant, which can be
  promoted to a field additively.

## 6. Acceptance criteria

Each item maps to an R-0012 AC and becomes a QA test.

- [ ] **AC1.** `Shape::Path` and `Prim2::Path` exist. Both sinks' `match`
  statements are exhaustive with no `_` arm, checked by grep in the test
  plan and by compilation.
- [ ] **AC2.** Fill only, stroke only, and both, render. With both, the
  fill composites before the stroke: a white-filled, red-stroked square has
  red on its boundary pixels, not white. Neither paints nothing.
- [ ] **AC3.** The pentagram's central pentagon is filled with **no seam**:
  its interior pixels are byte-equal to the fill colour. A same-orientation
  nested square is solid. An opposite-orientation one is a hole. An open
  subpath fills as if closed. Each case is asserted in both sinks: PPM by
  pixels, SVG by attributes plus the path data.
- [ ] **AC4.**
  - Orthographic: emitted cubic control points equal the projected model
    control points bit-for-bit.
  - Pinhole: dense sampling (10³ `t` per cubic) of the true projected curve
    lies within `τ` of the emitted pieces.
  - A control point behind the camera culls the whole path.
- [ ] **AC5.** The three exact identities in §2.6, and the 0.25 px²
  per-corner bound on a 1/16-px offset grid.
- [ ] **AC6.**
  - `chords` returns the §2.5 formula's value.
  - The measured chord deviation is ≤ 0.1 px over a sweep of cubics.
  - The stroked-circle band property holds for R ∈ {10, 100, 500} px and
    r ∈ {1, 2, 4} px.
- [ ] **AC7.**
  - `K` equals `0.5522847498307936` bit-exactly.
  - Circle radial error is ≤ 3e-4·R.
  - Generator orientation is CCW (positive signed area).
  - Arc end points lie on the circle.
  - The corner radius clamps, and `rad = 0` yields 4 lines.
  - Every degenerate input returns an empty `Vec` without panicking.
- [ ] **AC8.** The SVG `<path>` grammar matches exactly on a fixture, and
  PPM/SVG boundary agreement is within ±1 px.
- [ ] **AC9.** Two-render byte identity in both sinks. Trig-free goldens
  `r0012_paths.{ppm,svg}` match byte-for-byte.
- [ ] **AC10.** All pre-existing goldens are byte-unchanged and all
  pre-existing tests pass. The only edits to old tests are `fill: None`,
  and they are listed.
- [ ] **AC11.** `cargo tree -p motoreel` shows no new dependencies, and the
  `--no-default-features` build, clippy and tests are green.
- [ ] **AC12.** `cargo run --example card` produces frames. An
  ffmpeg-gated test encodes them, skipping (not failing) when ffmpeg is
  absent.

## 7. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-09-27 | Fill coverage uses **boundary-classified** signed distance, with each nearby edge tested by winding probes at `±δ` | Plain SDF plus winding sign draws seams along internal chords of self-intersecting or nested same-orientation outlines (pentagram, nested squares), which R-0012 AC3 forbids. Probing keeps the rule closed-form per pixel and so keeps SPEC-0006 §2.7 intact |
| 2026-09-27 | Only `Prim2::Path` honours `Style::fill` in this spec | `Point`, `Segment` and `Edges` have no area. Filling `Polyline` is additive later. This sharpens R-0012's OQ-3 rationale, which read "any primitive can be filled": any primitive can *carry* a fill, and this spec defines where it is *honoured* |
| 2026-09-27 | Pinhole: adaptive subdivision into cubics, not flattening to lines | The SVG keeps curves, and the tolerance is checked where it is defined, in image space. De Casteljau halving uses only averages |
| 2026-09-27 | Chord count from a closed-form bound on `‖B''‖`, not an adaptive search | A bound is a pure function of the control points. Adaptive flatness searches are equally deterministic but harder to state as a guarantee |

## Changelog

- 2026-09-27: created (Draft) from the accepted R-0012.
