# SPEC-0013 — Glyph coverage clamp

- **Status:** Draft — code outline awaiting the owner
- **Realizes:** R-0013
- **Author:** Claude (main session)
- **Created:** 2026-10-06
- **Depends on:** SPEC-0006 (blend), R-0009's `rasterize`
- **Module(s):** `motoreel-typeset::raster`

## 1. Motivation

Realizes R-0013: coverage above 1 from overlapping contours extrapolates
the sink's blend past the ink.

## 2. Design

The coverage chain, from the face to the pixel:

1. `ab_glyph::OutlinedGlyph::draw` gives `c = |Σ signed area|`. With two
   overlapping contours of the same winding, c = 2 inside the overlap.
2. `motoreel_typeset::rasterize` (`raster.rs`) passes it to `emit`
   unchanged when `c > 0`.
3. `PpmSink` (`ppm.rs`, text path) calls `blend_pixel(…, alpha * c)`.
4. `blend_pixel` computes `alpha.mul_add(over − under, under)`: with
   alpha > 1 this lands past `over`, and the `as u8` cast saturates.

**Option A (recommended).** Clamp where the value is born, step 2:
`f64::from(c.min(1.0))`. One place owns the invariant ("coverage is a
fraction of the pixel"), and every consumer of `rasterize` inherits it:
`PpmSink`, and any future sink. `blend_pixel` stays as it is. Its other
callers pass `style.alpha`, which R-0006 already bounds.

**Option B.** Emit glyph outlines as `Subpath`s and fill them with
R-0012's nonzero filler. Exact union coverage on edges inside overlaps.
A second requirement, if ever needed (R-0013 OQ-1).

## 3. Code outline

```rust
// crates/motoreel-typeset/src/raster.rs, inside `rasterize`
outline.draw(|dx, dy, c| {
    if c > 0.0 {
        // ab_glyph sums every contour under the pixel; where two contours
        // of one glyph overlap (stroke junctions in shipped faces) the sum
        // passes 1. Coverage is a fraction of the pixel (R-0013).
        emit(x0 + i64::from(dx), top - i64::from(dy), f64::from(c.min(1.0)));
    }
});
```

Tried locally, uncommitted: the three R-0013 tests turn green and all of
`cargo test --workspace` stays green, goldens included.

## 4. Non-goals

- Option B (R-0013 OQ-1).
- Changing `blend_pixel`.

## 5. Open questions

- R-0013 OQ-1 (A or B).

## 6. Acceptance criteria

- [ ] `ac1_coverage_never_exceeds_one`: `motoreel-typeset/tests/r0013_coverage.rs` (red today: coverage 2)
- [ ] `ac2_no_pixel_is_brighter_than_the_ink`: `motoreel/tests/r0013_glyph_coverage.rs` (red today: (255, 200, 100) on a (200, 100, 50) ink)
- [ ] `ac3_outside_the_overlap_nothing_changes` (green today; must stay green) and the existing suite unchanged

The fixture is `motoreel-typeset/tests/fixtures/overlap.ttf` (676 bytes),
remade by `overlap.py` (fontTools). It has two glyphs: 'X', two
overlapping squares wound the same way, and 'U', their union.

## 7. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| | | |

## Changelog

- 2026-10-06 — created (Draft).
