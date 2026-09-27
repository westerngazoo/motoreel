# R-0012 — Paths and fills

- **Status:** **Accepted** (2026-09-27, owner): all four open questions settled on the recommendations
- **Milestone:** MC (the creator pipeline)
- **Owner:** Gustavo Delgadillo (westerngazoo)
- **Created:** 2026-09-27
- **Depends on:** R-0002 (`Shape`/`Prim2`, projection, cull), R-0003
  (`SvgSink`, golden scheme), R-0006 (`PpmSink`, coverage rule, determinism)
- **Amends:** R-0006 §4, which lists "no fills" as a non-goal. The
  amendment is additive: every existing primitive, coverage rule and golden
  byte stays as it is.
- **Realized by:** SPEC-0012 (to be written)
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

motoreel must be able to draw **curved outlines** and **filled regions**.

Concretely:

1. **A path primitive.** An outline is made of straight and **cubic Bézier**
   pieces and may be open or closed. It can be *stroked* (as every primitive
   is today), *filled*, or both.
2. **Fills by the nonzero winding rule**, with an opacity, anti-aliased by a
   documented coverage rule. Both sinks render them, and both place the same
   boundary.
3. **Shape generators**, pure functions that emit paths:
   - circle
   - circular arc (open)
   - circular sector (closed: pie slice)
   - rounded rectangle
   - closed polygon

   These cover the whole vocabulary of an explanatory card: panels, dots,
   gauges, the friction circle, and the area under a curve.

The engine's instruction set grows from "stroke these segments" to
"MoveTo / LineTo / CubicTo / Close, then stroke and/or fill". That is the
instruction set Manim is built on. Anything drawable is some sequence of
those four commands.

## 2. Rationale

**The engine cannot paint an area.** `Style` is `{ stroke, width, alpha }`,
and every primitive rasterizes as a union of round-capped segments
(SPEC-0006 §2.4). The rasterizer works like a pen plotter, and it cannot
draw any of the following:

- **Filled shapes.** A dot larger than its stroke, a card behind a formula,
  a tyre contact patch.
- **Areas that carry meaning.** In physics-lab RFC-002 "the area IS the
  work", so its gym lessons are blocked on a filled polygon (their `tag 6`).
  mechanicars' first reels are blocked on the interior of the **friction
  circle** (Kart Lab #3) and on the shaded area under a P-V or Δt(s) curve.
- **Curves drawn as curves.** Today a circle is a polyline the author
  sampled by hand. Its smoothness depends on a vertex count the author had
  to guess, it changes with zoom, and it looks faceted under an orthographic
  close-up.

Three findings shaped this requirement:

**The existing rasterizer already contains half of a fill.** Its coverage
rule is a one-pixel ramp on *distance to the boundary*
(`coverage = clamp(r + 0.5 − d, 0, 1)`). A fill is the same ramp on the
**signed** distance to the outline: inside is positive, outside is
negative, and `coverage = clamp(0.5 + s, 0, 1)`. The only new ingredient is
the *sign*, which is an **inside test**. The nonzero winding number supplies
it: a signed count of edge crossings, the way a quadrature encoder counts
signed transitions. Built this way, the fill and the stroke agree
pixel-for-pixel on where the boundary is, which a scanline fill with a
separate area rule would not guarantee.

**The circle can be trig-free.** A quarter-circle cubic has handle length
`k = 4(√2 − 1)/3 · R ≈ 0.5522847 R`, and `√` is correctly rounded. A circle
built from four such quarters is therefore bit-portable. That keeps the
golden-fixture discipline (SPEC-0006 §2.7, point 5) intact for the shape
that will appear in almost every reel. Its maximum radial error is about
`2.7 × 10⁻⁴ R`, which is 0.13 px at R = 500 px and below the one-pixel ramp.
Arcs of arbitrary angle need `tan(θ/4)`, which is a transcendental, so they
are admitted with the same "trig lives in the scene" caveat R-0003 already
carries.

**Béziers survive the orthographic camera but not the pinhole one.** Cubic
Béziers are *affine*-invariant: posing a curve means transforming its four
control points, and the result is exactly the posed curve. The orthographic
projection is affine, so a path projects by projecting its control points,
exactly. The pinhole projection is *projective*, and a projected cubic is
not the cubic of the projected control points. The requirement therefore
has to say what "correct" means under a pinhole camera (AC4, OQ-1).

## 3. Acceptance criteria

- **AC1. Path shape.** A new `Shape` variant describes a path in model space
  as an ordered list of subpaths. Each subpath is a start point followed by
  line and cubic segments, and is optionally closed.
  - It is posed by its object's motor track like every other shape.
  - It is carried to the sinks as a new `Prim2` variant.
  - Existing variants are unchanged.
  - The `Prim2` match in both sinks stays exhaustive with no `_` arm.

- **AC2. Fill paint.** An object can carry a **fill** (colour and opacity)
  independently of its stroke: fill only, stroke only, or both. When both
  are present:
  - the fill is composited first and the stroke second, each with its own
    opacity, in the order SVG paints them;
  - each is composited **exactly once** per pixel per primitive, preserving
    SPEC-0006 §2.5's invariant;
  - an object with neither paints nothing.

- **AC3. Nonzero winding, matching SVG.** Fill membership follows SVG's
  `fill-rule="nonzero"`. The following hold in both sinks:
  - a pentagram drawn as one self-intersecting closed subpath fills its
    central pentagon;
  - a square inside a square with the **same** orientation is filled
    solid;
  - with the **opposite** orientation the inner square is a hole;
  - an *open* subpath is filled as though closed by a straight segment,
    which is SVG's behaviour.

- **AC4. Geometry agreement, with the pinhole case stated.**
  - Under the **orthographic** camera, the curve rendered is the exact
    image of the model-space curve, up to the flattening tolerance of AC6.
  - Under the **pinhole** camera, the rendered outline lies within the
    flattening tolerance of the true perspective image of the model-space
    curve. It does *not* lie within tolerance of the cubic through the
    projected control points.
  - **Cull rule:** a primitive with any control point behind the camera, at
    the camera plane, or non-finite is culled *whole*, as R-0002 specifies.

- **AC5. The fill coverage rule is pinned and consistent with strokes.**
  Coverage is a closed-form function of each pixel's centre and the
  outline, with no accumulation across pixels. The spec pins its exact
  form; SPEC-0006 §2.7, point 3 still holds. The following are **exact**
  identities:
  - an axis-aligned filled rectangle whose edges lie on pixel boundaries
    paints its interior pixels at coverage exactly `1` and its exterior
    exactly `0`, with no fringe;
  - an edge through pixel centres paints that column at coverage exactly
    `0.5` (byte `round(127.5) = 128` for white on black), matching the
    stroke table in SPEC-0006 §2.5;
  - for an axis-aligned rectangle at any sub-pixel offset, the summed
    coverage equals the rectangle's area in px² within a per-corner
    tolerance that the spec derives and pins.

- **AC6. Curves are flattened to a stated tolerance.**
  - Wherever the rasterizer approximates a cubic by chords, the maximum
    deviation between chord and curve is **≤ 0.1 px**. The spec derives
    the chord count per segment from that bound.
  - A stroked circle of radius R px and stroke radius r ≥ 1 px lights only
    pixels whose centres lie within `R ± (r + 0.5 + 0.1)` px of the true
    centre, and lights every pixel within `R ± (r − 0.5)`.
  - Chord counts are a pure function of the segment's pixel-space control
    points, so they introduce no nondeterminism.

- **AC7. Shape generators.** Pure functions return paths for:
  - circle (centre, radius)
  - arc (centre, radius, start and sweep angles)
  - sector (the same arguments, closed through the centre)
  - rounded rectangle (centre, width, height, corner radius)
  - closed polygon (vertices)

  Required properties:
  - **The circle is trig-free.** It is four cubics with handle length
    `4(√2 − 1)/3 · R`, pinned as a golden constant. Its maximum radial
    deviation is `≤ 3 × 10⁻⁴ R`, measured by dense sampling.
  - **Arc endpoints are exact.** They lie on the circle up to the rounding
    of `cos`/`sin`.
  - **Rounded-rectangle corner radius is clamped** to half the shorter
    side; a radius of zero yields a plain rectangle.
  - **Degenerate inputs never panic.** A zero radius, zero sweep or fewer
    than 3 polygon vertices returns an empty path or a documented
    degenerate form.

- **AC8. `SvgSink` renders paths.**
  - It emits `<path d="…">` using the absolute commands `M`, `L`, `C`
    and `Z`, with `fill`, `fill-opacity` and `fill-rule="nonzero"`, plus the
    existing stroke attributes. A stroke-only path gets `fill="none"`.
  - Number formatting follows SPEC-0003's rule, so output stays
    byte-deterministic.
  - PPM and SVG place a fill boundary within **±1 px** of each other in the
    same regime R-0006 AC2 covers.

- **AC9. Determinism, as R-0006 AC3.**
  - Two renders of the same scene produce byte-identical files in both
    sinks.
  - A checked-in **trig-free** golden frame per sink matches byte-for-byte.
    It contains a filled circle, a filled-and-stroked rounded rectangle
    with corner radius 0 or built from quarter circles, and a nonzero-rule
    hole.
  - No transcendental, `mul_add` or cross-pixel accumulation enters the
    raster path.

- **AC10. Nothing already rendered moves.**
  - Every existing golden fixture is **byte-unchanged**: R-0003, R-0006,
    and R-0007/R-0009 where they exist.
  - The existing test suites pass unmodified, except for mechanical
    edits that the `Style` change (OQ-3) forces on struct literals. Those
    edits are listed in the PR.

- **AC11. The kernel stays thin.**
  - Zero new dependencies.
  - `cargo build -p motoreel --no-default-features` still builds, lints
    and tests.
  - Fills and paths live in the core, not behind the `text` feature.

- **AC12. A demo a creator can run.** `cargo run --example card` renders
  a short vertical clip that encodes with the documented stock-ffmpeg
  command. It shows:
  - a filled panel with a hairline border, the explanatory-card layout;
  - a friction circle, filled at low opacity and stroked;
  - a point moving on the circle;
  - a shaded area under a curve.

  Colours are passed in by the example and are not built into the engine.

## 4. Constraints & non-goals

- **Not gradients.** Fills are one flat colour plus opacity. Linear and
  radial gradients are a later requirement: the area in the reference card
  is a yellow→green gradient, and the flat version is accepted for now.
- **Not even-odd.** SVG's other fill rule is not offered. Holes are made
  by orientation, which the nonzero rule already expresses.
- **Not reveal or morph animations.** Drawing a path progressively (Manim's
  `Create` / `Write`) and interpolating one path into another (`Transform`)
  are the next requirement. This one only guarantees that a path is a
  parameterised curve list that a later requirement can truncate by de
  Casteljau subdivision or align point-for-point, so they need no new
  representation.
- **Not text as outlines.** Glyphs stay with R-0009. Emitting glyph
  outlines as paths, so that `Write` works on text, is a later question and
  touches R-0009 OQ-3.
- **Not stroke joins other than round.** Stroked paths keep SPEC-0006's
  round caps and joins; miter and bevel are out.
- **Not a GPU path.** The raster sink stays a CPU, in-crate, deterministic
  rasterizer.
- **Not layout.** Placing a card next to a formula (`next_to`, `arrange`)
  is the job of the declarative layer (guion), not of the engine.

## 5. Open questions

- **OQ-1. Where does flattening happen?**
  - *Option A (recommended).* `Prim2::Path` carries image-space cubics.
    Under the orthographic camera, eval projects control points, which is
    exact. Under the pinhole camera, eval subdivides each cubic in *view
    space* until each piece's projection is within tolerance, then emits
    lines. The sink flattens to pixel tolerance.
  - *Option B.* Eval always flattens to lines, and `Prim2::Path` holds
    polylines only. This is simpler, but the SVG output loses its curves
    and the tolerance has to be chosen before the pixel scale is known.
- **OQ-2. Which fill coverage rule?**
  - *Option A (recommended).* Signed distance to the flattened outline,
    through the existing ramp: `clamp(0.5 + s, 0, 1)`, with the sign taken
    from the nonzero winding number at the pixel centre. It is closed-form
    per pixel, agrees with the stroke ramp on the boundary and keeps §2.7
    intact. Its cost is an error at sharp corners, which AC5 bounds.
  - *Option B.* Exact area coverage by a signed-area accumulation buffer,
    the font-rasterizer technique. It is more accurate at corners, but it
    is an accumulation across pixels, which SPEC-0006 §2.7, point 3
    forbids, so adopting it means amending that argument.
- **OQ-3. How does fill enter `Style`?**
  - *Option A (recommended).* Add `fill: Option<Fill>` to `Style`, with
    `Fill { colour, alpha }`. This breaks struct-literal construction:
    36 sites in motoreel (mostly tests) and 5 in guion-video-creator, all mechanical.
  - *Option B.* Put the fill on `Object` and `Prim2::Path` only. That
    avoids the churn, but only paths could ever be filled, and a filled
    `Point` (a dot) would need a circle path instead.
- **OQ-4.** Should `Shape::Point` with a fill mean "a disc of radius
  `width/2`"? It already renders as one; the recommendation is to leave it
  unchanged and make dots through the circle generator.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-09-27 | **OQ-1:** `Prim2` carries image-space cubics. Orthographic eval projects control points; pinhole eval subdivides in view space until each piece projects within tolerance | Affine maps preserve Béziers exactly, so the common case loses nothing, and the SVG keeps real curves. The projective case is honest about its tolerance instead of silently wrong (owner) |
| 2026-09-27 | **OQ-2:** fill coverage is the existing one-pixel ramp on the signed distance to the outline, with the sign from the nonzero winding number at the pixel centre | Closed-form per pixel, so SPEC-0006 §2.7's determinism argument holds unamended, and fill and stroke agree on where the boundary is. Corner error is bounded by AC5 rather than eliminated (owner) |
| 2026-09-27 | **OQ-3:** `Style` gains `fill: Option<Fill>` | Fill lives in the one style every primitive already carries, so it can be honoured on more variants later without another type change. SPEC-0012 honours it on `Path`. The churn is mechanical: 36 literal sites here (mostly tests) and 5 in guion-video-creator (owner) |
| 2026-09-27 | **OQ-4:** `Shape::Point` is unchanged; filled dots come from the circle generator | A point stays a point. The generator is trig-free and exact, so the dot costs nothing extra (owner) |

## Changelog

- 2026-09-27: created as a Draft for discussion with the owner.
- 2026-09-27: **Accepted**. OQ-1 to OQ-4 settled on the recommendations; §5 is kept for the record.
