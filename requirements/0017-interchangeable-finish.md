# R-0017 — Interchangeable finish («acabado»)

- **Status:** Draft
- **Milestone:** MC (the creator pipeline)
- **Owner:** Gustavo Delgadillo (westerngazoo)
- **Created:** 2026-11-22
- **Depends on:** R-0012 (`Prim2::Path`, `Fill`, `Style`, nonzero fills),
  R-0016 (gradient paint, the three-point frame that rides an object, the
  Bayer dither), R-0006 (`PpmSink`, determinism), R-0003 (`SvgSink`)
- **Realized by:** — (SPEC-0017, to be written after acceptance)
- **Architecture:** guion-video-creator
  [ADR-0006](https://github.com/westerngazoo/guion-video-creator/blob/main/docs/decisions/0006-layered-architecture-kit-and-finishes.md)
  (the layered architecture this requirement is one layer of)
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

motoreel must separate **what a surface is** from **how it is painted**.
A caller hands motoreel a batch of *surfaces*. Each surface has an
image-space outline, an orientation to the light, a view depth, a stable
identity and a material. A pluggable *finish* («acabado») turns that batch into
ordinary `Prim2`s. The same scene can then be rendered low-poly, painterly,
toon or x-ray by changing one value, and nothing else.

Concretely:

1. **A surface is data.** It carries an outline as `Subpath<Pt2>`, a unit
   normal in view space (or "faces the viewer" for a flat 2D piece), a view
   depth, a stable `u64` id and a material. The material is a tone ramp from
   shadow to light, an opacity and an optional seam colour.
2. **A finish is a trait.** It receives the whole batch, the light and a
   frame context, and returns `Prim2`s in the batch's order. It gets the
   batch, not one surface at a time, because today's low-poly look quantises
   tones by population over the **whole figure** (see §2).
3. **This requirement ships the trait and one finish: `Facetas`.**
   `Facetas` is today's low-poly look, and it must reproduce it byte for
   byte. `Pintura`, `Cel` and `RayosX` are planned (§4) and each gets its own
   requirement. This one only has to prove that the trait can carry what
   they need.

## 2. Rationale

The house now draws in one style across four Instagram accounts
(«juguete + rayos X», rotorf-sico `docs/decisions.md`, 2026-11-18). Every
account's motor repeats the same painting decisions by hand:

- `brand/rf-marca/src/poly.rs` in rotorf-sico holds the low-poly rules.
  `faceta` fills **and** strokes 1 px in its own colour, because without the
  stroke the shared edge of two antialiased fills lets the background show
  (measured there: up to 8 levels darker). `velo` is the translucent fill with
  no stroke. `lambert` lights a face against the 135° candle `VELA`, and
  `cuantizar` splits the lit values into N bands **of equal population**,
  once per figure.
- `brand/rf-marca/src/tres.rs` repeats it for 3D. It culls back faces and takes
  each face's tone from its normal against the same light («four tones, no
  gradients»).
- `fbf/motor/src/juguete.rs` already has an in-house `enum Acabado { Capas,
  Suave }`. Its `Forma` struct (points, colour, opacity, optional gradient) is
  a surface in all but name, and `pintar(formas, ventana, alfa)` is a finish
  in all but name. Each piece takes a `_con` variant just to thread the finish
  through.

That last point is the real prior art. The idea has been reinvented once
already, as an enum that each drawing function has to thread through. A
painterly or toon look added the same way would double every `_con` function
again. A trait at the renderer boundary decouples the two: drawing code
describes surfaces once, and the finish is chosen per reel.

It also follows the separation that RenderMan's shading language made
standard. Geometry and the "shader" that paints it are independent
(Cook, *Shade Trees*, SIGGRAPH 1984). glTF 2.0 does the same with meshes and
materials (Khronos, 2017).

## 3. Acceptance criteria

- **AC1. The surface.** A public `Superficie`-like type (the name is the
  spec's call) holds:
  - an outline as `Vec<Subpath<Pt2>>` in image space;
  - an orientation, either a unit normal `(x, y, z)` in view space or a
    "faces the viewer" marker for flat 2D pieces;
  - a finite view depth;
  - a stable id (`u64`), chosen by the caller and constant across frames for
    the same piece;
  - a material;
  - an optional three-point image-space frame (R-0016's representation) that
    rides the piece. A finish that places marks *on* a surface (strokes,
    hatching) anchors them to this frame, so the marks move with the piece.

  It stays plain data. Construction validates finiteness and returns a typed
  error. A surface with a non-finite coordinate cannot be built.
- **AC2. The material.** A material holds a tone ramp of 1 to 8 `Rgb` values
  from shadow to light, an opacity in `[0, 1]` and an optional seam colour
  and width. Bad input (an empty ramp, more than 8 tones, an opacity out of
  range or non-finite) is a typed error that names the field.
- **AC3. The light.** A light is a direction in view space, plus an ambient
  term if the spec needs one. The house candle (`VELA = [-0.6, -0.6, 0.52]`,
  unnormalised) can be expressed exactly. motoreel ships no house constant;
  the brand layer passes it in.
- **AC4. The finish trait.** One trait, object-safe, with one painting
  method:
  - **inputs:** the batch of surfaces, the light, and a frame context with
    at least the frame time `t` and a `u64` seed;
  - **output:** `Vec<Prim2>`;
  - **order:** the output keeps the batch's order. A finish may emit zero or
    several primitives per surface, but never reorders surfaces.
  - **determinism:** no interior mutability, no global state, no clock and
    no thread-dependent behaviour. It is a pure function of its inputs.
- **AC5. `Facetas` is today's look, byte for byte.** With `bands = N`:
  - each surface's light value is `n·l / (|n| |l|)`, computed exactly as
    rf-marca's `lambert` does (same operation order, and the normal flipped to
    face the viewer when `z < 0`);
  - tones are assigned by population over the **batch**, exactly as rf-marca's
    `cuantizar` does (stable `total_cmp` sort, cut indices
    `len·k/N`);
  - an opaque surface is emitted as a fill plus a stroke in the same colour.
    The stroke is 1 px wide, a width the caller passes, because rf-marca's
    `u(1.0)` is a canvas unit and motoreel does not know the canvas scale.
    That is rf-marca's `faceta`;
  - a surface with a seam colour is emitted as `faceta_con_junta`;
  - a translucent surface is emitted as a fill with a zero-width,
    zero-alpha stroke, which is rf-marca's `velo`.

  For each of those helpers, a test builds the same polygon through the
  helper and through `Facetas`. The two `Prim2` values must be equal
  (`PartialEq`, exact `f64`).
- **AC6. Nothing already rendered moves.**
  - Every golden fixture in `crates/motoreel/tests/golden` is byte-unchanged.
  - Every existing test passes unedited. The trait is additive: nothing in
    `Scene::eval`, `Object`, `Prim2` or the sinks changes signature or
    behaviour for a caller that does not use a finish.
  - **Downstream gate (recorded here, run in rotorf-sico):** when a reel moves
    from rf-marca's helpers to `Facetas`, `tools/golden-reels.sh` stays green
    for every reel it covers. See the migration plan (ADR-0006, phase 5).
- **AC7. The trait can carry the planned finishes.** Three test-only finishes
  live in `tests/`, not in the public API, to prove the trait is not shaped
  around `Facetas` alone:
  - a *stroke-placing* finish that emits marks anchored to the surface frame,
    seeded by `(seed, surface id)`. Rendered at two times with the surface
    translated between them, the marks translate with it and keep their
    count and order. That is the temporal-stability property `Pintura` needs;
  - a *two-tone* finish that interpolates between a cool and a warm colour
    from `n·l`, the shape of Gooch shading;
  - a *two-layer* finish that paints surfaces tagged as exterior translucent
    and interior opaque, the shape of `RayosX`. If this needs a tag on the
    surface or material, the spec adds it here. It must not be smuggled in
    through the id.
- **AC8. Determinism.**
  - Two renders of the same batch give byte-identical frames in both sinks,
    also when frames are rendered on several threads at once.
  - `Facetas` uses only `+ − × ÷`, `sqrt` and comparisons: no
    transcendental, no `mul_add`. This matches R-0016 AC8, so a trig-free
    golden fixture holds on any host.
  - A checked-in trig-free golden frame per sink shows a five-face figure
    under `Facetas` with 4 bands and matches byte for byte.
- **AC9. The kernel stays thin.** Zero new dependencies.
  `cargo build -p motoreel --no-default-features` still builds, lints and
  tests. The trait and `Facetas` live in the core, not behind a feature.

## 4. Constraints & non-goals

- **Not the other finishes.** `Pintura` (painterly, after Hertzmann 1998),
  `Cel` (toon, after Gooch et al. 1998 and Lake et al. 2000) and `RayosX` are
  separate requirements. Only their *shape* is tested here (AC7).
- **Not 3D.** Surfaces arrive already projected. Building them from a posed
  mesh (projection, back-face culling, depth sort) is R-0018. The depth and
  normal fields exist so that R-0018 can fill them.
- **Not a scene-graph change.** `Scene::eval` keeps returning `Prim2`s in
  draw order. The finish is called by whoever builds surfaces (R-0018's mesh
  path, or a brand layer drawing 2D pieces), and it returns `Prim2`s that join
  the usual stream.
- **Not palette tokens.** motoreel sees `Rgb`. Token-to-colour mapping stays in
  each account's brand layer (rf-marca, guion-marca).
- **Not real-time.** This is the offline backend's finish. A real-time backend
  re-implements the same *named* finishes as shaders against the same
  description (ADR-0006). Byte equality between the two backends is not a
  goal. Only "the same look by name" is.

## 5. Open questions

1. **Batch scope for quantisation.** rf-marca quantises "once over the
   complete figure". Is a batch always one figure, or does the trait need
   groups inside a batch (e.g. two toys side by side, each quantised on its
   own)? *Recommendation:* one batch is one figure. The caller makes one call
   per figure, which keeps the trait minimal.
2. **Where the house light lives.** `VELA` is brand data, so this
   recommendation keeps it out of motoreel. Should the light come from
   `cuenta.toml` (guion-marca) so each account can tilt it?
3. **The `RayosX` tag.** Is the exterior/interior tag an attribute of the
   surface or of the material? AC7 forces the decision during the spec.
4. **Names.** Should the public names be Spanish (`Acabado`, `Facetas`,
   `Superficie`, the house's words) or English like the rest of the public API
   (`Finish`, `Faceted`, `Surface`)? *Recommendation:* English types in
   motoreel, with the Spanish names in the doc comments. The house's Spanish
   names live in the kit (ADR-0006).

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-11-22 | Draft written under the owner's approval of the direction («dale con tu recomendación»). The acceptance criteria await his OK | ADR-0006 |
| 2026-11-22 | **Batch, not per-surface, painting** (proposed) | Today's tones are quantised by population over the whole figure (rf-marca `cuantizar`). A per-surface trait could not reproduce them, and AC5 would fail by construction |
| 2026-11-22 | **Stable surface id plus a frame that rides the piece** (proposed) | Temporal coherence for marks placed on surfaces: Meier, *Painterly Rendering for Animation*, SIGGRAPH 1996, attaches particles to surfaces for the same reason. Reuses R-0016's three-point frame instead of inventing a second pose representation |
| 2026-11-22 | **`Facetas` is defined by equality with rf-marca's helpers** (proposed) | The byte-identity promise is only checkable if the reference is executable. The helpers are small and pure, so the tests can copy them verbatim as oracles |

## Prior art

- R. L. Cook, "Shade Trees", *SIGGRAPH '84*: separates geometry from the
  procedure that shades it.
- B. J. Meier, "Painterly Rendering for Animation", *SIGGRAPH '96*: particles
  attached to surfaces for frame-to-frame coherence (AC7's stroke property).
- A. Hertzmann, "Painterly Rendering with Curved Brush Strokes of Multiple
  Sizes", *SIGGRAPH '98*: the planned `Pintura`.
- A. Gooch, B. Gooch, P. Shirley, E. Cohen, "A Non-Photorealistic Lighting
  Model for Automatic Technical Illustration", *SIGGRAPH '98*: the planned
  `Cel`'s cool-to-warm tone.
- A. Lake, C. Marshall, M. Harris, M. Blackstein, "Stylized Rendering
  Techniques for Scalable Real-Time 3D Animation", *NPAR 2000*: hard-banded
  toon shading.
- Khronos glTF 2.0 (2017): mesh and material as separate, renderer-neutral
  data.
- In-house: rotorf-sico `brand/rf-marca/src/poly.rs` (`faceta`, `velo`,
  `lambert`, `cuantizar`, `VELA`), `brand/rf-marca/src/tres.rs`,
  `fbf/motor/src/juguete.rs` (`Acabado`, `Forma`, `pintar`).

## Test plan

| AC | Unit tests (`tests/r0017_finish.rs`) | E2E |
|----|-------------------------------------|-----|
| AC1, AC2 | One test per constructor error variant, plus a valid build per field | — |
| AC3 | `VELA` round-trips exactly through the light type | — |
| AC4 | The output keeps input order for a shuffled batch. The trait is object-safe (`Box<dyn …>` compiles) | — |
| AC5 | Oracle copies of `faceta`, `faceta_con_junta`, `velo`, `lambert`, `cuantizar` (verbatim, cited). `Prim2` equality on 50 polygons per helper, plus band equality on 1000 seeded random normals | — |
| AC6 | The existing suite and goldens run unchanged in CI | rotorf-sico `tools/golden-reels.sh` in the phase-5 PR |
| AC7 | Three test-only finishes, each with its property test (translation, monotone cool-to-warm, interior visible only under the two-layer finish) | — |
| AC8 | Two renders byte-equal. Four threads byte-equal. A golden PPM and SVG for the five-face figure | `cargo run --example` renders the figure and stock ffmpeg encodes it |
| AC9 | The existing `--no-default-features` build test | — |

## Changelog

- 2026-11-22: created as Draft. The owner approved the direction; the
  acceptance criteria await his OK.
