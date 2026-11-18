# SPEC-0016 — Gradient fills

- **Status:** **Accepted** (2026-11-18, on the owner's go-ahead for R-0016). Implemented in the R-0016 PR; architect and owner review pending
- **Realizes:** R-0016
- **Author:** Claude (main session) with owner
- **Created:** 2026-11-18
- **Depends on:** SPEC-0012 (`Fill`, `Prim2::Path`, the fill rasterizer
  §2.6, the SVG `<path>` §2.8), SPEC-0006 (coverage, `src_over`,
  determinism §2.7), SPEC-0003 (number formatting, golden scheme)
- **Module(s):**
  - new `crates/motoreel/src/gradient.rs`: `Stop`, `Gradient`,
    `GradientKind`, `GradientError`; validation, the frame, the offset of a
    point and the colour at an offset. std-only, pure
  - `crates/motoreel/src/prim.rs`: `Paint`, `Fill { paint, alpha }`,
    `Fill::gradient`
  - `crates/motoreel/src/scene.rs`: carry a path's gradient frame into
    image space
  - `crates/motoreel/src/ppm.rs`: per-pixel gradient paint and the ordered
    dither inside the fill composite
  - `crates/motoreel/src/svg.rs`: `<linearGradient>` / `<radialGradient>`
  - `lib.rs`: exports
  - new `tests/r0016_gradients.rs`, new goldens `tests/golden/r0016_*`

## 1. Motivation

R-0016 asks for linear and radial gradient fills without banding. This
spec adds them on top of SPEC-0012's fill rasterizer without touching its
coverage: the coverage rule decides *how much* of a pixel is painted, and
this spec only changes *with what*. Flat fills keep their exact code path,
which is why no golden byte moves.

## 2. Design

### 2.1 The gradient (`gradient.rs`)

```rust
pub const MAX_STOPS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stop { pub offset: f64, pub colour: Rgb, pub alpha: f64 }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GradientKind { Linear, Radial }

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gradient {
    kind: GradientKind,
    origin: Pt2, x_end: Pt2, y_end: Pt2,   // the frame (§2.2)
    stops: [Stop; MAX_STOPS],              // unused slots repeat the last stop
    len: usize,
}
```

`Gradient::linear(p0, p1, &stops)` and `Gradient::radial(centre, radius,
&stops)` validate (§2.4) and return `Result<Gradient, GradientError>`.
Unused stop slots are filled with copies of the last stop, so the derived
`PartialEq` compares meaning, not leftovers. Accessors: `kind()`,
`frame()`, `stops()`.

### 2.2 The frame

Both kinds are described by three points, the **frame**: `origin`
(offset-space `(0, 0)`), `x_end` (`(1, 0)`) and `y_end` (`(0, 1)`). With
`u = x_end − origin`, `v = y_end − origin` and `det = u × v`
(`u.x·v.y − u.y·v.x`), a point `q` has frame coordinates

```text
d = q − origin
a = (d × v) / det
b = (u × d) / det
```

and its **offset** is `t = a` (linear) or `t = sqrt(a·a + b·b)` (radial).

| kind | origin | x_end | y_end |
|---|---|---|---|
| linear | `p0` | `p1` | `p0 + (−(p1.y − p0.y), p1.x − p0.x)` |
| radial | `centre` | `centre + (radius, 0)` | `centre + (0, radius)` |

The frame is what makes every pose one case: eval and the raster sink
map the three points (an affine map sends the frame of a gradient to the
frame of its image), and SVG takes the same numbers as
`matrix(u.x u.y v.x v.y origin.x origin.y)`.

**Exactness.** At `q = origin`, `d = 0` and `t = 0`. At `q = x_end`, `d`
is computed by the same subtraction as `u`, so `d × v` and `det` are the
same expression on the same bits and `a = 1` exactly, `b = 0`
(`u × u = 0` exactly, as both products are the same rounded value).

`Gradient::offset_at(q) -> Option<f64>` returns `None` when `det` is zero
or non-finite: a **singular** frame, which paints nothing (R-0016 §6).

### 2.3 The colour at an offset

`Gradient::paint_at(t) -> ([f64; 3], f64)`: sRGB channel values in
`[0, 255]` and the stop opacity, by this exact procedure:

1. If `t ≤ stops[0].offset` (or `t` is NaN): the first stop.
2. If `t ≥ stops[len−1].offset`: the last stop.
3. Otherwise the span is the last `i` with `stops[i].offset ≤ t`, so
   `stops[i].offset ≤ t < stops[i+1].offset` and the span has positive
   width. `s = (t − o_i) / (o_{i+1} − o_i)`, and each channel and the
   opacity is `c_i + (c_{i+1} − c_i) · s`.

Pad is steps 1–2. Taking the *last* `i` makes the later of two equal
offsets win, as in SVG. The ends are exact because steps 1–2 return the
stop's own values. Equal stops interpolate to themselves exactly
(`c + 0·s = c`). The values are unpremultiplied (R-0016 §6).

### 2.4 Validation (R-0016 AC2)

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GradientError {
    StopCount { count: usize },          // not in 2..=8
    StopOffset { index: usize },         // non-finite or outside [0, 1]
    StopsUnsorted { index: usize },      // offset < predecessor's
    StopAlpha { index: usize },          // non-finite or outside [0, 1]
    NonFinite,                           // a point, the radius, or the frame
    DegenerateAxis,                      // linear p0 == p1
    NonPositiveRadius,                   // radial radius ≤ 0
}
```

Checks run in that order, stops in index order, and the first failure is
returned. `NonFinite` also covers a finite input whose frame overflows
(`det` non-finite, e.g. `p1 − p0` beyond `f64::MAX`), so every constructed
gradient has a finite, non-zero `det` in model space. Implements
`Display` and `std::error::Error`.

### 2.5 Paint and fill (`prim.rs`)

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Paint { Flat(Rgb), Gradient(Gradient) }

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fill { pub paint: Paint, pub alpha: f64 }

impl Fill {
    pub const fn solid(colour: Rgb, alpha: f64) -> Self;      // Paint::Flat
    pub const fn gradient(gradient: Gradient, alpha: f64) -> Self;
}
```

`alpha` keeps R-0012's contract and guard (a non-finite or non-positive
`alpha` paints nothing in both sinks). The paint's stop opacity multiplies
it, as SVG multiplies `stop-opacity` by `fill-opacity`.

### 2.6 Posing (`scene.rs`, R-0016 AC4)

For `Shape::Path` only (the one shape that honours `fill`), when the fill
is a gradient, eval maps its frame: each frame point `(x, y)` becomes
`pga::Point::new(x, y, 0.0)`, rides `to_view` and is projected with the
primitive's projection. If any of the three fails to project, the
primitive is culled (`None`). Every other style field and every flat fill
is still carried verbatim.

Orthographic: the map from the model plane to the image is affine, so the
image frame is exact. Pinhole: the image frame is the affine map through
the three projected points; it agrees with the true perspective image at
those three points only.

### 2.7 Raster (`ppm.rs`, R-0016 AC5, AC7)

`draw_fill` decides the **ink** once per primitive, then hands it to the
unchanged coverage passes:

1. The alpha guard of SPEC-0012 §2.13 (unchanged).
2. `Paint::Flat(c)` → `Ink::Flat(c)` at `alpha`: the exact R-0012 path.
3. `Paint::Gradient(g)`: map the frame to pixel space with `to_pixel`. If
   the pixel-space frame is singular, paint nothing. If every stop is
   identical, `Ink::Flat(colour)` at `alpha · stop.alpha` (R-0016 AC6).
   Otherwise `Ink::Ramp(g_px)`.

`composite_fill` keeps its coverage and visiting order; per painted pixel
it calls the ink:

- `Flat(c)`: `src_over(px, c, cov · alpha)` — the R-0012 expression, the
  same operations in the same order.
- `Ramp(g)`: with `(rgb, a_s) = g.paint_at(t)` at the pixel centre and
  `a = cov · alpha · a_s`, each channel is
  `floor(rgb · a + dst · (1 − a) + θ(x, y))`, where

```text
θ(x, y) = (BAYER[y mod 8][x mod 8] + 0.5) / 64

BAYER =  0 32  8 40  2 34 10 42
        48 16 56 24 50 18 58 26
        12 44  4 36 14 46  6 38
        60 28 52 20 62 30 54 22
         3 35 11 43  1 33  9 41
        51 19 59 27 49 17 57 25
        15 47  7 39 13 45  5 37
        63 31 55 23 61 29 53 21
```

the recursive Bayer index matrix (B. E. Bayer, *An optimum method for
two-level rendition of continuous-tone pictures*, ICC 1973). `x`, `y` are
canvas pixel indices, so the pattern is fixed to the canvas, not to the
object: it does not crawl when the object moves, and it does not change
between frames.

Properties: `θ ∈ (0, 1)`, so `floor(n + θ) = n` for an integer `n` (opaque
stops and pad are exact) and `out + θ < 256` (no overflow). The 64
thresholds average ½, so `floor(v + θ)` averages `v` over an aligned 8 × 8
block to within the block's own spread (AC7's bias bound).

### 2.8 SVG (`svg.rs`, R-0016 AC9)

A gradient fill whose image frame is invertible writes, immediately
before its `<path>`:

```xml
<linearGradient id="g0" gradientUnits="userSpaceOnUse" gradientTransform="matrix(ux uy vx vy ox oy)" x1="0" y1="0" x2="1" y2="0" spreadMethod="pad">
<stop offset="0" stop-color="#rrggbb" stop-opacity="1"/>
…
</linearGradient>
```

or `<radialGradient … cx="0" cy="0" r="1" spreadMethod="pad">`, and the
path's fill attribute is `fill="url(#g0)"` followed by the existing
`fill-opacity` and `fill-rule`. Ids are `g<n>`, `n` counting gradient
fills from 0 in each frame's draw order. Every number uses SPEC-0003's
`{}` rule. A singular frame writes `fill="none"`, as the raster sink
paints nothing. The gradient lives inside the flipped `<g>`, so its
coordinates are image coordinates, like the path's.

A uniform gradient is still written as a gradient: SVG has no banding to
avoid, and the element states what the author wrote.

### 2.9 Determinism (R-0016 AC8)

The raster path adds `+ − × ÷`, one `sqrt` (radial), `floor` and a table
lookup; nothing transcendental, no `mul_add`, nothing carried between
pixels. Coverage is SPEC-0012's. The golden scene is trig-free (identity
holds, binary-exact coordinates), so it is bit-portable.

### 2.10 Compatibility (R-0016 AC6)

- Flat fills run the identical expressions: every golden is unchanged.
- Mechanical edits forced by `Fill { paint, alpha }`: the `Fill { colour,
  alpha }` literal and the `f.colour` read in `r0012_paths_fills.rs`;
  downstream, `style.fill.unwrap().colour` in guion-video-creator's
  `guion-marca/tests/r0014_plantilla.rs`.

## 3. Code outline

```rust
// ppm.rs — the ink, decided once per fill
enum Ink { Flat(Rgb), Ramp(Gradient) }

fn ink_for(paint: Paint, alpha: f64, map: impl Fn(Pt2) -> Px) -> Option<(Ink, f64)> {
    match paint {
        Paint::Flat(c) => Some((Ink::Flat(c), alpha)),
        Paint::Gradient(g) => {
            let g = g.map_frame(|p| { let (x, y) = map(p); Some(Pt2 { x, y }) })?;
            g.offset_at(g.frame().0)?; // singular → None: paint nothing
            match g.uniform() {
                Some(s) => Some((Ink::Flat(s.colour), alpha * s.alpha)),
                None => Some((Ink::Ramp(g), alpha)),
            }
        }
    }
}

// inside composite_fill, per pixel with cov > 0
match ink {
    Ink::Flat(c) => src_over(px, *c, cov * alpha),
    Ink::Ramp(g) => {
        let t = g.offset_at(centre).unwrap_or(0.0); // invertible by construction
        let (rgb, a) = g.paint_at(t);
        dithered_over(px, rgb, cov * alpha * a, threshold(x, y));
    }
}
```

## 4. Non-goals

As R-0016 §4: no focal point, no reflect/repeat, no linear-light, no
perspective-correct gradients, no gradient strokes or text, no dithering
of flat paint, no bloom, additive blending, clip paths or isometric
projection.

## 5. Open questions

None.

## 6. Acceptance criteria → tests

All in `tests/r0016_gradients.rs`, one `mod` per criterion, plus unit
tests in `src/gradient.rs`, `src/ppm.rs` and `src/svg.rs` for the private
pieces. A singular image frame cannot be reached from public input with
binary-exact arithmetic (an edge-on rotation leaves a residue of order
1e-17), so it is pinned by unit tests on a collapsed frame.

| AC | Test(s) |
|---|---|
| AC1 | `ac1::solid_is_flat_paint_and_everything_stays_copy`, `ac1::linear_and_radial_keep_their_frame_and_stops`, `ac1::eight_stops_are_accepted_and_equality_is_by_meaning` |
| AC2 | `ac2::stop_count_outside_two_to_eight_is_rejected`, `ac2::bad_offsets_are_rejected_at_their_index`, `ac2::unsorted_offsets_are_rejected_at_their_index`, `ac2::bad_stop_alpha_is_rejected_at_their_index`, `ac2::non_finite_geometry_is_rejected`, `ac2::degenerate_axis_and_non_positive_radius_are_rejected`, `ac2::errors_display_and_implement_error` |
| AC3 | `ac3::endpoints_are_the_stops_exactly`, `ac3::pad_holds_the_end_stops_beyond_the_ends`, `ac3::interpolation_is_linear_in_srgb_unpremultiplied`, `ac3::equal_offsets_make_a_hard_edge_and_the_later_stop_wins`, `ac3::linear_offset_is_constant_across_the_axis`, `ac3::radial_offset_is_the_distance_over_the_radius` |
| AC4 | `ac4::a_translated_object_moves_its_gradient`, `ac4::a_rotated_object_rotates_its_gradient`, `ac4::pinhole_frame_is_the_projected_frame_points`, `ac4::a_frame_point_behind_the_camera_culls_the_primitive`, `ac4::flat_fills_and_other_styles_pass_verbatim`; unit: `gradient::tests::a_frame_point_that_does_not_map_fails_the_map` |
| AC5 | `ac5::linear_endpoints_paint_their_stop_colours_exactly`, `ac5::pad_region_is_exactly_the_end_stop`, `ac5::radial_centre_and_edge_paint_their_stops` |
| AC6 | `ac6::equal_stops_render_byte_identical_to_solid`, `ac6::the_r0012_golden_is_unchanged` (and every existing golden test, run unmodified); unit: `gradient::tests::uniform_means_every_stop_is_equal` |
| AC7 | `ac7::a_long_ramp_has_no_column_run_longer_than_the_dither_period`, `ac7::dither_is_unbiased_per_8x8_block`, `ac7::flat_fills_are_not_dithered`; unit: `ppm::tests::bayer_thresholds_are_each_level_once_and_average_a_half`, `ppm::tests::dithered_over_keeps_integers_and_saturates_at_255` |
| AC8 | `ac8::two_renders_are_byte_identical`, `ac8::renders_on_several_threads_are_byte_identical`, `ac8::the_ppm_golden_matches_byte_for_byte` |
| AC9 | `ac9::the_svg_golden_matches_byte_for_byte`, `ac9::a_linear_gradient_is_written_as_pinned`, `ac9::a_radial_gradient_names_its_unit_circle`, `ac9::ids_count_from_zero_in_draw_order_each_frame`; singular frames (both sinks agree on "nothing"): `svg::tests::a_singular_gradient_frame_writes_fill_none`, `ppm::tests::a_singular_gradient_frame_paints_no_ink`, `gradient::tests::a_collapsed_frame_has_no_offset` |
| AC10 | `ac10::gradients_need_no_feature` (built under `--no-default-features` by CI), plus the existing dependency-set test |

## 7. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-11-18 | As R-0016 §6 | Recorded once, in the requirement |

## Changelog

- 2026-11-18: created and accepted with R-0016.
