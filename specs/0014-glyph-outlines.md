# SPEC-0014 — Glyph outlines

- **Status:** Draft
- **Realizes:** R-0014
- **Author:** Claude, with the owner
- **Created:** 2026-10-14
- **Depends on:** SPEC-0012 (paths), R-0009 (faces)
- **Module(s):** `motoreel-typeset` (new module `outline`)

## 1. Motivation

Realizes R-0014: text as contours, so a creator can transform it and fill it
with the nonzero rule. Lifted from rotorf-sico's `Letras::linea`
(`mathematicus/motor/src/texto.rs`), which has drawn every text of that
studio's second account since 2026-10-06.

## 2. Design

`ab_glyph::Font::outline(id)` gives a glyph's curves in font units, y up,
relative to its origin. For each character: look up the glyph (missing → the
same `Error::Missing` as `measure`), walk its curves, start a new contour when
a curve does not begin where the previous one ended, convert each curve:

- `Line(a, b)` → `Seg::Line(b)`;
- `Quad(a, m, b)` → `Seg::Cubic(a + ⅔(m − a), b + ⅔(m − b), b)` (exact);
- `Cubic(a, h1, h2, b)` → `Seg::Cubic(h1, h2, b)`;

scale by `size / units_per_em`, offset by the pen. The pen advances by
`h_advance + kern(this, next)`, scaled, plus `spacing` between letters. These
are the advances `measure` uses (AC2 is the test that keeps them the same).

`to_subpaths(map)` turns each contour into one closed `Subpath<Pt2>` through
`map` (the caller's px → view map), so the caller fills them as one
`Prim2::Path` with `Fill` and the R-0012 nonzero rule.

## 3. Code outline

```rust
// crates/motoreel-typeset/src/outline.rs — to be reviewed before any code lands
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Seg { Line((f64, f64)), Cubic((f64, f64), (f64, f64), (f64, f64)) }

#[derive(Clone, Debug, PartialEq)]
pub struct Contour { pub start: (f64, f64), pub segs: Vec<Seg> }

impl Contour {
    /// Every point and handle through `f` (exact for affine maps).
    pub fn map(&self, f: impl Fn((f64, f64)) -> (f64, f64)) -> Contour;
}

#[derive(Clone, Debug, PartialEq)]
pub struct Outline { pub contours: Vec<Contour>, pub advance: f64 }

impl Outline {
    pub fn to_subpaths(&self, map: impl Fn((f64, f64)) -> motoreel::Pt2) -> Vec<motoreel::Subpath<motoreel::Pt2>>;
}

pub fn outline(text: &str, face: FaceId, size: f64, spacing: f64, fonts: &Fonts) -> Result<Outline, Error>;
```

`to_subpaths` needs `motoreel`'s `Subpath`, and `motoreel` depends on this
crate under `text`: the method lives in `motoreel` instead
(`motoreel::text::outline_subpaths`, behind `text`), keeping the dependency
pointing one way. That is OQ-1 of this spec.

## 4. Non-goals

- Shaping, hinting, variable fonts; drawing `Prim2::Text` through outlines (R-0014 OQ-1).

## 5. Open questions

- OQ-1. `to_subpaths` in `motoreel` (behind `text`) or a `Subpath`-free
  `Contour` API only? Recommend the former: it is what every caller writes.

## 6. Acceptance criteria

- [ ] AC1 — `tests/r0014_outlines.rs::ac1_contours_are_closed_and_cubic`
- [ ] AC2 — `ac2_advance_is_the_measured_width`
- [ ] AC3 — `ac3_spacing_goes_between_letters`
- [ ] AC4 — `ac4_missing_glyph_is_an_error`
- [ ] AC5 — `ac5_affine_maps_are_exact`
- [ ] AC6 — `crates/motoreel/tests/r0014_outline_fill.rs` (after OQ-1)

The tests find a face the way the R-0009 gate does (`tests/spanish.rs`: the
same candidates on macOS and on the Ubuntu runner, and a failure, not a skip,
when none is found).

## 7. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| | | |

## Changelog

- 2026-10-14 — created.
