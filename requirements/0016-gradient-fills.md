# R-0016 — Gradient fills

- **Status:** **Accepted** (2026-11-18, owner: «sí, dale al degradado en motoreel»). The decisions in §6 are Claude's recommendations, taken to implementation under that go-ahead; the owner may overrule any of them in the PR review
- **Milestone:** MC (the creator pipeline)
- **Owner:** Gustavo Delgadillo (westerngazoo)
- **Created:** 2026-11-18
- **Depends on:** R-0012 (`Fill`, `Style::fill`, `Prim2::Path`, the fill
  coverage rule), R-0006 (`PpmSink`, determinism), R-0003 (`SvgSink`,
  number formatting, golden scheme)
- **Amends:** R-0012 §4, whose first non-goal reads "Not gradients". The
  amendment is additive: a flat fill renders byte-for-byte as it does today.
- **Realized by:** [SPEC-0016](../specs/0016-gradient-fills.md)
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

motoreel must be able to fill an area with a colour that **varies across
it**: a **linear** gradient along an axis and a **radial** gradient out from
a centre, each defined by 2 to 8 colour stops, and render it in both sinks
without visible banding in the raster one.

Concretely:

1. **A fill's paint is flat or a gradient.** `Fill::solid` keeps its
   signature and its meaning. A gradient is built by a validating
   constructor that returns a typed error for bad input.
2. **The gradient rides its object.** Its geometry is authored in the
   object's model plane, next to the path it paints, and is posed and
   projected with it.
3. **The raster sink does not band.** A slow ramp quantised to 8 bits
   shows steps; the raster sink breaks them up with a deterministic
   ordered dither, so the same scene still gives the same bytes.

## 2. Rationale

The house adopted a colourful «juguete + rayos X» reel style (2026-11):
soft shading on toy capsules, a warm spotlight cone on a stage, and a
vignette. With flat fills the only way to approximate a soft falloff is a
stack of translucent shapes, and the result is visibly stepped. R-0012
already called this out: "the area in the reference card is a
yellow→green gradient, and the flat version is accepted for now".
`Fill::solid` was introduced then precisely so `Fill` could grow without
breaking literals again.

Three findings shaped this requirement:

**8 bits band even with an exact ramp.** A dark warm ramp that changes
by 8 code values across 1000 px holds each value for 125 px. The eye sees
those edges (Mach bands) in a full-screen falloff, and H.264 at reel
bitrates makes them worse, not better. Interpolating more precisely does
not help: the quantisation to 8 bits is the step. Dithering trades the
step for fine noise below one code value.

**Only an ordered dither keeps SPEC-0006 §2.7.** Error diffusion
(Floyd–Steinberg) carries each pixel's error to its neighbours, which is
the cross-pixel accumulation SPEC-0006 §2.7 point 3 forbids and makes the
result depend on visiting order. Random noise needs a seeded generator and
shimmers from frame to frame. An ordered (Bayer) threshold is a closed form
of the pixel's coordinates: deterministic, order-free and the same on
every frame, so a static gradient stays static in the video.

**SVG already has the model.** `<linearGradient>` and `<radialGradient>`
with `gradientUnits="userSpaceOnUse"`, a `gradientTransform` and `pad`
spread express exactly what this requirement asks for, including a
gradient posed by an arbitrary affine map. The SVG sink therefore emits
native elements rather than approximating.

## 3. Acceptance criteria

- **AC1. The paint model.**
  - `Fill` holds a paint and an opacity. The paint is either one flat
    colour or a gradient. `Fill::solid(colour, alpha)` is unchanged in
    signature and meaning, and `Fill`, `Style` and the gradient stay plain
    `Copy` data.
  - A gradient is **linear** (from `p0` to `p1`: offset 0 at `p0`, 1 at
    `p1`, constant along lines perpendicular to the axis) or **radial**
    (offset 0 at a centre, 1 on the circle of a radius, constant on
    concentric circles).
  - It holds **2 to 8** stops, each an offset in `[0, 1]`, a colour and an
    opacity in `[0, 1]`, with offsets non-decreasing.
  - Spread is **pad**: beyond offset 0 the paint is the first stop, beyond
    offset 1 the last.

- **AC2. Invalid input is a typed error.** The constructors reject, each
  with its own error variant naming the offending stop where there is one:
  - fewer than 2 or more than 8 stops;
  - an offset that is non-finite or outside `[0, 1]`;
  - an offset smaller than its predecessor's;
  - a stop opacity that is non-finite or outside `[0, 1]`;
  - a non-finite point or radius;
  - a linear gradient with `p0 == p1`;
  - a radial gradient with radius ≤ 0.

  An invalid gradient is unrepresentable: no public path builds one.

- **AC3. Interpolation is pinned.**
  - Between two stops, each colour channel and the opacity are linear in
    the offset, in **sRGB-encoded** values, unpremultiplied.
  - At offset 0 the paint is the first stop and at offset 1 the last,
    **exactly** (equality of `f64`, not a tolerance).
  - Two stops at the same offset make a hard edge; at that offset the
    later stop wins, as in SVG.

- **AC4. The gradient rides its object.**
  - Gradient geometry is authored in the model plane `z = 0`, the plane
    `Object::planar` lifts shapes into. `Scene::eval` carries it into
    image space with the object's pose and the camera, so translating,
    rotating or zooming an object moves its gradient with it.
  - Under the orthographic camera the image-space gradient is the exact
    image of the model-space one.
  - Under the pinhole camera it is the affine map through the projections
    of its three frame points. This is stated, not hidden: perspective-
    correct gradients are out of scope.
  - If a frame point fails to project, the primitive is culled whole,
    under R-0002's rule.

- **AC5. The raster sink paints the gradient.**
  - Each pixel's paint is evaluated at its centre, then composited with the
    fill's coverage from R-0012, once per pixel.
  - A pixel whose centre lies at offset 0 or 1 of an opaque gradient over
    any background takes the stop's colour **exactly**; so does every
    pixel in the pad region.
  - The radial centre takes the first stop and a pixel centred on the
    radius the last.

- **AC6. Nothing already rendered moves.**
  - Every existing golden fixture is byte-unchanged; existing suites pass
    with only the mechanical edits the `Fill` change forces, listed in the
    PR.
  - A gradient whose stops are all identical renders **byte-identically**
    to `Fill::solid` with that colour, in the raster sink.

- **AC7. The raster sink does not band.**
  - Gradient fills are dithered by a deterministic ordered threshold that
    depends on the pixel's coordinates only. Flat fills, strokes and text
    are not dithered.
  - On a ramp whose undithered quantisation steps are ≥ 100 px wide, no
    run of identical 8-pixel-tall columns along the ramp is longer than
    **8 px**, one dither period.
  - The dither is unbiased: the mean of each aligned 8 × 8 block is within
    0.5 code values of the undithered composite's mean.

- **AC8. Determinism.**
  - Two renders of the same scene give byte-identical files in both
    sinks, and so do renders of one scene on several threads at once.
  - Only `+ − × ÷`, `floor` and a correctly rounded `sqrt` enter the
    gradient's raster path: no transcendental and no `mul_add`.
  - A checked-in trig-free golden frame per sink, containing a linear and
    a radial gradient, matches byte-for-byte.

- **AC9. The SVG sink emits native gradients.**
  - A gradient fill writes one `<linearGradient>` or `<radialGradient>`
    element immediately before its `<path>`, with
    `gradientUnits="userSpaceOnUse"`, the pose as `gradientTransform`,
    `spreadMethod="pad"` and one `<stop>` per stop, and the path refers to
    it as `fill="url(#…)"`.
  - Ids are stable: numbered in draw order from 0 within each frame, so
    the output is byte-deterministic.
  - The fill opacity stays `fill-opacity`, so the two sinks multiply the
    same opacities.

- **AC10. The kernel stays thin.** Zero new dependencies; `cargo build -p
  motoreel --no-default-features` still builds, lints and tests; gradients
  live in the core, not behind a feature.

## 4. Constraints & non-goals

- **Not a focal point.** SVG's `fx`/`fy` (an off-centre radial focus) is
  not offered. A spotlight is a radial or linear ramp clipped by its cone
  path; a focus can be added later without changing the model.
- **Not reflect or repeat spread.** Pad is the only spread: no reel in the
  house style needs a repeating gradient, and every renderer agrees on pad.
- **Not linear-light interpolation.** See §6; it would need a
  transcendental in the raster path and would disagree with SVG renderers.
- **Not perspective-correct gradients** under the pinhole camera (AC4).
- **Not gradient strokes or gradient text.** Only fills take a gradient;
  `Style::stroke` stays a flat colour.
- **Not dithering of flat paint.** It would move every golden byte and
  there is no ramp to hide.
- **Not bloom, additive blending, arbitrary clip paths or isometric
  projection.** Each is a separate, later requirement.

## 5. Open questions

None outstanding; §6 records how each was settled.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-11-18 | **Paint model:** `Fill { paint: Paint, alpha }` with `Paint::{Flat(Rgb), Gradient(Gradient)}`; `Gradient` has private fields and validating constructors | Mirrors SVG (`fill` paint + `fill-opacity`), so the opacity guard stays shared. Private fields make an invalid gradient unrepresentable. Breaks direct field access to `Fill::colour`: 2 test sites here and 1 in guion-video-creator's `guion-marca` tests, all mechanical |
| 2026-11-18 | **Stops in a fixed array of 8**, not a `Vec` | Keeps `Fill` and `Style` `Copy`, which every sink and dozens of call sites rely on. Eight covers every falloff the style guide draws (two or three stops in practice) |
| 2026-11-18 | **Interpolation in sRGB-encoded values, unpremultiplied** | It is SVG's default `color-interpolation` and what browsers implement, so the two sinks agree; the compositing is already sRGB (SPEC-0006 §2.5). Linear-light would need `powf(2.4)` per pixel, a transcendental banned from the raster path, or a LUT of decoded values. Unpremultiplied makes equal stops exactly the flat colour. The cost: a stop that fades to transparent should keep its colour (author `GOLD@1 → GOLD@0`, not `GOLD@1 → BLACK@0`); renderers that interpolate premultiplied would otherwise differ |
| 2026-11-18 | **Geometry in the model plane, carried as a three-point frame** (origin, x-end, y-end) | One representation for both kinds and every pose: eval maps three points the way it maps control points, and SVG's `gradientTransform` takes the same frame as a matrix. Exact under the orthographic camera |
| 2026-11-18 | **Dither: 8 × 8 Bayer ordered threshold**, `floor(v + (B[y mod 8][x mod 8] + ½)/64)`, gradient fills only | Closed form of (x, y): no accumulation, no visiting order, no seed, no frame index, so identical across runs, threads and frames. Unbiased (thresholds average ½). An exact integer stays exact, so opaque stops and pad keep their colours. Rejected: error diffusion (cross-pixel accumulation), seeded noise (temporal shimmer), blue-noise texture (a 64 × 64 table for a marginal gain) |
| 2026-11-18 | **A gradient whose stops are all equal paints as flat** | It *is* a flat colour; routing it through the flat composite gives AC6's byte identity and spends no dither where nothing varies |
| 2026-11-18 | **A singular image-space frame paints no fill in either sink** | An object seen edge-on has no area to fill; SVG renderers drop a non-invertible gradient transform, so both sinks agree on "nothing" |

## Changelog

- 2026-11-18: created and accepted on the owner's go-ahead; decisions
  recorded for review in the PR.
