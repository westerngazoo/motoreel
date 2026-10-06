# R-0013 — Glyph coverage is a fraction of a pixel

- **Status:** Discussing — red tests committed; code outline awaiting the owner (CLAUDE.md §4 step 4)
- **Milestone:** MC (the creator pipeline)
- **Owner:** Gustavo Delgadillo (westerngazoo)
- **Created:** 2026-10-06
- **Depends on:** R-0009 (real typesetting, `motoreel-typeset::rasterize`), R-0006 (`PpmSink` blend)
- **Realized by:** [SPEC-0013](../specs/0013-glyph-coverage.md)
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

The coverage the typesetter's rasterizer reports for a pixel must be a
fraction of that pixel: between 0 and 1, whatever the contours of the face
look like. The raster sink must never paint a text pixel past the text's
colour.

## 2. Rationale

`ab_glyph` accumulates the signed area of every contour under a pixel and
reports its absolute value, unclamped. Where two contours of one glyph
overlap with the same winding — common at stroke junctions in shipped faces —
the sum reaches 2. `rasterize` hands that on unchanged, and `PpmSink`
blends with `alpha · c`, so a coverage of 2 extrapolates past the ink:
on black, a (200, 100, 50) stroke came out (255, 200, 100).

Found in production. The `rotorf-sico` reels set JetBrains Mono 700, whose
'g' and 'r' junctions overlap, and the `designer` review measured flashes up
to (255, 255, 87) on a gold that should top out at (227, 169, 58). They
survived H.264. The reels worked around it by drawing every glyph as
contours through their own nonzero fill. The defect belongs here: any
creator who sets one of these faces gets the flashes.

## 3. Acceptance criteria

- **AC1.** No coverage emitted by `rasterize` exceeds 1, and a glyph's
  solid inside is still exactly 1.
- **AC2.** On `PpmSink`, every channel of a text pixel lies between the
  background's and the ink's. On black, no channel exceeds the ink's.
- **AC3.** Where a glyph's contours do not overlap, its coverage is
  bit-identical to before. Every existing golden and test is unchanged.

## 4. Constraints & non-goals

- `motoreel` keeps its garust-only graph without the `text` feature
  (R-0009 AC7). No new dependency.
- Not here: exact union coverage on antialiased edges inside an overlap
  (SPEC-0013 §2, option B, and OQ-1), or new text features.

## 5. Open questions

- **OQ-1. Clamp (A) or nonzero outline fill (B)?**
  - **A** clamps the coverage where it is produced (one line). Every
    over-bright pixel is gone. Along an antialiased edge that runs through
    an overlap, the coverage is min(2f, 1) instead of the union's f: an
    edge pixel there can read up to twice as dense, but never past the ink.
  - **B** turns each glyph into an R-0012 path and fills it with the
    nonzero rule the sink already has. That gives the union exactly, at the
    cost of a second raster path for text and every text pixel's bytes.
  - **Recommendation:** A now, as this requirement. B later, as its own
    requirement, if an edge is ever seen wrong.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| | | |

## Changelog

- 2026-10-06 — created (Discussing) with SPEC-0013 and the red tests.
