# SPEC-0006 — `PpmSink`: P6 raster frames and the in-crate stroke rasterizer

- **Status:** Draft — architect review 2026-08-27 (REQUEST CHANGES)
  applied; awaiting owner acceptance
- **Realizes:** R-0006
- **Author:** Claude (main session) with owner
- **Created:** 2026-08-27
- **Depends on:** SPEC-0002 (`Prim2`, projection), SPEC-0003 (`FrameSink`,
  `Scene::render`, byte-determinism discipline, golden scheme),
  SPEC-0004 (`Prim2::Edges`)
- **Module(s):** `crates/motoreel/src/ppm.rs`; additive amendments to
  `lib.rs` and `examples/first_light/main.rs`; a repo-root `.gitattributes`
  (§2.11). §2.13 amends SPEC-0003 §2.5 in text only — no code, no bytes.

## 1. Motivation

R-0006 is a bug report: the encode command motoreel documents does not work
on an ordinary machine, because ffmpeg has an `svg_pipe` demuxer but no SVG
*decoder* without librsvg. Every STEM creator — the audience the project
acquired on 2026-08-27 — hits it on their first attempt.

This spec adds `PpmSink`: a second `FrameSink` writing binary P6 frames
every ffmpeg build decodes (AC1), rasterized in-crate with an anti-aliased
stroke rasterizer covering the whole `Prim2` vocabulary (AC4, AC5), placing
ink where the SVG sink places it (AC2), byte-deterministic with a checked-in
golden (AC3), proven end to end against stock ffmpeg by a test that skips
rather than fails when ffmpeg is absent (AC6), with zero new dependencies
(AC7).

Nothing in `sink.rs`, `scene.rs`, `prim.rs`, `camera.rs`, or `svg.rs`
changes. `PpmSink` is a peer of `SvgSink` behind the same trait; the render
walk never learns a second sink exists. **This spec reinvents nothing from
SPEC-0003** — trait, walk, filename rule, directory policy, validation
policy, error currency, determinism argument, and golden/bless scheme are
adopted verbatim, and every deviation is called out where it occurs.

## 2. Design

### 2.0 Module and test layout

```
crates/motoreel/src/
  ppm.rs                       — `PpmSink`, the `Canvas`, the rasterizer
crates/motoreel/tests/
  r0006_ppm_sink.rs            — every AC (one file per requirement, house
                                 convention since SPEC-0003)
  golden/frame_00000.ppm       — the checked-in golden fixture (64×36, 6 925 B)
crates/motoreel/examples/first_light/
  main.rs                      — amended: also renders `out/ppm/`; documents
                                 the stock-ffmpeg command (§2.10)
```

`lib.rs` gains `pub mod ppm;` and `pub use ppm::PpmSink;` — the exact shape
`svg`/`SvgSink` already has. Dependencies point inward and gain no edges:
`ppm` → `sink` → `scene` → `track` → garust, plus `ppm` → `prim`. `ppm`
does **not** depend on `svg`; the two sinks agree by sharing a documented
mapping (§2.3), not by sharing code, because the only thing they could
share is four lines of arithmetic whose two spellings — an SVG attribute
and an f64 expression — have no common form.

**One module, not two.** The rasterizer is a private `Canvas` inside
`ppm.rs`, not a `raster.rs` of its own: it has exactly one consumer, and a
module per single *abstraction* — a `Canvas` extracted for one caller — is
the premature abstraction §2 forbids. **The rule is about abstractions, not
line counts.** A module that holds *data* rather than an abstraction — a
`const` table, say — is a different question and is not what this sentence
rejects; it is judged on whether inlining it would bury the logic around it.
The R-0007 promotion path is recorded in §2.12.

**`Cargo.toml` is untouched** (AC7): the rasterizer is `std` arithmetic and
`Vec<u8>`.

### 2.1 `PpmSink` — construction, defaults, directory policy

```rust
/// Writes one `frame_%05d.ppm` per frame into a directory: binary P6,
/// which every ffmpeg build decodes without librsvg or an external
/// rasterizer.
pub struct PpmSink { /* dir, header bytes, canvas */ }

impl PpmSink {
    /// Sink into `dir` (created now, parents included): 1920×1080 pixels
    /// over the default 3.2 × 1.8 centred view window, on black.
    pub fn new(dir: impl AsRef<Path>) -> io::Result<Self>;

    /// Sink into `dir` with an explicit raster size (px) and centred
    /// image-space view window — the pair `SvgSink::with_view` takes,
    /// mapped identically (§2.3).
    pub fn with_view(dir: impl AsRef<Path>, size: (u32, u32), view: (f64, f64))
        -> io::Result<Self>;

    /// Composite alpha against `background` instead of black. PPM has no
    /// alpha channel, so the background is explicit rather than implied
    /// (R-0006 §4).
    pub fn with_background(self, background: Rgb) -> Self;
}
```

- **Defaults are `SvgSink`'s, deliberately identical:** 1920×1080 over
  3.2 × 1.8. AC2 rests on it, and a creator must be able to swap
  `SvgSink::new("out/")` for `PpmSink::new("out/ppm/")` and get the same
  picture. A test asserts the two defaults agree by reading both headers.
- **`with_background` is a consuming builder,** matching the house pattern
  (`Object::with_style` / `with_track`), not a fourth constructor.
  Default `Rgb::BLACK` — SPEC-0003 §4 already records that a transparent
  SVG background composites to black in typical encodes, so black is the
  choice that keeps the two sinks looking the same.
- **Validation, then side effects** — SPEC-0003 §2.4's rule verbatim: zero
  raster dimension, or a non-finite/non-positive view, is
  `ErrorKind::InvalidInput` **before** `create_dir_all`. `PpmSink` adds one
  check SvgSink cannot need, because SvgSink allocates nothing per pixel:
  the frame byte count `w · h · 3`, computed in `u64`, must fit in `usize`
  → otherwise `ErrorKind::InvalidInput` ("frame too large"). The pixel
  buffer is then reserved with `Vec::try_reserve`, mapping failure to
  `ErrorKind::OutOfMemory`. Constitution §6 forbids unchecked failures in
  library code, and a caller-supplied `(100_000, 100_000)` would otherwise
  abort the process inside `Vec`.
- **Directory policy, filenames, overwrite semantics:** SPEC-0003 §2.4
  verbatim — `create_dir_all` at construction only; `format!("frame_{index:05}.ppm")`;
  existing files truncated, never deleted; the stale-higher-numbered-frame
  caveat and "encode from a clean directory" note carry over unchanged.
- **Buffers are allocated once and reused** across frames: the pixel buffer
  (`w·h·3` bytes — 6.2 MB at 1080p), a coverage tile, and a segment
  scratch. Reuse affects capacity only, never bytes (§2.7).

### 2.2 The P6 byte grammar (AC1)

A frame file is exactly two pieces, no padding, no comments, no trailing
byte:

```
P6\n{w} {h}\n255\n            ← the header, ASCII, built once at construction
<w·h·3 bytes>                 ← the body, row-major, top row first, R,G,B
```

- **Header** is `format!("P6\n{w} {h}\n255\n")` frozen into a `Vec<u8>` at
  construction: `w`/`h` are `u32`, so no float formatting exists anywhere in
  this sink — SPEC-0003 §2.5's pinned-`Display` rule has nothing to govern
  here. `P6\n1920 1080\n255\n` is 17 bytes; `P6\n64 36\n255\n` is 13.
- **Body** is the pixel buffer verbatim. `file_len == header.len() + w·h·3`
  is an AC1 assertion for several sizes.
- **Pixel order** is P6's: index `((y · w) + x) · 3`, `y = 0` the **top**
  row, channels R, G, B. The mapping in §2.3 puts image-space +y (up) at
  small `y` (top), so the two conventions meet in exactly one minus sign.
- **Write path — a documented deviation from SPEC-0003 §2.4.** SvgSink
  composes the whole frame into one `String` and issues one `fs::write`.
  Here that would mean copying 6.2 MB per frame into a third buffer for no
  gain, so `frame` does `File::create` + `write_all(header)` +
  `write_all(pixels)`: still one file, one code path, and the same
  guarantee — on failure at most one partial file remains, and no cleanup
  is attempted. No `BufWriter`: the body is a single contiguous slice.

### 2.3 The world→pixel mapping, and why it agrees with `SvgSink` (AC2)

`SvgSink` maps image space to pixels through three composed pieces
(SPEC-0003 §2.5): the static `<g transform="scale(1 -1)">`, the
`viewBox="{-vw/2} {-vh/2} {vw} {vh}"`, and SVG's **default**
`preserveAspectRatio="xMidYMid meet"` against `width="{W}" height="{H}"`.
That third piece is an attribute `SvgSink` never emits, so the default is
what applies — a load-bearing fact SPEC-0003 left unpinned. **§2.13 amends
SPEC-0003 §2.5 to make its absence normative, and AC2 asserts it.**
Composing them for an image-space point `(x, y)`:

1. flip → user coordinates `(x, −y)`;
2. `meet` scales uniformly by `s = min(W/vw, H/vh)` and centres the scaled
   viewBox in the viewport, translating by `tx = (W − s·vw)/2`,
   `ty = (H − s·vh)/2`;
3. viewport `px = tx + s·(x + vw/2)`, `py = ty + s·(−y + vh/2)`.

Since `tx + s·vw/2 = W/2` and `ty + s·vh/2 = H/2`, the whole chain collapses
to two lines, and **this is the normative mapping**:

```
s  = min(W / vw, H / vh)                     px per image unit
px = W/2 + s·x
py = H/2 − s·y
```

- **`min`, not two independent scales.** Taking `W/vw` and `H/vh`
  separately would stretch non-matching aspect ratios; SVG letterboxes
  instead. Reproducing `meet` is what makes AC2 hold for *every*
  `with_view` pair, not only the 16:9 defaults. A test pins the
  letterboxed case (200×100 px over 3.2 × 1.8 → `s = 55.55…`, ink centred,
  bands of background left and right).
- **Pixel centres are at `+0.5`.** SVG's viewport places pixel `(i, j)`
  over `[i, i+1) × [j, j+1)`, so the rasterizer samples at
  `(i + 0.5, j + 0.5)`. This is the only convention under which the two
  sinks agree at the sub-pixel level, and it is what makes the AA
  arithmetic in §2.5 hand-checkable.
- **Stroke width is mapped by the same `s`** — `radius_px = s · width / 2`
  — which is precisely "stroke width honoured in image units exactly as SVG
  honours it" (AC4). At the defaults `s = 600` exactly (`1920/3.2` and
  `1080/1.8` are both exactly `600.0` in f64 — checked), so `Style`'s
  default `width: 0.01` is a 6 px stroke, matching `svg.rs`'s own doc
  comment. Every size this project uses over the default view lands on an
  exact integer `s`: 1920×1080 → 600, 320×180 → 100, 160×90 → 50,
  96×54 → 30, 64×36 → 20.
- **No matrix, no state.** The mapping is a free function
  `to_pixel(p: Pt2, scale: f64, size: (u32, u32)) -> (f64, f64)`, unit-tested
  in `ppm.rs`'s own `#[cfg(test)] mod tests` (the house pattern in `prim.rs`,
  `camera.rs`, `scene.rs`) — so AC2's mapping half is verified *exactly*,
  not through pixels.

### 2.4 The ink model: every primitive is a union of round-capped segments (AC5)

`SvgSink` strokes with `stroke-linecap="round"` and, where joins exist,
`stroke-linejoin="round"`. The region such a stroke covers is exactly the
**Minkowski sum of the primitive's centre-lines with a disc of radius
`width/2`** — round caps are that disc at the ends, round joins are that
disc at the corners. So the rasterizer needs one geometric idea:

> the ink of a primitive is the set of points within `r` of *any* of its
> segments.

The whole `Prim2` vocabulary flattens into segment pairs by one exhaustive
match, with no per-variant geometry:

| `Prim2` | segments | notes |
|---|---|---|
| `Point { at }` | one **degenerate** pair `(at, at)` | distance-to-segment degenerates to distance-to-point → a disc of radius `width/2`, which is exactly SvgSink's `<circle r="width/2">` |
| `Segment { a, b }` | `(a, b)` | |
| `Polyline { points }` | `points.windows(2)` | joins are the union at shared vertices; `< 2` points yields **zero** segments and therefore no ink — the degenerate case falls out of `windows(2)` with no branch, matching `points=""` rendering nothing |
| `Edges { segments }` | the pairs verbatim | an empty list yields no ink, matching SPEC-0004's `d=""` |

Consequences worth stating:

- **Caps and joins need no special code.** They are the same disc the
  interior is made of. This is why the distance formulation is chosen over
  a scanline/edge-list polygon rasterizer, which would need cap geometry,
  join geometry, and a fill rule, each an opportunity to disagree with SVG.
- **`Point` and a degenerate `Segment` are byte-identical output.** An AC5
  test asserts exactly that, which is the cheapest possible proof that the
  unification is real and not decorative.
- **The match is exhaustive with no `_` arm** — deliberately, so R-0007's
  new variant is a compile error rather than a silently unrendered label
  (§2.12).
- **Whole-primitive culling is not this sink's job.** `Scene::eval` culls
  (SPEC-0002 §2.5); a culled primitive never reaches `frame`. The sink
  inherits R-0002's behaviour by not having any of its own — the AC5 test
  drives a behind-the-camera scene and asserts a pure-background frame.

### 2.5 The rasterizer: coverage, union, compositing (AC4)

Per primitive, in three phases. Every loop is an integer pixel loop over
half-open bounds; nothing is carried from one pixel to the next.

**Phase 1 — bounds.** Map every segment endpoint to pixel space. Skip any
segment with a non-finite endpoint (§2.7). Take the axis-aligned bounding
box of the surviving endpoints, expand by `pad = r + 0.5` (the ramp's
reach), and clamp to the canvas:

```
x0 = clamp(floor(min_x − pad), 0, W)      x1 = clamp(ceil(max_x + pad), 0, W)
y0 = clamp(floor(min_y − pad), 0, H)      y1 = clamp(ceil(max_y + pad), 0, H)
```

An empty rectangle returns immediately. This is the whole reason the
non-finite filter is load-bearing rather than defensive: `NaN.floor() as u32`
is a saturating 0 and would silently produce a nonsense tile.

**Phase 2 — coverage, unioned by `max`.** A scratch `Vec<f64>` sized to the
tile is filled with `0.0`, then for each segment, over *that segment's own*
padded tile (intersected with the primitive's), for each pixel:

```
d   = distance_to_segment((x + 0.5, y + 0.5), a, b)     // pixels
cov = clamp(r + 0.5 − d, 0, 1)                          // the pinned rule
tile[..] = max(tile[..], cov)                           // union
```

- **The coverage rule (pinned):** a one-pixel-wide linear ramp centred on
  the true boundary — `1` at `d ≤ r − 0.5`, `0.5` exactly on the boundary
  `d = r`, `0` at `d ≥ r + 0.5`. It is the standard analytic approximation
  to the box-filtered area of a half-plane, and for a stroke whose radius
  of curvature is ≥ 1 px it is accurate to a few percent. It is evaluated
  **once per pixel** — there is no sampling, no supersampling grid, and
  therefore no accumulation order to pin.
- **Union by `max`, not by per-segment compositing.** Compositing each
  segment separately would double-composite the overlap at every polyline
  joint, drawing a visible dark seam at exactly the places SVG's round join
  makes seamless. `max` is the correct union operator for coverage,
  is order-independent on finite floats, and adds no accumulated error.
  (It slightly *under*-estimates coverage where two centre-lines cross
  within one pixel — the standard, accepted artefact; the alternative,
  `a + b − a·b`, over-estimates and is order-dependent in floating point.
  `max` is the pinned choice.)
- **Per-segment tiles, not one whole-primitive sweep.** Cost is proportional
  to ink, not to the primitive's bounding box: a 12-edge wireframe box
  spanning 1000×800 px costs ~12 × (edge length × stroke width) pixels
  instead of 12 × 800 000.
- **A tile is cleared per primitive, not per frame** — a `clear()` +
  `resize(area, 0.0)`, which is a memset and leaves no dependence on the
  previous primitive's values.

**Phase 3 — one src-over pass over the tile.** For each pixel with
`cov > 0`:

```
a         = cov · clamp(style.alpha, 0, 1)
out_ch    = src_ch · a + dst_ch · (1 − a)        // per channel, f64
dst_ch    = round(out_ch) as u8                  // one rounding, at the end
```

- **Exactly one composite per pixel per primitive**, in draw order = slice
  order = scene insertion order — the same painter's order SVG's document
  order gives. The sink never sorts, groups, or dedups (SPEC-0003 §2.5).
- **The framebuffer is `u8`,** so each layer quantizes. That is a deliberate
  choice: it makes the buffer *identical to the file body* (no conversion
  pass, no 50 MB f64 shadow buffer), and quantization is a pure function of
  the pinned rounding, so it costs nothing in determinism. It does cost a
  little in colour accuracy under many stacked translucent layers — stated
  in §2.8 as explicitly outside AC2's claim.
- **Rounding is `f64::round`** — ties away from zero. Both operands are
  small exact integers in f64 and `a ∈ [0, 1]`, so `out ∈ [0, 255]` and the
  `as u8` cast never saturates in practice; the cast is nonetheless
  saturating-by-language, so the path is total.
- **Guards, each SVG-matching rather than defensive:**
  `radius.is_finite() && radius > 0.0` must hold or the primitive paints
  nothing — SVG draws nothing for `stroke-width="0"`, and without the guard
  the ramp would paint a phantom 50 %-covered hairline for a zero-width
  stroke. The finiteness half is not decoration: an *infinite* radius passes
  `> 0.0`, makes `pad` infinite, and makes the tile the whole canvas.
  `alpha == 0.0` returns early; it is observationally identical
  (`out = dst` exactly) and saves a full tile sweep.
  A NaN `width` or `alpha` paints nothing, via the total `unit` helper
  (`Style`'s contracts are documented but *not validated* — `prim.rs` says
  so — and a NaN must not poison a frame). Both guards are spelled a
  particular way for a reason no lint can see; §3 pins the spelling.

**Worked numbers (hand-checkable, and the basis of the AC4 tests).** Black
background, white stroke, `alpha = 1`, `r = 1` px:

| centre-line at | rows | `d` (px) | coverage | byte |
|---|---|---|---|---|
| `y = 18.0` (pixel boundary) | 17, 18 | `0.5` | `1.0` | 255 |
| | 16, 19 | `1.5` | `0.0` | 0 (untouched) |
| `y = 18.5` (pixel centre) | 18 | `0.0` | `1.0` | 255 |
| | 17, 19 | `1.0` | `0.5` | `round(127.5)` = **128** |

At `r = 2` px with the centre-line on the boundary `y = 18.0`, rows 16–19
are all exactly `1.0` and rows 15 and 20 exactly `0.0` — four full rows, no
fringe. These are exact f64 identities, not tolerances.

**The lit extent is `2r + 1` px; the summed coverage is `2r`.** Both follow
from the ramp, and neither is "the stroke is `2r` px wide". The ramp reaches
`r + 0.5` either side of the centre-line, so a cross-section touches
`2r + 1` px of pixels: `r = 1` lights 3 px, `r = 2` lights **5**, not 6 —
doubling the radius does not double the lit extent, and a test written
against doubling fails on a correct rasterizer. What *is* exactly
proportional to the width is the **ink**: the cross-section coverages sum to
exactly `2r = s · width`, at every sub-pixel offset, for every `r ≥ 0.5`.
§2.8 states that identity, proves it, and documents the sub-pixel regime
where it fails; it is the form AC4 asserts.

### 2.6 Colour, alpha and the background

- **Colour is 8-bit sRGB, verbatim from `Style`** (R-0006 §4). No
  linearization, no gamma correction, no ICC. Compositing therefore happens
  in sRGB space, which is also what SVG does by default
  (`color-interpolation: sRGB`), so this is agreement, not laziness.
- **Alpha is composited at write time against an explicit background**
  (R-0006 §4 and its decision log): PPM has no alpha channel, and
  flattening silently onto black would hide the choice. `Rgb::BLACK` is the
  default because it reproduces what an encoded transparent SVG looks like.
- **Frame reset:** every frame begins by filling the pixel buffer with the
  background triple (`chunks_exact_mut(3)`, `copy_from_slice`). No frame
  inherits a pixel from its predecessor — a property AC3's two-render test
  would catch immediately if it broke.

### 2.7 Determinism analysis — what "byte-for-byte" rests on (AC3)

SPEC-0003 §2.6's argument, extended through the raster stage:

1. **Upstream bits.** R-0001 AC7 / R-0002 AC6 guarantee identical `Prim2`
   bits for identical scene and `t`; the render walk (SPEC-0003 §2.3) is
   unchanged and derives `t_i = i / fps` rather than accumulating.
2. **Fixed iteration order.** Primitives in slice order; segments in
   primitive order; pixels row-major (`y` outer, `x` inner) over integer
   half-open bounds obtained by `floor`/`ceil` on f64 and clamped to the
   canvas. No parallelism, no work stealing, no `HashMap`.
3. **No accumulation across pixels.** Each pixel's coverage is a closed-form
   function of its own centre and the segment endpoints. There is no DDA,
   no Bresenham error term, no scanline edge list, no incremental
   `x += dx` — the classes of algorithm whose output depends on where the
   walk started. Union is `max`; compositing is one src-over per primitive.
4. **No transcendental enters the raster path.** The rasterizer uses only
   `+ − × ÷`, `sqrt`, `floor`, `ceil`, `round`, comparisons, and `min`/`max`.
   Every one of these is an IEEE-754 *correctly rounded* operation with a
   uniquely defined result — including `sqrt`, whose exactness is mandated,
   and `round`, which is exact by construction. No `sin`/`cos`/`exp`/`powf`
   /`hypot` is called, and `f64::mul_add` is **forbidden** in this module
   (FMA rounds differently from multiply-then-add; both are deterministic,
   but mixing them would silently move golden bytes). Consequently the
   *raster stage itself is bit-portable across platforms* — a stronger
   position than SPEC-0003's text stage needed, and the reason the golden
   fixture can be trusted on any CI host. **With one clause, stated rather
   than assumed:** this holds on targets whose `f64` arithmetic *is* IEEE
   double. x87-only targets (`i586-*` and friends, no SSE2) historically
   compute in 80-bit registers and double-round when a value spills, which
   can move a last-ulp result and therefore a golden byte. Every target this
   project builds for — `x86_64` (SSE2 by baseline) and `aarch64` — is IEEE
   double, so the claim stands as written; an x87 host is out of scope, and
   the golden test is exactly where it would announce itself.
5. **The platform caveat is unchanged and unhidden.** It lives in the
   *scene*, not the sink: a trig-bearing rollout may differ in the last ulp
   across libm versions (project-specifics; SPEC-0003 §2.6). So AC3's
   golden scene is **transcendental-free by construction**, exactly as
   SPEC-0003's is — single-key tracks, `Motor3::identity`/`translator`
   poses only, `Camera::default()` orthographic, every vertex authored in
   the `z = 0` plane at view depth 5. Two-render bit-identity (also AC3) is
   claimed same-process/same-machine, where trig is fine.
6. **Hygiene.** No clock, environment, randomness, thread, or map iteration
   touches the write path. Buffer reuse affects capacity only: the pixel
   buffer is fully overwritten by the background fill each frame, and the
   coverage tile is `resize`d to `0.0` per primitive.

### 2.8 The AC2 tolerance and the AC4 width identity, stated precisely

R-0006 AC2 says "within the rasterizer's documented tolerance"; AC4 says
stroke width is honoured "by an **exact identity**". This is the
documentation for both. Every claim below is labelled **exact** or
**bounded**, and every bounded one carries the precondition that makes it
true — conflating those is what makes such statements rot.

**Exact — the mapping.** For every image-space coordinate, `PpmSink`'s
pixel-space position is `(W/2 + s·x, H/2 − s·y)` with `s = min(W/vw, H/vh)`,
which is the closed form of the transform chain `SvgSink` emits (§2.3
derives it; §2.13 pins the `preserveAspectRatio` default it rests on). The
mapping is verified by bit-exact assertions on `to_pixel`, not by tolerance.
**The two sinks do not "approximately agree" on where things are; they agree
exactly, and only the pixel filter differs.**

**Exact — the ink region.** Both sinks describe the same point set: the
Minkowski sum of the primitive's centre-lines with a disc of radius
`s · width / 2` (§2.4). Round caps and round joins are not an approximation
of SVG's; they are the same construction.

**Exact — the width identity (AC4), for `r ≥ 0.5` px.** For a straight
stroke of radius `r = s · width / 2`, the coverages across its cross-section
sum to exactly `s · width`, **bit-exactly, at every sub-pixel offset**:

> `Σ_j coverage(|j + 0.5 − y₀|, r) == 2r == s · width`, for a horizontal
> centre-line at any `y₀ ∈ ℝ`, and correspondingly for a vertical one.

Why it is exact and not merely close: as a function of the signed offset,
`coverage(d, r) = clamp(r + 0.5 − d, 0, 1)` is the convolution of a box of
width `2r` — the stroke's true cross-section — with a box of width `1`, the
pixel. A unit-spaced sample sum of anything convolved with a unit box equals
that thing's integral, because the unit box's Fourier transform vanishes at
every non-zero integer frequency; and the integral of a box of width `2r` is
`2r`. The `clamp` at `1` is what makes that plateau real, which is precisely
why the identity needs `r ≥ 0.5`. Verified numerically over 1000 sub-pixel
offsets at `r ∈ {0.5, 0.75, 1, 1.5, 2, 3, 6}` px: `max |Σcov − 2r| = 0`.

**This — not "doubling the width doubles the lit width" — is the true
statement of "width is honoured in image units".** The *lit extent* is
`2r + 1` px, so `r: 1 → 2` takes a cross-section from 3 px to 5 px, not 3 to
6. AC4 asserts the identity and the `2r + 1` extent; it must not assert
proportional extent, which is false (§2.5, §6 AC4).

**Bounded — where the ink lands on the grid: ±1 px, for `r ≥ 1` px.**

> **Precondition: `r = s · width / 2 ≥ 1` px.** For every primitive vertex
> mapping to pixel-space `(u, v)`, the frame then contains a pixel whose
> coverage is ≥ 0.5 within a Chebyshev distance of **1 pixel** of
> `(floor(u), floor(v))`; and no pixel further than `r + 1` px from any
> centre-line differs from the background.

**Why the precondition exists, in numbers.** Coverage is evaluated only at
pixel centres, so the worst case for a vertex is one sitting exactly on a
pixel corner: the nearest centre is then `√2/2 ≈ 0.7071` px away, and the
best coverage available *anywhere* in the surrounding 3×3 block is
`clamp(r + 0.5 − 0.7071, 0, 1)` — which is `0.2929` at `r = 0.5`, `0.0429`
at `r = 0.25`, and `0.0` at `r = 0.1`. A ≥ 0.5 pixel therefore requires
`r ≥ √2/2` for a **vertex** (a lone endpoint, or a `Point`), and `r ≥ 0.5`
for a **segment interior** (a straight line's worst-case distance to the
nearest pixel centre is 0.5, attained by an axis-aligned line on a pixel
boundary, not `√2/2`). `r ≥ 1` px is the one memorable threshold that covers
both with room to spare, and it is what R-0006 AC2 now words.

That regime is not hypothetical: `Style::default().width = 0.01` at the
golden's `s = 20` is `r = 0.1` px — a default stroke, lighting *no* pixel to
0.5 anywhere. It is exactly why the golden fixture is deliberately
fat-stroked (§3: `r ∈ [1.2, 3.0]` px) and why a 32×18 fixture was rejected:
at `s = 10` the same widths give `r ∈ [0.6, 1.5]`, which puts the polyline
(`0.8`) and the edges (`0.6`) below the threshold and the segment exactly on
it — a fixture asserted outside the regime it demonstrates (§5).

Why exactly ±1 px above the threshold and not tighter: the 50 %-coverage
contour *is* the geometric boundary, but it is observed only at pixel
centres, which quantizes by up to half a pixel, and the ramp itself spans a
further half pixel. ±1 px is the honest sum. Why not looser: at the default
`s = 600` px/unit, 1 px is 1/600 of an image unit — far below what any AC2
test could confuse with a real placement bug (a sign error, a missing
y-flip, or a stretched aspect ratio all move ink by tens of pixels).

**Explicitly not claimed.**

- *Nothing about sub-pixel strokes (`r < 1` px).* Two distinct failures live
  there, both measured, both scoped out rather than papered over.
  **Placement:** the ±1 px guarantee is simply false below `r ≈ 0.71` — a
  vertex on a pixel corner peaks at `0.2929` coverage at `r = 0.5`, and at
  `r = 0.1` lights nothing at all. **Ink:** below `r = 0.5` the ramp is a
  cone with no plateau, its integral is `(r + 0.5)²` rather than `2r`, and
  the rasterizer therefore **over-inks** a hairline — ~3× on average at
  `r = 0.05` and up to **5.5× at the worst offset** (a single sample of
  `0.55` where `2r = 0.1`). A sub-pixel stroke is drawn darker and wider than
  SVG draws it. This is the standard behaviour of an unweighted
  centre-sampled coverage rasterizer; the fixes (a conservative
  minimum-width term, or true area sampling) would move every golden byte and
  are not R-0006's business. A creator wanting a thin line at a small raster
  size raises the width or the size — §2.10's disk-cost note is the same
  trade-off from the other end.
- *No pixel-level agreement with any SVG renderer.* Two SVG renderers do
  not agree with each other at stroke edges; a coverage rasterizer agrees
  with neither. AC2 is verified against the *mapped coordinates the SVG
  sink writes*, parsed out of the pinned template — never against a
  rasterized SVG. No renderer is invoked, and no dependency is added.
- *No colour agreement.* Anti-aliased edge values and the result of
  stacking translucent layers depend on filter and framebuffer precision.
  §2.5's `u8` framebuffer quantizes per layer. AC2 is a statement about
  geometry.
- *No claim for degenerate polylines.* A `Polyline` of fewer than two
  points draws nothing here (§2.4); SVG renderers disagree among themselves
  about a lone `moveto`, so there is nothing to agree *with*. Pinned and
  documented rather than papered over.

### 2.9 Error handling summary

Mirrors SPEC-0003 §2.9, with the two rows only a raster sink can have:

| Site | Failure | Behaviour |
|------|---------|-----------|
| `PpmSink::new` / `with_view` | zero size / bad view | `ErrorKind::InvalidInput`, before dir creation (no side effects) |
| `PpmSink::new` / `with_view` | `w·h·3` exceeds `usize` | `ErrorKind::InvalidInput`, "frame too large" |
| `PpmSink::new` / `with_view` | pixel buffer allocation fails | `ErrorKind::OutOfMemory` via `Vec::try_reserve` — never an abort (§6) |
| `PpmSink::new` / `with_view` | dir creation fails | `io::Error` propagates (fail before rendering) |
| `PpmSink::frame` | `File::create` / `write_all` fails | `io::Error` propagates; ≤ 1 partial file, no cleanup |
| `PpmSink::frame` | style contract violated (width NaN, infinite or ≤ 0; alpha NaN) | that primitive paints nothing; frame stays valid and deterministic (§2.5's guard, §3's spelling) |
| `PpmSink::frame` | non-finite coordinate (contract-breaking) | `debug_assert` tripwire; in release that segment is skipped so tile bounds stay sane |
| `Scene::render` | bad fps / bad duration / sink error | unchanged — SPEC-0003 §2.9 |

No panics on any library path. The single `debug_assert` mirrors `svg.rs`'s
and asserts the same SPEC-0002 §2.5 invariant. The release-mode skip beside
it is **not** dead code: it is what keeps `floor`/`ceil` → `u32` from
saturating to a nonsense tile on a boundary that is documented but
unvalidated.

### 2.10 The demo and the documented ffmpeg command (AC6)

**The demo.** `examples/first_light/main.rs` gains a second sink over the
*same* scene — the shipped demo becomes the proof that the pipeline works:

```rust
scene::scene().render(60.0, &mut SvgSink::new("out/")?)?;
scene::scene().render(60.0, &mut PpmSink::new("out/ppm/")?)?;
```

`scene.rs` is untouched, so `tests/r0003_svg_sink.rs`'s `#[path]` include and
its AC4 determinism test are unaffected. The SVG output path stays `out/`
because R-0003's QA-signed AC5 test asserts the literal string
`ffmpeg -framerate 60 -i out/frame_%05d.svg`; moving it would break a
signed-off requirement to tidy a directory. `examples/tumbling_box/` is
deliberately untouched — one demo satisfies AC6.

**The documented command (exact, normative).** Recorded in `main.rs`'s doc
header and here:

```bash
cargo run --example first_light
ffmpeg -framerate 60 -i out/ppm/frame_%05d.ppm -c:v libx264 -pix_fmt yuv420p first_light.mp4
```

- `-c:v libx264` is named rather than left to the muxer default, because
  AC6 claims *h264* and a named encoder makes the claim checkable and its
  absence legible.
- `-pix_fmt yuv420p` is what makes the result playable everywhere; it
  requires **even** width and height. `PpmSink` does not enforce evenness —
  odd sizes are valid PPM — but the demo (1920×1080), the golden (64×36)
  and the AC6 test (320×180) are all even, and the example header says so.
- **Disk cost, stated plainly in the header:** PPM is uncompressed, so 240
  frames at 1080p is ≈ 1.5 GB in `out/ppm/`, deleted after the encode.
  `out/` is already git-ignored. A creator wanting less passes
  `PpmSink::with_view` a smaller size.

**The AC6 test, and how it skips.** In `tests/r0006_ppm_sink.rs`:

1. **Probe the binary:** run `which ffmpeg`. On a non-success exit, or if
   `which` itself cannot be spawned, print
   `SKIP: ffmpeg not on PATH — AC6 end-to-end encode not exercised` and
   `return`. The note names what was missing so a skipped CI run is
   legible.
2. **Probe the encoder:** run `ffmpeg -hide_banner -encoders` and require
   `libx264` in the output. If absent, print
   `SKIP: this ffmpeg has no libx264 encoder` and `return`. A creator's
   ffmpeg packaging is not a motoreel regression.
3. **Otherwise assert.** Render a small sequence into a **freshly cleaned**
   directory under `CARGO_TARGET_TMPDIR` (`remove_dir_all`, ignoring
   `NotFound`, before constructing the sink) — 320×180, `duration 0.25` at
   60 fps → exactly **15** frames (0.25 is dyadic, so `0.25 · 60 = 15.0`
   exactly and the frame count is not a rounding accident). The clean is
   load-bearing, not tidiness: §2.1's directory policy truncates but never
   deletes, so a stale higher-numbered frame from an earlier run would
   silently join the encode and make the second run of the test differ from
   the first. Then run the documented command shape against them and assert
   the exit status is success, the mp4 is non-empty, and **its bytes `4..8`
   are `b"ftyp"`** — the ISO-BMFF box type every mp4 begins with, four
   bytes that catch an ffmpeg exiting 0 having written garbage (§5). **A
   present ffmpeg that refuses our frames is a real failure and fails the
   test:** that is the whole point of R-0006.

   **The test's invocation adds `-y -nostdin`; the documented creator
   command deliberately does not.** ffmpeg prompts `File ... already exists.
   Overwrite?` when the output is present and reads the answer from stdin,
   which under `cargo test` is not a tty and may be closed or shared — so
   without the flags the encode can hang, or fail for a reason that has
   nothing to do with our frames. Both flags are test-harness hygiene rather
   than part of the claim: a creator running the command by hand *wants* the
   prompt, and inserting the flags into the documented string would also
   break AC6's literal-substring assertion against `main.rs`.

Both probes can only ever cause a *skip*, never a false pass and never a
false failure — which is the argument for `which` over a more portable
capability check. On a host without `which` (Windows), the test skips; the
promotion path, if Windows CI ever matters, is to spawn `ffmpeg -version`
directly and treat `ErrorKind::NotFound` as absent (decision log).

Nothing else in the suite shells out, and `cargo test` still compiles the
examples without rendering them (SPEC-0003 §2.7).

### 2.11 Zero dependencies (AC7)

`crates/motoreel/Cargo.toml` is not edited. The AC7 test reuses the
`section_keys` assertion already written for R-0003 AC5: `[dependencies]`
stays exactly `["garust"]`, `[dev-dependencies]` exactly `["proptest"]`, no
`[build-dependencies]`, no target-specific `.dependencies]` table. The
rasterizer is ~150 lines of `std` arithmetic.

**One repo-hygiene item this spec does add:** a `.gitattributes` covering
both golden fixtures, because an `eol`/`autocrlf` setting would silently
corrupt a byte-exact file — a failure that would look like a rasterizer
bug on someone else's clone.

```
crates/motoreel/tests/golden/*.ppm binary
crates/motoreel/tests/golden/*.svg -text
```

**Why two rules rather than one `** binary`.** `binary` implies both
`-text` and `-diff`. The `-text` half is what protects byte-exactness and
both fixtures need it. The `-diff` half suppresses textual diffs, which
is right for the PPM (a binary blob, unreviewable by eye — hence the
failure reporter in §2.11) and *wrong* for the SVG: SPEC-0003 §1 chose
SVG first precisely because its frames are "pure text, diffable in
review", and marking it `binary` would take that back to buy nothing.

Without it, a future `core.autocrlf` or an eol setting could rewrite the
`\n` bytes in the P6 header — or worse, bytes in the body that happen to be
`0x0D 0x0A` — and silently corrupt a byte-exact fixture. This is not
optional polish; it is the checked-in golden's integrity.

**The pattern is the whole directory, not `*.ppm`.** R-0003's
`tests/golden/frame_00000.svg` is a byte-exact fixture too, and it is
unprotected today — a *text* fixture is more exposed to an eol rewrite than
a binary one, not less, and R-0003's golden bytes are asserted with the same
`==` this spec's are. One glob covers both, covers R-0007's text golden and
font specimen before they exist, and cannot be forgotten by whoever adds the
next fixture. The cost, stated: `binary` implies `-diff`, so `git diff`
reports the SVG golden as "Binary files differ" instead of a text diff.
Accepted — a golden is reviewed through its bless regeneration and the
test's own reporter (§6 AC3), never by eyeballing a diff hunk; and this
changes no byte of any fixture, so R-0003's AC3 test must still pass
untouched, which is the cheapest proof the entry is inert.

### 2.12 R-0007 touchpoints — room left, nothing designed

R-0007 adds anchored labels: a `Prim2::Text`-like record that `PpmSink`
must rasterize from an embedded bitmap font (R-0007 AC6).

**Sequencing, normatively: R-0006 must be `Met` before R-0007
implementation begins.** SPEC-0007 does not merely follow this spec, it
*consumes* it — `to_pixel`, `src_over`, `unit`, `Tile` and `Canvas` are all
named in its dependency and module lists as things it calls or amends.
Starting R-0007 against an unsigned-off rasterizer means either duplicating
those five items or absorbing a correction to them mid-requirement, and
every §2.8 claim R-0007 inherits would be inherited from an unverified
source. The gate is the ordinary one — R-0006 QA sign-off and merge, then
R-0007 step 5 (constitution §4) — and it is written here because this is the
file R-0007's author reads.

What this design deliberately leaves in place for it, **without designing
any of it**:

- **The compositing half is already general.** `src_over` and the
  coverage → alpha rule take a coverage value, not a geometry: a glyph
  contributes coverage exactly as a stroke does, so a bitmap mask blitted
  at an integer offset reuses §2.5 phase 3 unchanged, including the pinned
  rounding and the `u8` framebuffer. R-0007 needs a new *coverage source*,
  not a new compositor.
- **The coverage half is not general, and that is honest.** Glyphs are not
  unions of round-capped segments, so `push_segments` will not stretch to
  cover them. Expect a second entry point on `Canvas` beside `draw`.
- **The mapping is already the anchor arithmetic labels need.** A world- or
  pose-anchored label arrives as a `Pt2` from `Scene::eval` (R-0007 AC2/AC3
  keep projection upstream), and R-0007's "offset in image units" converts
  with the same `scale`: `offset_px = scale · offset_image`. No new mapping
  concept, and the screen-corner anchor is a `Pt2` like any other.
- **The variant dispatch fails loudly.** `push_segments` matches `Prim2`
  exhaustively with no `_` arm, so adding a variant is a compile error in
  `ppm.rs`. A label that silently fails to render is the failure mode this
  is chosen to prevent.
- **Determinism extends without a new argument.** A bitmap face is a
  constant byte table; glyph coverage is a table lookup, so §2.7's
  "no transcendental, no accumulation" claim survives intact and R-0007's
  AC7 inherits it rather than re-deriving it.
- **The fixture scheme extends.** A text golden is one more small PPM under
  `tests/golden/`, covered by the same bless command, the same byte-diff
  reporter (§6 AC3), and the same `.gitattributes` glob — §2.11 marks the
  whole directory, so a new fixture needs no new line and cannot be
  forgotten.
- **The module split is the pressure valve.** If R-0007's glyph code makes
  `ppm.rs` unwieldy, extracting `raster.rs` (the `Canvas`, `src_over`,
  `coverage`, `distance_to_segment`) is a mechanical, additive move with no
  public-API change — §2.0 chose one module because there is one consumer
  *today*, not as a position to defend.

Not designed here, and left entirely to SPEC-0007: the glyph format and
font table, baseline/advance/alignment rules, the non-ASCII fallback
(R-0007 AC8), how `Prim2` gains the variant, and the documented fact that
the two sinks are *not* pixel-identical for text (R-0007 §4).

### 2.13 Amending SPEC-0003 §2.5 — the absent `preserveAspectRatio` is normative

§2.3's mapping is composed from three pieces of `SvgSink`'s output, and only
two of them are written down in SPEC-0003 §2.5's pinned file shape: the
`<g transform="scale(1 -1)">` line and the `viewBox` attribute. The third is
written down nowhere, because it is an attribute `SvgSink` **does not emit** —
`preserveAspectRatio`, whose SVG default is `xMidYMid meet`. The whole
`s = min(W/vw, H/vh)` derivation, and with it AC2 for every non-16:9
`with_view` pair, rests on that default.

SPEC-0003 never pinned it, and nothing it defines could notice its loss: a
future edit adding `preserveAspectRatio="none"` to the header would leave the
SVG well-formed, the golden would be re-blessed as a matter of course, and the
two sinks would silently stop agreeing — the SVG stretching where `PpmSink`
letterboxes. An invariant no test can see is not an invariant. This spec
closes the hole in the shape SPEC-0007 §2.6 uses to amend the same section,
which SPEC-0004 §2.3 established.

**The amendment (normative).** SPEC-0003 §2.5's "File shape (exact bytes, in
order)" gains this clause:

> The `<svg>` element carries exactly the attributes shown — `xmlns`,
> `width`, `height`, `viewBox` — and **no `preserveAspectRatio` attribute**.
> Its absence is normative rather than incidental: the default
> `xMidYMid meet` is the uniform-scale-and-centre rule that `PpmSink`
> reproduces as `s = min(W/vw, H/vh)` (SPEC-0006 §2.3), so emitting the
> attribute with *any* value — including a redundant `"xMidYMid meet"` —
> changes the byte shape of every frame and moves R-0003's golden. Changing
> the letterbox behaviour means amending this clause and SPEC-0006 §2.3
> together, never one of them.

**What changes concretely: nothing.** Not a line of `svg.rs`, not a byte of
R-0003's golden fixture. The amendment records a property the code already
has and adds the test that keeps it — the cheapest kind of amendment, and the
same "additive, byte-unchanged" argument SPEC-0004 §2.3 made for `Edges` and
SPEC-0007 §2.7 makes for `<text>`.

**What asserts it:** R-0006 AC2(e) (§6), by reading the header `SvgSink`
writes and asserting the substring `preserveAspectRatio` does **not** occur.
The assertion lives with R-0006 rather than being retrofitted into R-0003's
suite because R-0006 is what depends on it: a test should fail in the
requirement whose claim it breaks, and R-0003's own tests stay untouched
(§4).

## 3. Code outline

```rust
// ppm.rs — the sink (representative, not final)

/// Writes one `frame_%05d.ppm` per frame into a directory: binary P6,
/// `P6\n{w} {h}\n255\n` then exactly `w·h·3` bytes — decoded by every
/// ffmpeg build, with no librsvg and no external rasterizer.
pub struct PpmSink {
    dir: PathBuf,
    header: Vec<u8>,   // frozen at construction; ASCII, no float formatting
    canvas: Canvas,
}

impl FrameSink for PpmSink {
    fn frame(&mut self, index: usize, prims: &[Prim2]) -> io::Result<()> {
        self.canvas.clear();
        for prim in prims {
            self.canvas.draw(prim); // draw order = slice order (painter's)
        }
        let mut file = fs::File::create(self.dir.join(format!("frame_{index:05}.ppm")))?;
        file.write_all(&self.header)?;
        file.write_all(self.canvas.pixels())
    }
}
```

```rust
// ppm.rs — the rasterizer (representative, not final)

/// Pixel canvas: the pinned mapping plus the coverage rasterizer.
struct Canvas {
    size: (u32, u32),
    scale: f64,               // px per image unit — `min(W/vw, H/vh)` (§2.3)
    background: Rgb,
    pixels: Vec<u8>,          // row-major RGB, top row first: exactly w·h·3
    coverage: Vec<f64>,       // scratch tile for the primitive in flight
    segments: Vec<(Px, Px)>,  // scratch, pixel space (capacity caches only)
}

type Px = (f64, f64);

/// Image space (y-up, centred) → pixel space (y-down, centres at `+0.5`).
/// The closed form of `SvgSink`'s `scale(1 -1)` ∘ viewBox ∘ `xMidYMid meet`
/// chain — §2.3 derives it; the two sinks agree because of this function.
fn to_pixel(p: Pt2, scale: f64, size: (u32, u32)) -> Px {
    (
        f64::from(size.0) / 2.0 + scale * p.x,
        f64::from(size.1) / 2.0 - scale * p.y,
    )
}

/// Every primitive is a union of round-capped segments (§2.4). Exhaustive
/// with no `_` arm on purpose: R-0007's new variant must be a compile
/// error, never a silently unrendered label.
fn push_segments(prim: &Prim2, map: impl Fn(Pt2) -> Px, out: &mut Vec<(Px, Px)>) {
    match prim {
        Prim2::Point { at, .. } => out.push((map(*at), map(*at))),
        Prim2::Segment { a, b, .. } => out.push((map(*a), map(*b))),
        // `< 2` points yields no segments — the degenerate case needs no branch
        Prim2::Polyline { points, .. } => {
            out.extend(points.windows(2).map(|w| (map(w[0]), map(w[1]))));
        }
        Prim2::Edges { segments, .. } => {
            out.extend(segments.iter().map(|(a, b)| (map(*a), map(*b))));
        }
    }
}

impl Canvas {
    /// Rasterize one primitive: union coverage of its round-capped
    /// segments, composited src-over in a single pass (§2.5).
    fn draw(&mut self, prim: &Prim2) {
        let style = style_of(prim);
        let radius = 0.5 * self.scale * style.width;
        let alpha = unit(style.alpha);
        // `stroke-width="0"` paints nothing in SVG, and must here too:
        // without this the ramp would paint a phantom 50 % hairline. The
        // `is_finite()` half rejects NaN *and* an infinite radius, which
        // would make `pad` infinite and the tile the whole canvas. Spelled
        // as a negated conjunction, not `!(radius > 0.0)` — see the clippy
        // note under this block (§2.5, §2.9).
        if !(radius.is_finite() && radius > 0.0) || alpha == 0.0 {
            return;
        }

        let (scale, size) = (self.scale, self.size);
        self.segments.clear();
        push_segments(prim, |p| to_pixel(p, scale, size), &mut self.segments);
        // The finite contract is SPEC-0002's; the debug_assert is the
        // tripwire, the retain keeps `floor`/`ceil` → u32 honest (§2.9).
        debug_assert!(self.segments.iter().all(|&(a, b)| finite(a) && finite(b)));
        self.segments.retain(|&(a, b)| finite(a) && finite(b));

        let pad = radius + 0.5; // the AA ramp's reach beyond the boundary
        let Some(tile) = Tile::around(&self.segments, pad, size) else {
            return;
        };
        self.coverage.clear();
        self.coverage.resize(tile.area(), 0.0);

        for &(a, b) in &self.segments {
            let Some(band) = tile.intersect(Tile::around(&[(a, b)], pad, size)) else {
                continue;
            };
            for y in band.y0..band.y1 {
                for x in band.x0..band.x1 {
                    let centre = (f64::from(x) + 0.5, f64::from(y) + 0.5);
                    let c = coverage(distance_to_segment(centre, a, b), radius);
                    let slot = &mut self.coverage[tile.offset(x, y)];
                    if c > *slot {
                        *slot = c; // union by max — no accumulation (§2.5)
                    }
                }
            }
        }

        for y in tile.y0..tile.y1 {
            for x in tile.x0..tile.x1 {
                let c = self.coverage[tile.offset(x, y)];
                if c > 0.0 {
                    let i = (y as usize * size.0 as usize + x as usize) * 3;
                    src_over(&mut self.pixels[i..i + 3], style.stroke, c * alpha);
                }
            }
        }
    }
}

/// Distance in pixels from `p` to segment `a`–`b`. A degenerate segment
/// (`a == b`) gives the distance to the point — which is what makes a
/// `Point` a disc and a round cap a cap (§2.4).
fn distance_to_segment(p: Px, a: Px, b: Px) -> f64 {
    let (abx, aby) = (b.0 - a.0, b.1 - a.1);
    let (apx, apy) = (p.0 - a.0, p.1 - a.1);
    let len2 = abx * abx + aby * aby;
    let t = if len2 > 0.0 {
        ((apx * abx + apy * aby) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (dx, dy) = (apx - t * abx, apy - t * aby);
    (dx * dx + dy * dy).sqrt() // IEEE-exact; no libm, no `mul_add` (§2.7)
}

/// Coverage of a pixel whose centre lies `d` px from the centre-line of a
/// stroke of radius `r` px: the pinned one-pixel ramp — 1 inside, 0.5 on
/// the boundary, 0 outside (§2.5).
fn coverage(d: f64, r: f64) -> f64 {
    unit(r + 0.5 - d)
}

/// Source-over in 8-bit sRGB — SVG's default `color-interpolation` — with
/// one pinned rounding (`f64::round`, ties away from zero).
fn src_over(dst: &mut [u8], src: Rgb, a: f64) {
    for (slot, s) in dst.iter_mut().zip([src.r, src.g, src.b]) {
        let out = f64::from(s) * a + f64::from(*slot) * (1.0 - a);
        *slot = out.round() as u8; // `out ∈ [0, 255]`; the cast saturates
    }
}

/// The style every `Prim2` variant carries (SPEC-0002 §2.2).
fn style_of(prim: &Prim2) -> Style { /* one exhaustive match */ }

/// Both coordinates finite — the guard that keeps `floor`/`ceil` → `u32`
/// from saturating into a nonsense tile (§2.9).
fn finite(p: Px) -> bool {
    p.0.is_finite() && p.1.is_finite()
}

/// Total clamp to `[0, 1]`; NaN maps to 0, so a style violating its
/// documented contract paints nothing rather than poisoning the frame
/// (§2.5). **Not `x.clamp(0.0, 1.0)`** — `f64::clamp` returns NaN for NaN.
/// The comparison order is what routes NaN to the final `else`; see the
/// clippy note below before changing it.
fn unit(x: f64) -> f64 {
    if x > 1.0 {
        1.0
    } else if x > 0.0 {
        x
    } else {
        0.0
    }
}

/// A half-open pixel rectangle clamped to the canvas — the region ink can
/// reach. `None` when empty (entirely off-canvas, or no segments).
#[derive(Clone, Copy)]
struct Tile { x0: u32, y0: u32, x1: u32, y1: u32 }

impl Tile {
    fn around(segments: &[(Px, Px)], pad: f64, size: (u32, u32)) -> Option<Tile> { /* … */ }
    fn intersect(self, other: Option<Tile>) -> Option<Tile> { /* … */ }
    fn area(self) -> usize { /* (x1-x0) * (y1-y0) */ }
    fn offset(self, x: u32, y: u32) -> usize { /* row-major within the tile */ }
}
```

**Two spellings above are pinned against the merge gate**
(`cargo clippy --workspace --all-targets -- -D warnings`,
`project-specifics.md`), and both resist the "obvious" simplification. This
paragraph exists so the next reader does not quietly put them back.

- **`unit` is `> 1.0` / `> 0.0` / `else`.** The natural `is_nan()`-first
  spelling trips **two** lints at once: `clippy::if_same_then_else` (its first
  two arms both return `0.0`) and `clippy::manual_clamp`. The second lint's
  suggested fix, `x.clamp(0.0, 1.0)`, is **semantically wrong here**:
  `f64::clamp` returns NaN for a NaN input, so taking it would let a NaN alpha
  reach `src_over` and paint a tile of garbage — deleting §2.5's "a NaN width
  or alpha paints nothing" without changing a line of the spec. The form above
  is clippy-clean *and* NaN-correct: NaN fails both `>` tests and falls to
  `0.0`. (`distance_to_segment`'s `t.clamp(0.0, 1.0)` is *not* the same case
  and stays: `len2 > 0.0` is checked and both operands are finite, so `t`
  cannot be NaN there.)
- **The width guard is `!(radius.is_finite() && radius > 0.0)`.** The shorter
  `!(radius > 0.0)` trips `clippy::neg_cmp_op_on_partial_ord`, and rewriting
  it as `radius <= 0.0` would be wrong for NaN. The longer form is also the
  *stronger* guard, independently of any lint: an infinite radius passes
  `> 0.0`, then makes `pad = radius + 0.5` infinite and the tile the entire
  canvas (§2.5, §2.9).

Neither is a style preference; both are load-bearing, and a lint will report
neither if they are "tidied" away.

**The golden fixture (AC3), illustrative — exact bytes are frozen at
implementation via the bless path and reviewed as a decoded diff.** A
64×36 canvas over the *default* 3.2 × 1.8 view, so `s = 20` px per image
unit exactly (`64/3.2` and `36/1.8` are both exactly `20.0`), on black.
File length `13 + 64·36·3 = 6 925` bytes. The scene is trig-free by
SPEC-0003 §2.6's construction rules — `Scene::new(1.0)`'s default
orthographic camera at `translator(0, 0, 5)`, single-key tracks, every
vertex in `z = 0` — rendered by `scene.render(1.0, &mut sink)` (one frame,
`t = 0`), and is deliberately fat-stroked so a 64×36 fixture still
exercises interiors, fringes, caps, joins and alpha. **The strokes are fat
for a second reason:** at `s = 20` the four widths give
`r ∈ {2.0, 3.0, 1.6, 1.2}` px, every one of them above §2.8's `r ≥ 1` px
threshold — so the fixture is asserted inside the regime whose tolerance it
is meant to demonstrate, and a later thinning of any width would move it out
(§5):

| object | geometry (image space) | style | what it pins |
|---|---|---|---|
| segment | `(−1, 0) → (1, 0)` | white, `width 0.2` | `r = 2` px, centre-line on the pixel boundary `y = 18.0` → rows 16–19 exactly `255`, rows 15 and 20 exactly `0` (§2.5) |
| point | `(0.5, 0.5)` | orange `#ff4d00`, `width 0.3` | a `r = 3` px disc at pixel `(42, 8)` — the round-cap path and its fringe |
| polyline | `(−0.5,−0.5) → (0.5,−0.5) → (0, 0.25) → (−0.5,−0.5)` | teal `#00b4d8`, `width 0.16`, `alpha 0.5` | round joins, and src-over of a translucent layer over *both* background and the white segment |
| edges | two pairs near the top corners: `(−1.3, 0.7) → (−1.0, 0.7)` and `(1.0, 0.7) → (1.3, 0.7)` | white, `width 0.12` | the fourth variant. `r = 1.2` px; both centre-lines land on the pixel boundary `y = 4.0` (px `6..12` and `52..58`), so rows 3 and 4 are `255` between the caps and rows 2 and 5 carry a `0.2` fringe = byte **51**. Their padded tiles are x ∈ [4, 14) and [50, 60) — disjoint, which is what exercises per-segment tiling |

## 4. Non-goals

- No fills, gradients, blend modes, dashes, miter/bevel joins, or depth
  sorting — strokes and dots only, matching what `Prim2` can express
  (R-0006 §4).
- No PNG, no in-crate video encoding, no audio. Encoding stays the user's
  ffmpeg invocation, outside the crate (RFC-012 §2).
- No colour management: 8-bit sRGB verbatim, composited in sRGB. No
  linearization, no gamma, no ICC.
- No alpha channel in the output (no PAM/P7): alpha is flattened against an
  explicit background at write time.
- No text — R-0007. §2.12 records what is left room for and designs none of
  it.
- No supersampling, no gamma-aware AA, no distance-field caching: the
  single-evaluation ramp is the pinned rule (§2.5).
- No parallel frame or scanline rendering, no SIMD — determinism first;
  throughput is R-0011.
- No frame-directory cleanup **by the sink**, and no evenness enforcement on
  raster size (an encoder constraint, documented on the command, not a format
  one). §2.10's AC6 test cleans its own scratch directory before rendering;
  that is harness hygiene, not sink behaviour, and the sink's truncate-never-
  delete policy is unchanged.
- No changes to `sink.rs`, `scene.rs`, `prim.rs`, `camera.rs`, `svg.rs`, or
  `examples/first_light/scene.rs`; R-0003's golden fixture and its bytes are
  untouched, which is the cheapest proof this spec is additive. §2.13's
  amendment to SPEC-0003 §2.5 is **textual**: it pins a property `svg.rs`
  already has — an attribute it does not emit — and adds no byte anywhere.

## 5. Open questions

None — the architect review of 2026-08-27 adjudicated all four held open
here. Each is recorded in the decision log as architect-adjudicated, owner
acceptance pending.

- **Golden fixture size → 64×36** (6 925 B), as recommended, and now for a
  sharper reason than "it shows more". At `s = 20` the fat strokes give
  `r ∈ [1.2, 3.0]` px, every one above §2.8's `r ≥ 1` px threshold, so the
  fixture sits *inside* the regime whose tolerance AC2 asserts. A 32×18
  fixture (`s = 10`) halves every radius to `r ∈ [0.6, 1.5]`: the polyline
  (`0.8`) and the edges (`0.6`) drop below the threshold and the segment
  lands exactly on it — a fixture demonstrating a guarantee it does not
  meet. Reviewability is answered by §6 AC3's decoded byte-diff reporter,
  not by size.
- **Where the PPM demo lives → amend `examples/first_light/main.rs`**, not a
  new example. The broken command is documented there, so the fix belongs
  there; `scene.rs` stays a single shared source; and R-0003 AC5's literal
  SVG command string stays intact in the same file, untouched (§2.10). A new
  `examples/first_light_ppm/` would duplicate `main.rs` for nothing.
- **Default background → `Rgb::BLACK`, with a consuming `with_background`
  builder** (§2.1). It reproduces what an encoded transparent SVG looks like
  today, keeping the two sinks visually interchangeable out of the box; the
  builder matches `Object::with_style` rather than adding a fourth
  constructor. A white default would be friendlier for print-style diagrams
  and is one call away.
- **AC6 and `ffprobe` → no ffprobe; assert the container magic instead.** A
  second external tool would add a third skip axis to prove something
  `-c:v libx264` already states. In its place AC6 asserts the output's bytes
  `4..8` are `b"ftyp"` — the ISO-BMFF box type every mp4 opens with — which
  catches the one failure a zero exit status does not: ffmpeg succeeding and
  writing garbage. Four bytes of assertion, no new tooling, no new skip axis
  (§2.10).

## 6. Acceptance criteria

Each maps to an R-0006 AC; the qa agent derives the binding tests from
these (tests first, red, then implementation) in
`crates/motoreel/tests/r0006_ppm_sink.rs`, with unit tests for the private
mapping, coverage rule and distance function in `ppm.rs`'s own
`#[cfg(test)] mod tests`.

- [ ] **AC1 — P6 format.** `frame(0, &[])` on a 4×2 sink writes
  `frame_00000.ppm` whose bytes are exactly `b"P6\n4 2\n255\n"` followed by
  24 background bytes (full-file byte equality, no tolerance). Header text
  is exact for 1920×1080 and for a non-square size; `file_len ==
  header.len() + w·h·3` for several sizes; filenames are
  `frame_00000.ppm` / `frame_00123.ppm` for indices 0 / 123; a nested output
  directory is created by `new`; an existing file is overwritten; invalid
  size or view is `ErrorKind::InvalidInput` with the directory not created
  (§2.9).
- [ ] **AC2 — same geometry as SVG.** (a) `to_pixel` equals
  `W/2 + s·x`, `H/2 − s·y` **bit-exactly** for pinned inputs, including
  `s = min(W/vw, H/vh)` in a letterboxed 200×100-over-3.2×1.8 case
  (unit test). (b) `SvgSink::new` and `PpmSink::new` report the same default
  size and view, read from the two headers. (c) One `Prim2` slice through
  both sinks: coordinates parsed out of the SVG by splitting on `'"'` (the
  template is pinned — no XML parser, SPEC-0003 precedent), mapped by the
  §2.3 formula, and asserted to have stroke-coloured ink within **±1 px**
  (§2.8), with background preserved far from every centre-line. **Every
  primitive in this AC2 slice is stroked at `r ≥ 1` px — §2.8's precondition
  — and the assertion states the radius it relies on**, so a later thinning
  of the slice fails loudly instead of quietly invalidating the claim; the
  sub-pixel regime is explicitly not asserted here (it is pinned in AC4).
  (d) The letterboxed case leaves background bands where SVG's `meet`
  letterboxes, proving `min` and not two scales. (e) The header `SvgSink`
  writes contains **no** `preserveAspectRatio` substring — §2.13's amendment
  to SPEC-0003 §2.5, asserted in the requirement that depends on it.
- [ ] **AC3 — determinism.** Two renders of the same scene in one process
  into `CARGO_TARGET_TMPDIR/{a,b}` produce byte-identical files pairwise
  and the expected file list. The §3 trig-free golden scene at 64×36 equals
  `include_bytes!("golden/frame_00000.ppm")` byte-for-byte, and its length
  equals `13 + 6 912` (a truncated fixture cannot pass by prefix). On
  mismatch the test reports the **first differing offset decoded as
  `(x, y, channel)` with both values** — a binary golden must fail legibly.
  Regeneration only via
  `MOTOREEL_BLESS=1 cargo test -p motoreel --test r0006_ppm_sink`, reviewed
  like code. `.gitattributes` marks `crates/motoreel/tests/golden/*.ppm binary
crates/motoreel/tests/golden/*.svg -text`,
  covering R-0003's existing SVG fixture as well as this one (§2.11).
- [ ] **AC4 — anti-aliasing and width.** The §2.5 worked numbers, asserted
  exactly: a `r = 1` px stroke on a pixel boundary lights exactly two rows
  at `255` with no fringe; shifted half a pixel it lights one row at `255`
  and two at exactly `128`. A `r = 2` px stroke lights exactly four full
  rows. **Width is honoured in image units by §2.8's identity, asserted at
  the layer that owns it:** in `ppm.rs`'s unit tests, where `coverage`
  returns `f64`, `Σ_j coverage(|j + 0.5 − y₀|, r) == 2r == s · width` is
  compared with `==` — no tolerance — sweeping `y₀` across a whole pixel at
  `r ∈ {0.5, 0.75, 1, 1.5, 2, 3, 6}` px. The frame-level counterpart reads
  the same cross-section out of a white-on-black frame, where the per-channel
  `round` costs up to half a byte per pixel, so it asserts
  `|Σ(byte / 255) − s·width| ≤ n / 510` for an `n`-pixel cross-section:
  **which layer carries the exact claim is part of the claim**, and a
  frame-level `==` would be false precision. **The lit extent is asserted as
  `2r + 1` px** (`r = 1` → 3 px, `r = 2` → 5 px); the tempting "doubling the
  width doubles the lit width" is *false* and must not be written (§2.8).
  One assertion pins the documented sub-pixel regime the other way — at
  `r = 0.05` the summed coverage **exceeds** `2r` (up to 5.5× at the worst
  offset) — so the over-ink stays documented behaviour rather than becoming a
  later surprise. `width = 0`, negative or NaN paints nothing — as SVG draws
  nothing for `stroke-width="0"` — and so does an infinite width, by §2.5's
  guard. A white stroke at `alpha = 0.5` over black at full coverage is
  exactly `128`. A polyline's joint shows **no** darker seam — the
  union-by-max property, asserted by comparing the joint pixel against the
  interior of either arm.
- [ ] **AC5 — full vocabulary, cull, finiteness.** Each of `Point`,
  `Segment`, `Polyline`, `Edges` produces ink at its mapped positions;
  `Prim2::Point { at, style }` and `Prim2::Segment { a: at, b: at, style }`
  produce **byte-identical frames**; a `Polyline` of 0 or 1 points and an
  empty `Edges` produce a pure-background frame; a scene whose geometry is
  behind the camera yields a pure-background frame (the cull is
  `Scene::eval`'s, inherited unchanged); every coordinate reaching the sink
  is finite by SPEC-0002's structural guarantee, tripwired by
  `debug_assert`.
- [ ] **AC6 — end to end with stock ffmpeg.** The two probes skip with a
  printed note (`which ffmpeg`; then `ffmpeg -hide_banner -encoders`
  containing `libx264`); otherwise 15 frames at 320×180 (`0.25 s × 60 fps`,
  dyadic) are rendered into a **freshly cleaned** directory (§2.1 truncates
  but never deletes, so a stale frame would otherwise join the encode) and
  passed to the documented command shape **plus `-y -nostdin`** — harness
  hygiene, because ffmpeg otherwise prompts on an existing output and reads
  the answer from a stdin `cargo test` does not provide, hanging or failing
  for a reason unrelated to our frames (§2.10). The exit status must be
  success, the mp4 non-empty, and its bytes `4..8` exactly `b"ftyp"` — the
  ISO-BMFF box type, which is what catches an ffmpeg that exits 0 having
  written garbage. Separately (and unconditionally, no ffmpeg required)
  `examples/first_light/main.rs` must contain the literal
  `ffmpeg -framerate 60 -i out/ppm/frame_%05d.ppm` — the R-0003 AC5
  precedent, and the reason the *documented* command carries no `-y`
  or `-nostdin` — and R-0003's own SVG line must still be present, unchanged.
- [ ] **AC7 — zero new dependencies.** `[dependencies]` is exactly
  `["garust"]`, `[dev-dependencies]` exactly `["proptest"]`, no
  `[build-dependencies]` and no target-specific dependency table — the
  `section_keys` assertion from R-0003 AC5, reused. Architect/PR gate on the
  `Cargo.toml` diff.

## 7. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-08-27 | Rasterize by **per-pixel distance to segment** over each segment's bounding box, not a scanline/edge-list polygon filler | Caps, joins and dots are all "within `r` of a segment", so one formula covers the whole `Prim2` vocabulary; nothing is carried pixel to pixel, which is the determinism property that matters (§2.4, §2.7) |
| 2026-08-27 | Coverage rule pinned to the one-pixel ramp `clamp(r + 0.5 − d, 0, 1)`, evaluated **once** per pixel | Deterministic with no accumulation order to pin at all — the honest answer to "AA quality vs determinism"; supersampling would be 16× the work and would introduce a sample-accumulation order to freeze (§2.5) |
| 2026-08-27 | Coverage of a multi-segment primitive is combined by **`max`**, and the primitive is composited **once** | Per-segment compositing double-darkens every polyline joint — exactly where SVG's round join is seamless. `max` is order-independent on finite floats and adds no error; `a + b − a·b` over-estimates and is order-dependent (§2.5) |
| 2026-08-27 | AC2 tolerance split: the **mapping is exact** (bit-equal to `W/2 + s·x`, `H/2 − s·y`), the **ink lands within ±1 px** | Sampling at pixel centres quantizes by ½ px and the ramp spans a further ½ px; ±1 px is the honest sum, and at `s = 600` it is 1/600 of an image unit — far below any real placement bug (§2.8) |
| 2026-08-27 | Scale is `min(W/vw, H/vh)`, reproducing SVG's default `xMidYMid meet`, not two independent scales | Otherwise the two sinks agree only when the aspect ratios happen to match; `meet` makes AC2 hold for every `with_view` pair, and the letterboxed case is a test (§2.3) |
| 2026-08-27 | Pixel centres sampled at `+0.5`, matching SVG's viewport convention | The only convention under which the sinks agree sub-pixel, and it makes every AA expectation hand-computable (§2.5) |
| 2026-08-27 | Compositing in **8-bit sRGB** over a `u8` framebuffer, one `f64::round` per channel per layer | The framebuffer *is* the file body — no conversion pass, no 50 MB f64 shadow buffer — and sRGB compositing is what SVG does by default. Quantization per layer is a pure function of the pinned rounding, so determinism is untouched; colour agreement is not claimed (§2.6, §2.8) |
| 2026-08-27 | Only IEEE-correctly-rounded operations in the raster path; `mul_add` forbidden, no libm transcendental | `sqrt`/`floor`/`ceil`/`round` are uniquely defined, so the *raster stage* is bit-portable across platforms — the golden fixture is trustworthy on any CI host, and the platform caveat stays where it belongs, in trig-bearing scenes (§2.7) |
| 2026-08-27 | `width ≤ 0`, non-finite, or `alpha` NaN paints **nothing** | Matches SVG's `stroke-width="0"`; without the guard the ramp would paint a phantom 50 %-covered hairline, which would be a silent divergence from the other sink (§2.5) |
| 2026-08-27 | Buffer sizing validated (`w·h·3` must fit `usize`) and reserved with `Vec::try_reserve` | Constitution §6 forbids unchecked failures in library code; a caller-supplied huge size must be `InvalidInput`/`OutOfMemory`, never a process abort inside `Vec` (§2.9) |
| 2026-08-27 | Header + body written as two `write_all`s on one `File`, deviating from SPEC-0003's single `fs::write` | A third 6.2 MB buffer per frame buys nothing; the guarantee is unchanged — one file, ≤ 1 partial file on failure, no cleanup (§2.2) |
| 2026-08-27 | Default background `Rgb::BLACK`, overridable by a consuming `with_background` builder | Reproduces what an encoded transparent SVG looks like today (SPEC-0003 §4), keeping the two sinks visually interchangeable; the builder matches `Object::with_style`, not a fourth constructor (§2.1) |
| 2026-08-27 | One module `ppm.rs` with a private `Canvas`, not a separate `raster.rs` | One consumer today; a module per caller is the premature abstraction §2 forbids. The extraction is mechanical and additive if R-0007's glyph code needs it (§2.0, §2.12) |
| 2026-08-27 | `push_segments` matches `Prim2` exhaustively, no `_` arm | R-0007's new variant must be a compile error in this file, never a label that silently fails to render (§2.4, §2.12) |
| 2026-08-27 | AC6 skips via a `which ffmpeg` probe **and** a `libx264` encoder probe; a present ffmpeg that rejects our frames **fails** | Both probes can only cause a skip, never a false pass or false failure. A creator's ffmpeg packaging is not a motoreel regression; a decode failure is exactly the bug R-0006 exists to catch (§2.10) |
| 2026-08-27 | `which` accepted despite being non-portable; promotion path recorded | On a host without `which` the test skips, which is the safe direction. If Windows CI ever matters, spawn `ffmpeg -version` and treat `ErrorKind::NotFound` as absent (§2.10) |
| 2026-08-27 | PPM demo writes to `out/ppm/`, leaving the SVG demo's `out/` path untouched | R-0003's QA-signed AC5 test asserts the literal `-i out/frame_%05d.svg`; tidying the directory layout would break a signed-off requirement for cosmetics (§2.10) |
| 2026-08-27 | Documented command names `-c:v libx264` explicitly and states the `yuv420p` even-dimension requirement and the ≈ 1.5 GB intermediate | AC6 claims h264, so the claim should be checkable; the even-size and disk-cost facts are exactly what bites a creator first (§2.10) |
| 2026-08-27 | Golden fixture 64×36 over the default view (`s = 20` exactly), fat-stroked, trig-free; failures report the first differing byte as `(x, y, channel)` | A binary golden must still fail legibly; SPEC-0003 §2.6's construction rules keep the bytes platform-portable, and fat strokes keep a tiny fixture meaningful (§3, §6 AC3) |
| 2026-08-27 | Add `.gitattributes` with `crates/motoreel/tests/golden/*.ppm binary` | The repo has no `.gitattributes`; an eol/autocrlf setting could rewrite `\n` bytes in the P6 header or body and silently corrupt a byte-exact fixture (§2.11) |
| 2026-08-27 | A `Polyline` of fewer than two points, and an empty `Edges`, paint nothing — falling out of `windows(2)` with no branch | Matches the dominant SVG behaviour for a lone `moveto` and SPEC-0004's `d=""`; renderers disagree here, so the behaviour is pinned and the divergence documented rather than papered over (§2.4, §2.8) |
| 2026-08-27 | **AC2's ±1 px tolerance is scoped to `r ≥ 1` px** (architect review S6-1); sub-pixel placement moves to §2.8's "explicitly not claimed" | The guarantee is measurably false for thin strokes: a vertex on a pixel corner is `√2/2 ≈ 0.7071` px from the nearest centre, so the best coverage anywhere in the 3×3 block is `0.2929` at `r = 0.5`, `0.0429` at `r = 0.25`, `0.0` at `r = 0.1`. A ≥ 0.5 pixel needs `r ≥ √2/2` at a vertex and `r ≥ 0.5` in a segment interior; `r ≥ 1` covers both. `Style::default().width = 0.01` at the golden's `s = 20` is `r = 0.1` px, so this is a regime real callers reach (§2.8) |
| 2026-08-27 | **AC4 is restated as the exact identity `Σ coverage == s · width` for `r ≥ 0.5`**, replacing "doubling the width doubles the lit width" (architect review S6-2) | The doubling claim is false — lit extent is `2r + 1` px, so `r: 1 → 2` gives 3 px → 5 px, not 6 — and a test written against it would fail on a correct rasterizer. The ramp is a box-of-`2r` convolved with a box-of-`1`, so its unit-spaced sample sum equals `2r` *bit-exactly at every sub-pixel offset* (architect: `max |Σcov − 2r| = 0` over 1000 offsets at `r ∈ {0.5, 0.75, 1, 1.5, 2, 3, 6}`). Below `r = 0.5` the plateau vanishes and the sink over-inks — up to 5.5× at `r = 0.05` — documented, not claimed (§2.5, §2.8, §6 AC4) |
| 2026-08-27 | `unit` is spelled `> 1.0` / `> 0.0` / `else`, and the width guard `!(radius.is_finite() && radius > 0.0)` (architect review S6-3) | The outline as written failed the merge gate: the `is_nan()`-first `unit` trips `clippy::if_same_then_else` *and* `clippy::manual_clamp`, whose fix `x.clamp(0.0, 1.0)` returns **NaN for NaN** and would destroy §2.5's "a NaN width or alpha paints nothing"; `!(radius > 0.0)` trips `clippy::neg_cmp_op_on_partial_ord`. Both replacements are clippy-clean, and the guard is independently stronger — an infinite radius would make `pad` infinite and paint the whole canvas. Recorded in §3 so neither is "simplified" back (§2.5, §3) |
| 2026-08-27 | **SPEC-0003 §2.5 is amended: the absent `preserveAspectRatio` attribute is normative, and AC2 asserts its absence** (architect review S6-4) | §2.3's whole `s = min(W/vw, H/vh)` derivation rests on SVG's default `xMidYMid meet`, which SPEC-0003 never pinned and no existing test could see: adding `preserveAspectRatio="none"` would keep the SVG well-formed, get the golden re-blessed, and silently break sink agreement on every non-16:9 view. The amendment changes no code and no fixture byte — the shape SPEC-0004 §2.3 and SPEC-0007 §2.6 established (§2.13) |
| 2026-08-27 | The golden fixture's `Edges` row is given exact coordinates — `(−1.3, 0.7) → (−1.0, 0.7)` and `(1.0, 0.7) → (1.3, 0.7)` (architect review S6-5) | "Two disjoint pairs near the top corners" is not reproducible from the spec; a fixture that can only be recovered by blessing is a fixture nobody can check. The stated pair also makes its own expectations hand-checkable — `r = 1.2` px on the boundary `y = 4.0` → rows 3 and 4 at `255`, rows 2 and 5 at byte 51, tiles x ∈ [4, 14) and [50, 60) (§3) |
| 2026-08-27 | `.gitattributes` widened from `tests/golden/*.ppm binary` to `crates/motoreel/tests/golden/*.ppm binary
crates/motoreel/tests/golden/*.svg -text` (architect review S6-6) | R-0003's `frame_00000.svg` is a byte-exact fixture too and is unprotected today — a text fixture is *more* exposed to an eol rewrite, not less. One glob covers it, covers R-0007's fixtures before they exist, and cannot be forgotten by the next author. Cost accepted: `binary` implies `-diff`, so the SVG golden stops showing a text diff; goldens are reviewed through bless regeneration and §6 AC3's reporter anyway (§2.11) |
| 2026-08-27 | The AC6 encode test adds `-y -nostdin` and renders into a freshly cleaned directory; the **documented creator command stays exactly as published** (architect review S6-7) | ffmpeg prompts to overwrite an existing output and reads the answer from stdin, which `cargo test` does not give it — the test would hang or fail for a reason unrelated to our frames. §2.1's truncate-never-delete policy makes the clean equally load-bearing on a second run. The documented string keeps neither flag: a creator wants the prompt, and inserting flags would break AC6's literal-substring assertion against `main.rs` (§2.10, §6 AC6) |
| 2026-08-27 | The raster stage's bit-portability claim gains its precondition: targets whose `f64` arithmetic is IEEE double (architect review S6-8) | x87-only targets (`i586-*`, no SSE2) compute in 80-bit registers and double-round on spill, which can move a last-ulp result and a golden byte. `x86_64` and `aarch64` — everything this project builds for — are IEEE double, so the claim stands as written, but unqualified it was an overstatement (§2.7) |
| 2026-08-27 | §2.0's module rule narrowed from "a second module for a single caller" to "a module per single *abstraction*" (architect review C-3) | As written it read as forbidding SPEC-0007's `font.rs`, which holds *data* (a `const` bitmap table), not an abstraction. The rule is about premature abstraction, not line counts, and should not be quotable against a module whose only sin is having one reader (§2.0) |
| 2026-08-27 | **R-0006 must reach `Met` before R-0007 implementation begins** (architect review C-2) | SPEC-0007 consumes `to_pixel`, `src_over`, `unit`, `Tile` and `Canvas` directly. Building on an unsigned-off rasterizer means duplicating them or absorbing a correction mid-requirement, and every §2.8 claim R-0007 inherits would come from an unverified source (§2.12) |
| 2026-08-27 | §5's four open questions adjudicated as recommended: 64×36 golden, amend `first_light/main.rs`, `Rgb::BLACK` default with `with_background`, no `ffprobe` | Architect adjudication, owner acceptance pending. The golden's size gains a second reason: at `s = 20` its radii are `r ∈ [1.2, 3.0]` px, inside §2.8's `r ≥ 1` px regime, whereas a 32×18 fixture (`s = 10`) gives `r ∈ [0.6, 1.5]` — two of its four objects below the tolerance it is meant to demonstrate and a third exactly on it (§5) |
| 2026-08-27 | AC6 asserts the encoded mp4's bytes `4..8` are `b"ftyp"`, in place of an `ffprobe` stream check | The ISO-BMFF box type is four bytes of assertion with no new tool and no third skip axis, and it catches the one failure a zero exit status misses: ffmpeg succeeding and writing garbage (§2.10, §5) |

## Changelog

- 2026-08-27 — created.
- 2026-08-27 — architect review (REQUEST CHANGES) applied: S6-1 through S6-8,
  C-2 and C-3. §2.8 rewritten around the `r ≥ 1` px precondition and the
  exact `Σ coverage == s · width` identity; §2.13 added, amending SPEC-0003
  §2.5; §5's four open questions adjudicated and closed. History is appended
  to, not rewritten.
