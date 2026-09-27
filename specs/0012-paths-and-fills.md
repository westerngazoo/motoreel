# SPEC-0012 — Paths and fills

- **Status:** **Accepted** (2026-09-27, owner). The architect review's 15 findings are applied and the R-0012 AC6 amendment is approved. Implemented and in review as westerngazoo/motoreel#3 (architect: APPROVE, conditional on the companion PRs of §2.11)
- **Realizes:** R-0012
- **Author:** Claude (main session) with owner
- **Created:** 2026-09-27
- **Depends on:** SPEC-0002 (`Shape`, `Prim2`, projection, cull),
  SPEC-0003 (`SvgSink`, number formatting, golden scheme), SPEC-0006
  (`PpmSink`, coverage ramp, determinism argument §2.7)
- **Module(s):**
  - new `crates/motoreel/src/path.rs`: generic path types, builder, `map`,
    `reversed` and de Casteljau split. It has no point type of its own and no
    flattening
  - new `crates/motoreel/src/shapes.rs`: shape generators
  - `crates/motoreel/src/prim.rs`: `Fill`, `Style::fill`, `Prim2::Path`
  - `crates/motoreel/src/object.rs`: `Shape::Path`
  - `crates/motoreel/src/scene.rs`: path projection
  - `crates/motoreel/src/ppm.rs`: pixel-space flattening (on its own `Px`),
    the fill rasterizer and path strokes
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

`path.rs` depends on `std` alone and **names no concrete point type**. It
imports nothing from `prim.rs`, which imports `Subpath` from it, so the
module graph has no cycle (CLAUDE.md §2; architect finding 4). One generic
definition serves model space (`pga::Point`), image space (`Pt2`) and the
raster sink's pixel space (`Px`).

```rust
/// One piece of a subpath, ending at its last point.
#[derive(Clone, Debug, PartialEq)]
pub enum Seg<P> {
    Line(P),              // straight to P
    Cubic(P, P, P),       // handles h1, h2, then the end point
}

/// A connected outline: a start point and the segments that follow it.
#[derive(Clone, Debug, PartialEq)]
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
- **`split_cubic(p0, p1, p2, p3, mid)`** is the generic de Casteljau halving
  at `t = ½`, where `mid: Fn(&P, &P) -> P` is the caller's midpoint. It
  returns the two halves' control points. Pinhole subdivision (§2.4) calls
  it with a view-space midpoint on `to_euclidean` tuples. R-0013 (reveal
  and morph) will reuse it for truncation and alignment, which is why it
  stays generic.
- The derives are required because `Shape` and `Prim2` derive `Clone,
  Debug, PartialEq`.
- **`Seg` also derives `Copy`.** That is public API, since every variant
  holds only `P: Copy` points. It lets flattening and projection pass
  segments by value. `Subpath` is not `Copy`, because it owns a `Vec`.
  Recorded after the PR review.

### 2.2 Fill paint (`prim.rs`)

```rust
/// A flat fill: colour and opacity. Gradients are a later requirement.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fill { pub colour: Rgb, pub alpha: f64 }

impl Fill {
    /// The constructor downstream code should use, so the gradients
    /// requirement can change `Fill` without breaking literals again.
    pub const fn solid(colour: Rgb, alpha: f64) -> Self { Fill { colour, alpha } }
}

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
  - A non-finite coordinate anywhere, including in a projected pinhole
    piece, culls the **whole** primitive. This keeps SPEC-0002 §2.5's
    invariant total.
  - **Order: cull, then drop.** Every control point must project,
    including the start point of a subpath that has no segments, before
    empty subpaths are dropped. This matches AC4's "any control point"
    (recorded after the PR review).

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
   - **What the estimate is.** At each probe, the same-parameter distance
     bounds the geometric distance from `project(Q(t))` to `q` from above.
     It is a probe-point estimate, not a bound between probes. Architect
     measurements (2026-09-27) put the same-parameter distance between
     probes at up to 1.09 τ, while the **geometric** deviation of emitted
     pieces stayed within 0.07 τ (emitted → true) and about 0.55 τ
     (true → emitted, on a coarser polyline). AC4's test therefore measures
     **geometric nearest-point distance in both directions**, never the
     same-parameter one. Because the estimate also penalises the
     reparametrisation perspective introduces, it over-subdivides. That
     costs pieces, not accuracy: 131 per quarter circle at nearest depth
     0.5, and 388 to 830 closer in.
   - **Termination:** a hard depth limit `PINHOLE_MAX_DEPTH = 12`, i.e. at
     most 4096 pieces per authored cubic. A piece still over the estimate
     at the limit is emitted anyway. The limit is reached for curves that
     are **large relative to their nearest depth**. For example, a 3-unit
     ground-plane quarter circle whose nearest depth is 0.005 forced 232 of
     830 pieces. Even there the geometric error stayed small, because the
     over-estimate is reparametrisation, not deviation. A rigorous
     alternative, subdividing until the ratio of maximum to minimum
     control-point depth is at most `1 + κ·τ/diameter`, is recorded as a
     possible later refinement.
   - **Compound bound per sink.** SVG receives pieces within `τ` of the
     true image. PPM then flattens them (§2.5), so the rendered outline is
     within `τ + 0.1 px`, which is 0.154 px at 1080 × 1920. That compound
     figure is AC4's "flattening tolerance" for the pinhole camera.
   - Only `+ − × ÷` enter subdivision and projection. Subdivision works on
     `to_euclidean` `(x, y, z)` tuples through `path::split_cubic` with a
     tuple midpoint. Pinhole projection already divides by depth, so
     determinism is unchanged.
   - `Seg::Line` pieces project their end points. A projective map sends
     lines to lines, so they are exact.

### 2.5 Flattening for the raster sink (`ppm.rs`, on `Px`)

Flattening is a pixel-space concept (ε is 0.1 **px**), and its only
consumer is the raster sink, so it lives in `ppm.rs` and works on that
module's `Px` tuples (architect finding 4). The PPM sink maps control
points to pixel space with `to_pixel`, which is affine, so this is exact. It then flattens each cubic into `n` chords with
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
  `+ − ×`. `mul_add` and `hypot` are forbidden in all path and fill code.
  The norm in `L` is `sqrt(dx·dx + dy·dy)`.
- The derivation is verified: `B''` is a linear blend of `6·Δ²`, so
  `‖B''‖ ≤ 6L`. Over 3000 random cubics, including cusps, the worst
  measured chord deviation was 0.0975 px and measured/bound ≤ 0.997.
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
- **Non-finite endpoint:** if any edge has one, the **fill paints
  nothing**. Dropping a single edge would open a closed loop and corrupt
  the winding number across whole rows (architect finding 7). The stroke
  keeps its per-segment `retain`. Eval's finite invariant makes this a
  tripwire, not a path taken in practice.
- **Zero-length edges** (`a == b`) are **skipped** for fill. They add
  exactly 0 to the winding, and their normal would be NaN (finding 6). The
  generators also omit zero-length sides (§2.9).

**Winding number** `w(p)`, using the half-open crossing rule of Sunday's
algorithm. For each edge `(a, b)`:

```
left = (b.x − a.x)(p.y − a.y) − (p.x − a.x)(b.y − a.y)
if a.y ≤ p.y < b.y and left > 0   → w += 1      // upward crossing, p on the left
if b.y ≤ p.y < a.y and left < 0   → w −= 1      // downward crossing, p on the right
```

It uses integer counts and the **deterministic** sign of a rounded cross
product. That is not the exact orientation predicate, but it is the same
bits every run (finding 15). The result does not depend on edge order. `inside(p) ⇔ w(p) ≠ 0`.

**Boundary edges.** An edge is part of the *boundary* at a point only if
the inside test differs on its two sides. In a pentagram, the chords that
cross the central pentagon separate winding 2 from winding 1. That is
inside on both sides, so such chords must **not** draw a ramp, or the fill
would show seams (R-0012 AC3). The classification of edge `e` at its foot
point `f`, the point of `e` nearest the pixel centre, is:

```
t  = clamp(((c − a)·(b − a)) / |b − a|², 0, 1)
f  = a if t == 0;  b if t == 1;  a + t·(b − a) otherwise    // endpoints VERBATIM
n̂ = unit normal of e                                         (one sqrt)
side⁺ = inside(f + δ·n̂),  side⁻ = inside(f − δ·n̂),   δ = 2⁻²⁰ px
boundary ⇔ side⁺ ≠ side⁻ ;  the inside half-plane is the side that is inside
```

- **The foot point is pinned (finding 2).** When `t` clamps, `f` is the
  endpoint itself, because `a + 1.0·(b − a)` is not `b` in floating point.
  At a right-angle corner the probe must land exactly where the geometry
  says.
- **`δ = 2⁻²⁰` px (finding 5).** It is a power of two, and far above the
  rounding of coordinates on any canvas up to 2¹⁶ px. With the earlier
  1/64, slivers thinner than about 1/64 px flipped abruptly from a smooth
  ramp to aliased full-coverage specks. At 2⁻²⁰ that jump moves into the
  fully degenerate range, and the zone near self-intersections where a
  probe can land on another edge shrinks to 2⁻²⁰ px. The residual
  behaviour is documented, not claimed away.
- **Sub-pixel slivers over-ink,** as sub-pixel strokes do (SPEC-0006
  §2.8). A sliver of width `w < 1` px paints about `(0.5 + w/2)²` of ink
  per unit length rather than `w`. This regime is documented and outside
  AC5's claim.

**Coverage**, per pixel centre `c` in the tile. The tile is the bounding box
of the edges, grown by 0.5 px and clamped:

```
among edges e with d(c, e) < 0.5 that are boundary at their foot point:
    take the one with least d; break ties by (d, −s) lexicographically
    (at equal d the inside sign wins; owner decision), where
    s = +d if c lies STRICTLY in e's inside half-plane, else −d
        (c exactly on e's line, which happens beyond an endpoint, gives s = −d)
if such an edge exists:  cov = unit(0.5 + s)             // the SPEC-0006 ramp
else:                    cov = if inside(c) { 1 } else { 0 }
```

- **It is the stroke ramp.** With `r = 0` on each side, `unit(0.5 + s)` is
  `coverage(d, r)` from SPEC-0006 §2.5 extended through zero by sign.
  Where a path is both filled and stroked, the two agree on where the
  boundary is to within rounding.
- **Closed form per pixel.** A pixel's coverage depends only on its centre
  and the edge set. The minimum and the lexicographic tie-break do not
  depend on order. Nothing is carried from one pixel to the next.
  SPEC-0006 §2.7 point 3 names "scanline edge list" among the forbidden
  classes. The per-row list below is **not** that class: it is a pure
  filter with exact set equality (an edge is in a row's list if and only
  if its half-open y-range contains that row's centre line), and it carries
  no incremental state. No `x += dx` is walked across a row. The
  SPEC-0006 text is clarified accordingly (finding 12).
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
  pixel with `cov > 0`, in row-major order. SPEC-0006 §2.5's "exactly one
  composite per pixel per primitive" becomes, for `Path`, **one composite
  per pixel per paint: fill, then stroke** (finding 12).

**Exact identities, which are the AC5 tests:**

| Case | Why it is exact |
|---|---|
| An axis-aligned rectangle with edges on pixel boundaries | Every pixel centre is at `d ≥ 0.5` from every edge, so it takes the `inside` branch and gets exactly 1 or 0. No fringe. |
| An edge through a column of pixel centres | `d = 0` and the edge is a boundary, so `cov = unit(0.5) = 0.5`, which composites to byte `round(127.5) = 128` for white on black. This matches the stroke table in SPEC-0006 §2.5. |
| An axis-aligned edge at any sub-pixel offset, away from corners | `cov = 0.5 ± d` equals the box-filtered area of the covered half-plane exactly. |
| Corners | Not exact; see the bound below. |

**Corner bound (AC5).** For an axis-aligned rectangle with
**`min(w, h) ≥ 1 px`**:

    |Σcov − w·h| ≤ 0.25 px² per convex right-angle corner

**Why.**
- *Away from corners, the error is zero.* Along a straight axis-aligned
  side, `0.5 ± d` is the exact box-filtered area of a half-plane.
- *So all error sits in the vertex regions.* Error arises only where a
  pixel's footprint meets two sides. For the rectangle that is the 2 × 2
  pixel neighbourhood of each vertex. There, the ramp uses the distance to
  the *nearer* side (or to the vertex itself, when the pixel lies beyond
  both sides), whereas the true coverage is the *product* of the two
  one-dimensional overlaps.
- *Per-pixel error is at most 0.25.* The worst case is a pixel centred
  exactly on the vertex. Its true covered area is `0.5 × 0.5 = 0.25`
  (convex corner) or `0.75` (a hole's corner). What the rule paints there
  depends on the corner's orientation:
  - where a probe classifies an adjacent edge as boundary, `d = 0` and the
    ramp gives `0.5`;
  - where the probes at the shared vertex classify neither edge as
    boundary, the pixel takes the `inside` branch and gets `0` (convex)
    or `1` (hole corner).

  Every case is within 0.25 of the true area. An earlier draft said the
  vertex pixel always gets 0.5; QA found this false at one orientation
  (step 3), and the bound is unaffected.
- *Corners are independent* once each side is at least 1 px. The vertex
  regions of adjacent corners do not share a pixel whose footprint meets
  both of each corner's sides.

**Measured on an exhaustive grid** (architect, 2026-09-27): all 16 × 16
sub-pixel offsets, with `w, h ∈ [1, 3]` px in 1/16 steps.
- Worst total: `0.984375 px²`, which is 0.246 per corner. It occurs at
  `18/16 × 18/16` with offset `7/16`.
- The margin to the pinned `4 × 0.25` is **1.6 %**, and is stated as such.
- The per-pixel error reaches exactly 0.25.

**Below 1 px it breaks down.** For thinner rectangles the error grows with
length, not with the number of corners. On 5 px-long rectangles the total
error was 2.69 px² at `w = 1/16`, 2.25 at 1/4 and 1.5 at 1/2. This is the
sliver regime noted above.

**The tie rule is load-bearing.** The strict half-plane test gives the
0.984 above; the inclusive reading gives 1.559, which would break AC5.

**Hole corners are not bounded by AC5, and are worse.** AC5 bounds
*convex* corners only. The PR review measured a square hole in a filled
cell (four hole corners) with `w, h ∈ [1, 3]` px on the 1/16-px offset
grid.
- The worst case is **`|Σcov − area| = 2.24 px²`**, at `w = 1`,
  `h = 1.625`, offset `(8/16, 15/16)`.
- The cause is a pixel centre on an edge's line just past a hole corner:
  it lies inside the fill but takes `s = −d` under the lexicographic
  `(d, s)` tie-break.
- Reversing that tie-break (prefer `+d`) lowers the worst case to 1.55
  and still passes the convex suite. δ is not pinned by a test; the
  tie-break direction is (see below).

**Owner decision (2026-09-27): the tie-break is reversed**, so at equal
`d` the inside sign `+d` wins. The same grid then measures **1.541 px²**
at `w = h = 1.5625`, offset `(15/16, 15/16)`. The convex figures above do
not move (0.984), and no golden byte changes.
`hole_corner_error_is_bounded_with_the_inside_sign_tie_break` pins it at
1.6 + n/510: reverting the tie-break fails it at 1.62 on its first
rectangle.

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
- **A path whose stroke paints nothing** (width not finite and > 0, or
  alpha 0, the PPM guard's own test) writes `stroke="none"` and omits the
  other stroke attributes. Fill-only paths are then honest in SVG too.
  This touches no existing bytes, because only `Path` does it.
- **Sink agreement (R-0012 AC8)** follows from both sinks using the same
  image-space outline and the same pinned `to_pixel` mapping (SPEC-0006
  §2.3). The ±1 px claim is scoped as in R-0006 AC2.

### 2.9 Shape generators (`shapes.rs`, std-only)

Every generator returns `Vec<Subpath<Pt2>>` in planar model coordinates.
**Orientation (owner decision, 2026-09-27):**
- `circle`, `arc`, `sector` and `rounded_rect` always return
  **counter-clockwise** subpaths in y-up coordinates. A negative `sweep`
  describes the same region and is normalised to the counter-clockwise
  traversal.
- `polygon` **keeps the caller's vertex order**. That is the natural way
  to author a hole, which needs the opposite orientation.

So `reversed()` on any shape generator's output reliably makes a hole.

| fn | output | trig? |
|---|---|---|
| `circle(c, r)` | 1 closed subpath, 4 cubics starting at `(c.x + r, c.y)`, handle `K·r` with `const K: f64 = 4.0 * (SQRT_2 − 1.0) / 3.0` | **none** |
| `arc(c, r, start, sweep)` | open, `m = ceil(|sweep| / (τ/4))` cubics, each with handle `4/3 · tan(θ/4) · r` | `cos`, `sin`, `tan` |
| `sector(c, r, start, sweep)` | closed: `M c`, `L` to the arc start, the arc's cubics, `Z` | same as `arc` |
| `rounded_rect(c, w, h, rad)` | closed; `rad` clamped to `[0, min(w, h)/2]`; corners are quarter-circle cubics with handle `K·rad`; `rad = 0` gives a rectangle (3 lines + close) | **none** |
| `polygon(pts)` | closed lines | none |

**Generators never emit zero-length sides.** For example, `rounded_rect`
with `rad = min(w, h)/2` omits its vanished straight sides. `rad = 0`
emits exactly 3 `Line`s plus `close()`, because the closing side is
implicit, so no zero-length closing chord exists.

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
  comparisons.
- **The ban on `mul_add` and `hypot` is scoped (finding 10)** to the new
  path, flattening and fill code, wherever it lives (`ppm.rs`, `path.rs`,
  `shapes.rs`, the `scene.rs` path arm). QA checks it by grep over those
  functions.
- **One pre-existing exception is recorded, not fixed:** `ppm.rs`'s
  `blend_pixel` (the R-0009 text path) already calls `mul_add`, despite the
  module doc. Changing it could move R-0009 golden bytes, which AC10
  forbids. It is left to its own change.
- **Trig lives in the scene.** `arc` and `sector` call `cos`, `sin` and
  `tan` at scene construction, the same caveat R-0003 carries.

**The golden scene is pinned (finding 11).**
- **Canvas and view:** `PpmSink::with_view(dir, (320, 180), (3.2, 1.8))`,
  so `s = 100` px per image unit, on a black background. `SvgSink` uses
  the same view.
- **Camera:** `Camera::default()` (orthographic). Every object has a
  single-key track whose pose is `identity` or `translator` only. Every
  vertex is in the `z = 0` model plane.
- **Objects**, in insertion order:

  1. `circle((−1.0, 0.0), 0.5)`, filled `#e9b23f` at alpha 1, stroke width
     0 (so no stroke).
  2. `rounded_rect((0.2, 0.0), 1.0, 0.8, 0.2)`, filled `#141417` at alpha
     1, stroked `#ced1d9` with width 0.02. That is `r = 1` px, inside
     SPEC-0006's placement regime.
  3. A hole: `polygon` square with half-side 0.35 centred at `(1.15, 0.0)`,
     plus the same square at half-side 0.15 `.reversed()`, in one path,
     filled `#e0322a` at alpha 1 with no stroke.

- **Trig-free:** `circle`, `rounded_rect` and `polygon` need only
  `+ − × ÷` and the `const K`.

### 2.11 Compatibility (R-0012 AC10, AC11)

- **No existing byte moves.** Every existing `Prim2` variant takes an
  unchanged code path, and the new `match` arms are additive with no
  wildcard. R-0003, R-0006 and R-0009 goldens must stay byte-identical,
  and the suite asserts it.
- **`Style` literals.**
  - 36 sites in this repo gain `fill: None`, mostly in tests. The PR lists
    them.
  - **Two downstream repositories build against `../motoreel` by path**
    (finding 9), so their literals break the moment this merges:
    - **guion-video-creator**, 5 sites: `guion-render/src/lib.rs:61`,
      `guion-assemble/src/style.rs:14` and
      `apps/guion-playground/src/render.rs:23/38/46`.
    - **guion**, the legacy repository merged into guion-video-creator
      (ENCARGO §0.2), 1 site: `crates/guion-assemble/src/lib.rs:87`.
  - Companion PRs add `..Style::default()` rather than `fill: None`, so
    the next field addition breaks nothing. They are merged in the same
    window.
  - **Correction (PR review):** `guion-video-creator`'s
    `object_from_shape` (`guion-assemble/src/lib.rs`) matches `Shape`
    **exhaustively**, so it also needs a `Path` arm. `guion` maps from its
    own enum and needs only the literal.
  - Companion PRs: **westerngazoo/guion-video-creator#7** (5 literals and
    the `Path` arm) and **westerngazoo/guion#1** (1 literal).
    - Both were verified locally against this branch: clippy
      `-D warnings` and tests green, including guion-video-creator's Tauri
      apps.
    - **Merge order: motoreel#3 first.** guion-video-creator's CI checks
      out motoreel's default branch, and before this merges neither
      `Shape::Path` nor `fill` exists there.
  - Inside this repo, two test helpers match `Prim2` exhaustively without
    `_`: `prim_parts` in `r0002_scene_camera.rs:61` and in
    `r0004_physics_playback.rs:227`. Each gains a `Path` arm. R-0012 AC10
    was amended to allow exactly this.
- **CI gains two steps in the implementation PR** (found by QA):
  - `cargo test -p motoreel --no-default-features`, because AC11 promises
    the kernel build and nothing checked it;
  - a guard that fails the job if the AC12 encode test reports it was
    skipped while ffmpeg is installed.
  - Whether `guion` is formally retired is the owner's call; until then it
    gets its one-line PR.
- **Dependencies.** Zero new ones. `path.rs` and `shapes.rs` are std-only.
- **Kernel build.** `--no-default-features` builds, lints and tests,
  because nothing here touches the `text` feature.

### 2.12 The demo (R-0012 AC12)

`examples/card/main.rs` renders a 1080 × 1920 clip at 30 fps, 3 s, to PPM
with the documented ffmpeg command.

- **Portrait view (finding 14).** The scene's `view` must match the sink's
  view: `Scene::view = (1.8, 3.2)` and
  `PpmSink::with_view(dir, (1080, 1920), (1.8, 3.2))`, so `s = 600` px per
  unit. Otherwise `render` rejects the pair.

It shows:

- **Panel:** a `rounded_rect` filled `#141417` with a 1 px `#ffffff` stroke
  at alpha 0.10. A 1 px hairline is `r = 0.5` px, below SPEC-0006's
  `r ≥ 1` placement regime. That is acceptable for a demo and is noted, not
  claimed.
- **Friction circle:** a `circle` filled gold at alpha 0.16 and stroked
  steel, with a red `circle` dot orbiting on the rim. Its motion is a
  `Track` of motor keys, not a new mechanism. The track has **at least 3
  keys per revolution**, because a 2-key slerp cannot make a full turn.
- **Area under a curve:** a `polygon` of `y = k/x` samples closed down to
  the axis, filled gold-dim, with the curve stroked on top.

The colours are the Goosethropic tokens, **passed in by the example**. The
engine contains no brand.

## 3. Code outline

```rust
// path.rs: std-only, names no concrete point type
#[derive(Clone, Debug, PartialEq)]
pub enum Seg<P> { Line(P), Cubic(P, P, P) }
#[derive(Clone, Debug, PartialEq)]
pub struct Subpath<P> { pub start: P, pub segs: Vec<Seg<P>>, pub closed: bool }

impl<P: Copy> Subpath<P> {
    pub fn new(start: P) -> Self { Subpath { start, segs: Vec::new(), closed: false } }
    pub fn line_to(mut self, p: P) -> Self { self.segs.push(Seg::Line(p)); self }
    pub fn cubic_to(mut self, h1: P, h2: P, p: P) -> Self { self.segs.push(Seg::Cubic(h1, h2, p)); self }
    pub fn close(mut self) -> Self { self.closed = true; self }
    pub fn map<Q>(&self, f: impl Fn(&P) -> Q) -> Subpath<Q> { /* … */ }
    pub fn reversed(&self) -> Self { /* walk segs backwards, swap handles */ }
}

/// de Casteljau halving at t = ½, generic over the caller's midpoint.
pub fn split_cubic<P: Copy>(p: [P; 4], mid: impl Fn(&P, &P) -> P) -> ([P; 4], [P; 4]) { /* … */ }

// ppm.rs: flattening lives here, on the module's own `Px = (f64, f64)`
/// Chord count for ε = 0.1 px (§2.5): pure in the control points.
fn chords(p0: Px, p1: Px, p2: Px, p3: Px) -> u32 {
    let l = second_diff(p0, p1, p2).max(second_diff(p1, p2, p3)); // sqrt(dx·dx + dy·dy), no hypot
    ((7.5 * l).sqrt().ceil() as u32).clamp(1, FLATTEN_MAX)
}

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

### 2.13 Clarifications (QA step 3, owner-approved 2026-09-27)

- **`split_cubic`** takes the array form of §3,
  `split_cubic(p: [P; 4], mid)`. §2.1's five-argument wording is
  superseded.
- **`polygon(pts: &[Pt2])`.** Consecutive duplicate vertices, including
  the last equalling the first, are dropped, per the no-zero-length-sides
  rule. Fewer than 3 vertices remaining gives an empty `Vec`. The count
  is of vertices remaining, not distinct ones (owner decision,
  2026-09-27), so `[a, b, a, b]` is the zero-area 4-gon.
- **An infinite `r`, `rad`, `w`, `h` or `sweep`** counts as non-finite and
  returns an empty `Vec`. The `|sweep| > τ` clamp applies to finite values
  only.
- **A non-finite fill alpha** paints nothing in PPM. In SVG it writes
  `fill="none"` rather than `fill-opacity="NaN"`, so the sinks agree.
  This includes `+∞`.
  - **This is deliberately asymmetric with strokes.** A stroke alpha of
    `+∞` still paints as 1, because SPEC-0006's total `unit` helper clamps
    it, and R-0012 AC10 freezes that behaviour.
  - Fill alphas are new, so they take the stricter reading. The asymmetry
    is recorded here after the PR review rather than left for a reader to
    discover.
- **SVG `d` whitespace** is exactly: commands separated by one space, the
  coordinates of a point joined by a comma, and the points of a `C`
  separated by one space, e.g. `M 1,2 C 3,4 5,6 7,8 Z`. The AC8 fixture
  pins it.
- **The card example** is split into `examples/card/main.rs` and
  `examples/card/scene.rs` with `pub fn scene() -> Scene`, following
  R-0003's precedent, so that the AC12 test builds the same scene the
  example renders.

## 5. Open questions

- **OQ-A: resolved (architect).** A public `path` module, with `Seg` and
  `Subpath` re-exported at the root, and a public `shapes` module. This
  matches the existing `label`, `ppm` and `svg` pattern.
- **OQ-B: resolved (architect).** `PINHOLE_REL_TOL` is a crate constant;
  promoting it to a field later is additive.
- **Owner: the AC6 amendment: resolved** (approved 2026-09-27).

## 6. Acceptance criteria

Each item maps to an R-0012 AC and becomes a QA test.

- [ ] **AC1.** `Shape::Path` and `Prim2::Path` exist. Both sinks' `match`
  statements are exhaustive with no `_` arm, checked by grep in the test
  plan and by compilation.
- [ ] **AC2.** Fill only, stroke only, and both, render. With both, the
  fill composites before the stroke: a white-filled, red-stroked square has
  red on its boundary pixels, not white. Neither paints nothing.
- [ ] **AC3.** The pentagram's central pentagon is filled with **no seam**:
  its interior pixels are byte-equal to the fill colour. "Interior" means
  a distance of **at least 0.5 px from the 10-vertex outer outline**, not
  "winding number 2". Pixels near the star's concave inner vertices
  correctly take the ramp. A same-orientation
  nested square is solid. An opposite-orientation one is a hole. An open
  subpath fills as if closed. Each case is asserted in both sinks: PPM by
  pixels, SVG by attributes plus the path data.
- [ ] **AC4.**
  - Orthographic: emitted cubic control points equal the projected model
    control points bit-for-bit.
  - Pinhole: the **geometric nearest-point** distance, measured **in both
    directions** between the true projected curve (10³ samples per cubic)
    and the emitted pieces, is `≤ τ` for SVG and `≤ τ + 0.1 px` for PPM.
    Same-parameter distance is not used (§2.4).
  - A control point behind the camera culls the whole path.
- [ ] **AC5.**
  - The three exact identities in §2.6.
  - The 0.25 px² per-corner bound for `min(w, h) ≥ 1 px`, on the
    exhaustive 16 × 16 offset grid with sizes in 1/16 steps.
  - A test pins the strict half-plane tie rule: flipping it to inclusive
    must fail the corner bound.
- [ ] **AC6.**
  - The measured chord deviation is ≤ 0.1 px over a sweep of random
    cubics, including cusps. This sweep is the meaningful test; a test that
    `chords` equals its own formula would only restate it.
  - The stroked-circle band property holds for R ∈ {10, 100, 500} px and
    r ∈ {1, 2, 4} px, with the band `R ± (r + 0.5 + 0.1 + 3·10⁻⁴·R)` from
    the amended R-0012 AC6.
- [ ] **AC7.**
  - `K` equals `0.5522847498307936` bit-exactly.
  - Circle radial error is ≤ 3e-4·R.
  - Generator orientation is CCW (positive signed area).
  - Arc end points lie on the circle.
  - The corner radius clamps; `rad = 0` yields 3 `Line`s plus `close()`;
    no generator emits a zero-length side.
  - Every degenerate input returns an empty `Vec` without panicking.
- [ ] **AC8.** The SVG `<path>` grammar matches exactly on a fixture, and
  PPM/SVG boundary agreement is within ±1 px.
- [ ] **AC9.**
  - Two-render byte identity in both sinks.
  - The pinned trig-free golden scene (§2.10) in `r0012_paths.{ppm,svg}`
    matches byte-for-byte.
  - The scoped grep finds no `mul_add` or `hypot` in path, fill or
    flattening code.
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
| 2026-09-27 | Architect review: REQUEST CHANGES, all 15 findings applied | Flattening moved to `ppm.rs` on `Px`, removing the `prim` ↔ `path` cycle. The foot point and the strict half-plane tie rule are pinned, because the tie rule is load-bearing (0.984 vs 1.559 px²). Corner bound gains `min(w, h) ≥ 1 px`, exhaustive-grid numbers and a derivation. `δ` 1/64 → 2⁻²⁰. Zero-length edges skipped. Any non-finite edge means no fill. Pinhole wording corrected, with the compound bound stated. Both downstream repos listed. `mul_add` ban scoped and the pre-existing exception recorded. Golden scene and demo pinned. `Fill::solid` added. `stroke="none"` for no-stroke paths |
| 2026-09-27 | **R-0012 AC6 amendment (owner-approved 2026-09-27).** The stroked-circle band becomes `R ± (r + 0.5 + 0.1 + 3·10⁻⁴·R)` | The 4-cubic circle deviates only *outward*, by up to 2.725·10⁻⁴·R (0.136 px at R = 500), and chords only sag inward, so nothing absorbs it. As written, AC6 contradicts R-0012's own "0.13 px at R = 500": measured lit extent reaches `r + 0.5 + 0.129`. The alternative, capping the test at R ≤ 333 px, would leave the requirement false for larger circles, so it was rejected |
| 2026-09-27 | **Owner decisions after PR #3 review.** (1) The fill's `(d, s)` tie-break is reversed: at equal distance the inside sign wins. (2) `polygon` counts vertices remaining, not distinct | (1) Hole corners, which AC5 does not bound, drop from 2.24 to 1.541 px²; convex corners and every golden are unchanged; a test pins the direction. (2) `[a, b, a, b]` has no zero-length side, so the zero-area 4-gon is kept and the wording follows the implementation |

## Changelog

- 2026-09-27: created (Draft) from the accepted R-0012.
- 2026-09-27: revised after architect review; all 15 findings applied, and the AC6 amendment proposed to the owner.
- 2026-09-27: **Accepted** by the owner, with the AC6 amendment approved.
- 2026-09-27: QA step-3 findings resolved by the owner. AC10 amended; generator orientation settled (§2.9); §2.6 vertex wording corrected; clarifications recorded in §2.13; CI steps added (§2.11).
- 2026-09-27: PR #3 architect review (APPROVE, conditional on the companion PRs) recorded: `Copy` on `Seg`, cull-then-drop order, the +∞ alpha asymmetry, hole-corner error with the tie-break decision left to the owner, companion PRs and the §2.11 correction.
- 2026-09-27: owner decisions: the `(d, s)` tie-break is reversed (inside sign wins; hole corners 2.24 → 1.541 px², pinned by a test), and `polygon` counts vertices remaining, not distinct (`[a, b, a, b]` is the zero-area 4-gon).
