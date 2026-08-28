# R-0007 — Anchored text labels

- **Status:** Draft
- **Milestone:** MC (the creator pipeline)
- **Owner:** Gustavo Delgadillo (westerngazoo)
- **Created:** 2026-08-27
- **Depends on:** R-0002 (`Shape`/`Prim2`), R-0003 (`SvgSink`), R-0006 (`PpmSink`)
- **Realized by:** SPEC-0007
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

motoreel must be able to **name things on screen**: short plain-text
labels anchored to the scene, so a creator can identify an axis, a
quantity, or a moving body. An anchor is one of:

- a **world point** — the label tracks that point through the camera;
- a **body's pose** — the label rides an object's track with an offset,
  so it follows a moving thing;
- a **screen corner** — for titles and captions that must not move.

Labels are projected, culled, and emitted like any other primitive, and
render in **both** sinks.

## 2. Rationale

Text was an explicit non-goal, reversed by owner decision on 2026-08-27
because the project acquired an audience. An explanatory video that
cannot label an axis is not an explainer — it is an animation. This is
the single highest-value capability for STEM creators, and the narrow
scope below keeps it from becoming a typesetting project.

## 3. Acceptance criteria

- **AC1.** A `Label` carries its string, an `Anchor`, an offset in image
  units, an alignment, and a `Style`; a scene may hold labels alongside
  objects.
- **AC2.** A point-anchored label projects through the camera exactly as
  geometry does, obeying the same whole-primitive cull rule (behind the
  camera ⇒ the label is absent, never NaN-positioned).
- **AC3.** A pose-anchored label follows its object's track: at any `t`
  its position equals the object's pose applied to the anchor point,
  then projected — asserted against an independently computed position.
- **AC4.** A corner-anchored label sits at a fixed image-space position
  regardless of camera or time.
- **AC5.** `SvgSink` emits one pinned `<text>` element per label, with
  the byte-determinism rules of R-0003 (fixed attribute order, default
  float `Display`, no value-dependent branching), and the existing
  golden fixture stays byte-unchanged.
- **AC6.** `PpmSink` rasterizes labels from an in-crate bitmap font, at
  the same anchor positions, deterministically.
- **AC7.** Determinism end to end: same scene and `t` ⇒ byte-identical
  frames from both sinks.
- **AC8.** Text is ASCII-only, and the behaviour for non-ASCII input is
  documented and total (no panic, no silent corruption).

## 4. Constraints & non-goals

- **No layout engine**: no wrapping, no bidi, no kerning pairs, no
  shaping. One string, one line, one anchor.
- **No LaTeX, no rich text, no markup** — those stay out of scope, and
  their absence is documented rather than hidden.
- No font loading: the raster sink carries one embedded bitmap face;
  the SVG sink names a font family and lets the renderer resolve it.
  The two sinks are therefore *not* pixel-identical for text, which is
  stated rather than papered over.
- No collision avoidance between labels; placement is the author's job.

## 5. Open questions

None — scope settled with the owner on 2026-08-27.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-08-27 | Text is in scope, reversing a documented non-goal | The project has an audience now; an explainer must be able to name what it shows (owner) |
| 2026-08-27 | Plain strings, anchored to geometry — no markup, no LaTeX | Covers axis names, quantities and callouts, which is the whole ask; a typesetting engine is a different project (owner) |
| 2026-08-27 | Three anchor kinds: world point, body pose, screen corner | The pose anchor is the one that makes labels *ride* moving objects — the capability an explainer actually needs (owner) |
| 2026-08-27 | Sinks may differ in glyph rendering, and say so | The raster sink needs an embedded face; the SVG sink should use real fonts. Pretending they match would be the dishonest option (owner) |

## Changelog

- 2026-08-27 — created; shaped with the owner the same day.
