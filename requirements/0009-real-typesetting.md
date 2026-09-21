# R-0009 — Real typesetting

- **Status:** Draft
- **Milestone:** MC (the creator pipeline)
- **Owner:** Gustavo Delgadillo (westerngazoo)
- **Created:** 2026-09-20
- **Depends on:** R-0002 (`Shape`/`Prim2`), R-0006 (`PpmSink`), R-0007 (labels)
- **Amends:** **R-0007 AC8.** That criterion *requires* the ASCII-only
  behaviour this requirement removes. It is amended, not deleted: the
  guarantee «total, documented, never a panic» survives; what changes is
  that the total behaviour becomes an **error** instead of a `'?'`.
- **Realized by:** SPEC-0009 (to be written)
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

motoreel must be able to set **real type**: proportional faces with real
glyphs, real advances and real kerning, over the full Unicode range its
loaded faces cover — and it must be able to **measure** a run without
drawing it.

Two things follow, and the second is the one that matters long-term:

1. The engine can write Spanish. `bíceps`, `¿POR QUÉ TANTO?`, `30°`, `τ`.
2. Text acquires a **box model** — width, height above the baseline,
   depth below it — which is the substrate mathematical notation needs.
   Stage 1 uses one kind of box. Fractions, radicals and big operators
   are the same algebra applied to more of them.

## 2. Rationale

The engine cannot currently write the language of the channel it feeds.
Every accented character becomes `'?'`, so a reel that says «bíceps»
renders «b?ceps» and one that says «¿POR QUÉ TANTO?» renders
«?POR QU? TANTO?». That is not a cosmetic gap; it disqualifies the engine
from producing a single publishable piece, which is the whole point of
`guion-video-creator`.

Three findings shaped this requirement, and the first was a surprise:

**The substitution is not in the face.** It is `label::to_ascii`, applied
at eval, upstream of both sinks. So the SVG sink — which emits
`<text>` and lets the consumer's font engine do the work — is *already*
capable of Spanish and is being handed pre-destroyed text. Two separable
fixes hide behind one symptom.

**Monospace is load-bearing.** `PpmSink::draw_text` computes
`width = advance * text.len()`. A proportional face invalidates that
line, and measurement is precisely what a layout gate needs. So real type
and the ability to measure arrive together or not at all.

**`Prim2` has no fill.** `Style` is `{ stroke, width, alpha }`. A glyph is
coverage, not a stroke. The primitive vocabulary has to grow.

## 3. Acceptance criteria

- **AC1. Real faces.** A face is loaded from font bytes; glyph lookup,
  advance, kerning and vertical metrics come from the file, not a table.
- **AC2. Spanish renders.** `bíceps`, `¿POR QUÉ TANTO?`, `ángulo`, `30°`,
  `τ = F × L` set correctly in the raster sink, with no substitution.
- **AC3. Missing glyphs are an error, never a `'?'`.** A character the
  selected face does not cover fails loudly and names the character and
  the face. *This inverts R-0007 AC8 deliberately: the silent `'?'` is how
  «b?ceps» shipped without anyone noticing.*
- **AC4. Measurement is pure and separable.** A run can be measured to a
  box — width, height, depth — **without rasterizing**. No pixel buffer,
  no sink, no allocation of coverage.
- **AC5. The box model is a tree from the start.** Glyph, row, stack,
  glue and rule, each reporting the same three metrics. Stage 1 need only
  build rows of glyphs; the type must not need rewriting to hold a
  fraction.
- **AC6. Determinism survives.** R-0007 AC7 still holds: same scene and
  `t` ⇒ byte-identical frames. Rasterization is deterministic, with no
  dependence on iteration order or floating-point accumulation across
  runs.
- **AC7. The core stays thin.** The typesetter is its own crate. The
  `motoreel` crate's runtime dependency graph gains nothing; only the
  sink that rasterizes pays for the font machinery.
- **AC8. Licences travel.** Every embedded face ships its licence file,
  and a test fails if a face is embedded without one.

## 4. Constraints & non-goals

- **Not shaping.** No bidi, no Indic reordering, no Arabic joining.
  Spanish and mathematical notation need glyph lookup plus kerning, and
  nothing here should pretend otherwise.
- **Not a TeX.** Mathematical *layout* — fractions, radicals, limits — is
  a later requirement. This one only guarantees the box model can hold it.
- **Not LaTeX.** No shelling out. (Manim does; that is a dependency this
  project should not take on for a fifteen-construct subset.)
- Line breaking is out of scope: a label is one line.

## 5. Open questions

- **OQ-1. The mono face cannot be the one we use today.** The Python
  factory reads `/System/Library/Fonts/Menlo.ttc` — Apple's, not
  redistributable. Bangers, ComicNeue-Bold and LuckiestGuy are OFL and
  can ship. **Consequence:** ENCARGO §7.3 asks that a reel from the new
  engine be *indistinguishable* from the published one. With a different
  monospace it will not be. Either the brand adopts an OFL mono — and the
  Python factory adopts it too, so the two agree — or §7.3 is relaxed to
  "indistinguishable but for the mono face". **Owner decision.**
- **OQ-2. Which rasterizer.** `ab_glyph` (rasterization included, three
  crates) or `ttf-parser` (parsing and outlines only, zero dependencies,
  and we write the scanline fill). The house style leans to hand-written
  readers; antialiasing quality leans the other way. Recommendation:
  `ab_glyph` for stage 1, precisely because the point of stage 1 is to
  learn whether this is weeks or months.
- **OQ-3.** Does the SVG sink keep emitting `<text>` — letting the
  consumer's font engine do the work, at the cost of the two sinks no
  longer being byte-comparable — or does it emit outlines so both sinks
  agree? AC6 is about each sink being deterministic, not about the two
  matching, so either answer is admissible.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-09-20 | Own crate, not a module | `motoreel` depends on `garust` alone today. A font rasterizer in the core would be inherited by every consumer, wasm lessons included. In its own crate only the rasterizing sink pays |
| 2026-09-20 | Box tree from day one, though stage 1 builds only rows | Designing "a line of text" now means rewriting it for the first fraction. Width/height/depth is TeX's model and it holds both |
| 2026-09-20 | Missing glyph is an error, amending R-0007 AC8 | The silent `'?'` is the defect's own camouflage. It shipped «b?ceps» and nothing complained |
