# R-0014 — Glyph outlines: a line of text as closed Bézier contours

- **Status:** Discussing — requirement, spec and red tests written; code outline awaiting the owner (CLAUDE.md §4 step 4)
- **Milestone:** MC (the creator pipeline)
- **Owner:** Gustavo Delgadillo (westerngazoo)
- **Created:** 2026-10-14
- **Depends on:** R-0009 (faces, `measure`), R-0012 (paths and nonzero fills)
- **Realized by:** [SPEC-0014](../specs/0014-glyph-outlines.md)
- **QA:** `qa` agent run scoped to this requirement

> Numbering: R-0013 (glyph coverage) is drafted on its own branch and not yet
> on `main`; this is the next free integer after it.

## 1. Statement

`motoreel-typeset` must give a line of text as **contours**: the closed
outlines of its glyphs, each a start point and a sequence of line and cubic
Bézier segments, placed with the same advances and kerning as `measure`, with
optional extra spacing between letters. The contours must be exact under any
affine map (rotate, scale, mirror a word) and must fill as an R-0012 path with
the nonzero rule.

## 2. Rationale

motoreel draws text one horizontal line per primitive. A creator who needs a
word on a ring, a mirrored letter, tracking, or a glyph that morphs needs the
outline itself. The sibling `rotorf-sico` studio built exactly this
(`mathematicus/motor/src/texto.rs`, `Letras`, 2026-10-06) on the same
`ab_glyph` this crate uses, and found a second reason: drawing every glyph as
one nonzero path is the exact fix for the overlap flashes of R-0013 (its
option B). Every text in that studio's second account is drawn this way, and
none flashes. The owner decided (rotorf-sico `docs/decisions.md`, 2026-10-06 and
2026-10-14) that this belongs here, through this process.

## 3. Acceptance criteria

- **AC1. Contours.** `outline(text, face, size, fonts)` returns every
  glyph's closed contours (TrueType quadratics raised to cubics exactly: the
  handles at 2/3 of the control point) and the total advance, with the pen at
  x = 0 and the baseline at y = 0, y up (motoreel's convention, as `place`).
- **AC2. Same metrics as `measure`.** With no extra spacing, the advance
  equals the width `measure` reports for the same text, face and size, and
  each glyph's contours start where `place` puts that glyph.
- **AC3. Tracking.** `spacing` adds that much between consecutive letters
  and none after the last.
- **AC4. Missing glyphs are errors.** A character the face lacks is
  `Error::Missing`, as in `measure` (R-0009 AC3).
- **AC5. Affine exactness.** `Contour::map(f)` maps every point and handle
  through `f`; for an affine `f` the result is the outline of the mapped
  glyph (tested by mapping twice and inverting).
- **AC6. Paths.** `Outline::to_subpaths(map)` gives R-0012 subpaths (one
  closed subpath per contour) that `Prim2::Path` fills with the nonzero rule;
  on a face whose contours overlap, no filled pixel exceeds the ink.

## 4. Constraints & non-goals

- No new dependency: `ab_glyph` is already the crate's only one.
- `motoreel` keeps its garust-only graph without `text` (R-0009 AC7).
- Not here: shaping (ligatures, bidi), hinting, variable-font axes, and the
  choice between R-0013's clamp and this path for ordinary text (R-0013 OQ-1).

## 5. Open questions

- OQ-1. Should `PpmSink` draw `Prim2::Text` through outlines by default once
  this lands (R-0013 option B), changing every text pixel's bytes? Recommend:
  no; keep it opt-in, and let R-0013's clamp fix the flashes for plain text.
- OQ-2. Coordinates in em units or in the caller's size? Recommend the
  caller's size (as `measure`), so the result composes with `place`.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| | | |

## Changelog

- 2026-10-14 — created (Discussing) with SPEC-0014 and the red tests
  (`crates/motoreel-typeset/tests/r0014_outlines.rs`, behind the empty
  feature `r0014` until the code outline is signed).
