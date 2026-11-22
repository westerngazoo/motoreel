# SPEC-0017 — Interchangeable finish («acabado»)

- **Status:** Accepted (2026-11-22, on the owner's «facetas con rayos x»,
  «dale con el 3D»). Implemented in the R-0017/R-0018/R-0019 PR; architect
  and owner review pending
- **Realizes:** R-0017
- **Author:** Claude (main session) with owner
- **Created:** 2026-11-22
- **Depends on:** SPEC-0012 (`Prim2::Path`, `Fill`, `Style`), SPEC-0016
  (the three-point frame), SPEC-0006 / SPEC-0003 (the sinks)
- **Module(s):**
  - new `crates/motoreel/src/finish.rs`: `Surface`, `Facing`, `Material`,
    `Layer`, `Seam`, `Light`, `FrameCtx`, the `Finish` trait, typed errors.
    Std-only and pure, like `prim.rs`
  - new `crates/motoreel/src/faceted.rs`: `Faceted`, `lambert`, `quantize`
  - `lib.rs`: exports
  - new `tests/r0017_finish.rs`, goldens `tests/golden/r0017_facetas.{ppm,svg}`

## 1. Motivation

R-0017 separates what a surface is from how it is painted, and asks that
today's low-poly look, rf-marca's `faceta`/`faceta_con_junta`/`velo`/
`lambert`/`cuantizar`, come out of a finish byte for byte.

## 2. Design

### 2.1 The surface

```rust
pub enum Facing { Normal([f64; 3]), Viewer }        // Viewer = (0, 0, 1)
pub struct Surface { outline, facing, depth, id: u64, material, frame: Option<[Pt2; 3]> }
Surface::new(outline, facing, depth, id, material) -> Result<Surface, SurfaceError>
Surface::with_frame(self, [Pt2; 3]) -> Result<Surface, SurfaceError>
Surface::frame_point(&self, u, v) -> Option<Pt2>     // o + u(x − o) + v(y − o)
```

Fields are private behind accessors, so an invalid surface cannot exist.
`SurfaceError::{Outline, Facing, Depth, Frame}` names the field: an empty
outline, a subpath with no segment or a non-finite point (handles
included); a zero or non-finite normal; a non-finite depth or frame point.

**The normal is not normalised.** rf-marca's `lambert` divides the raw
cross product by its own length. Normalising first would change the last
bit of the light value and, at a cut, a band. So the surface keeps what it
is given and finishes normalise (R-0017 decision log).

### 2.2 The material and its layer

`Material::new(&[Rgb])` (1–8 tones, shadow to light), then the builders
`with_opacity`, `with_seam(colour, width)`, `with_layer`. Errors:
`MaterialError::{NoTones, TooManyTones(n), Opacity(a), SeamWidth(w)}`.
Storage is `[Rgb; 8]` plus a length, unused slots repeating the last tone,
so `PartialEq` compares meaning (as `Gradient` does) and the type is `Copy`.

`Layer::{Solid (default), Skin, Inner}` answers R-0017 OQ-3: the tag is on
the material.

### 2.3 Light, context, trait

`Light::new([f64; 3])` keeps the vector verbatim (`VELA` round-trips);
zero or non-finite is `LightError::Direction`. `FrameCtx { t, seed }`.

```rust
pub trait Finish: Send + Sync {
    fn paint(&self, batch: &[Surface], light: &Light, ctx: &FrameCtx) -> Vec<Prim2>;
}
```

Object-safe. `Send + Sync` lets one finish serve every render thread and
rules out unsynchronised interior mutability.

### 2.4 `Faceted`

`Faceted::new(bands ≥ 1, stroke ≥ 0)`, `FacetedError::{Bands, Stroke}`.

1. `bands_of(batch, light)`: the light value of every surface **not**
   `Inner`, by `lambert(normal, light)`, then `quantize(values, bands)`;
   `Inner` surfaces get `None` and do not count.
2. `tone(material, band) = tones[band · len / bands]`.
3. `prim(surface, band)`: seam → `faceta_con_junta`'s style; opacity 1 →
   `faceta`'s (stroke = the caller's width); otherwise `velo`'s.

`lambert` and `quantize` are rf-marca's bodies after the cross product,
operation for operation: flip when `z < 0`, `sqrt` of the sums of squares,
`n·l / (|n| |l|)`; a stable `total_cmp` sort and cuts at `len·k / bands`.

## 3. Code outline

See `faceted.rs`: `paint` is `zip(batch, bands_of).filter_map(prim)`.

## 4. Non-goals

`Pintura` and `Cel` (only their shape is tested, AC7). Palette tokens.

## 5. Open questions

None; R-0017 §5 records the resolutions.

## 6. Acceptance criteria → tests (`tests/r0017_finish.rs`)

| AC | Tests |
|----|-------|
| AC1 | `ac1::a_valid_surface_keeps_every_field`, `ac1::an_empty_or_non_finite_outline_is_refused`, `ac1::a_zero_or_non_finite_normal_is_refused`, `ac1::a_non_finite_depth_or_frame_is_refused` |
| AC2 | `ac2::valid_materials_hold_their_fields`, `ac2::every_bad_field_is_named`, `ac2::equality_compares_the_ramp_not_its_storage` |
| AC3 | `ac3::the_candle_round_trips_exactly`, `ac3::a_zero_or_non_finite_light_is_refused` |
| AC4 | `ac4::the_trait_is_object_safe`, `ac4::the_output_keeps_the_batch_order` |
| AC5 | `ac5::faceta_is_an_opaque_surface`, `ac5::faceta_con_junta_is_a_surface_with_a_seam`, `ac5::velo_is_a_translucent_surface` (50 polygons each), `ac5::lambert_is_rf_marcas_to_the_bit` (1000 triangles), `ac5::quantize_is_cuantizar`, `ac5::batch_bands_are_cuantizar_of_lambert` (1000 normals), `ac5::a_whole_figure_is_painted_as_rf_marca_paints_it`, `ac5::a_bad_faceted_is_refused` |
| AC6 | the existing suites and goldens, unedited but for one line (SPEC-0018 §2.7); `ac6::eval_with_a_look_changes_nothing_without_meshes` |
| AC7 | `ac7::marks_ride_the_surface_frame`, `ac7::two_tone_warms_monotonically_with_the_light`, `ac7::the_interior_shows_only_under_the_two_layer_finish` |
| AC8 | `ac8::two_renders_are_byte_identical`, `ac8::renders_on_several_threads_are_byte_identical`, `ac8::the_figure_uses_four_bands`, `ac8::the_ppm_golden_matches_byte_for_byte`, `ac8::the_svg_golden_matches_byte_for_byte`, `ac8::the_finish_code_is_trig_free` |
| AC9 | `ac9::no_new_dependency`, `ac9::the_finish_lives_in_the_core`; CI's `--no-default-features` step |

The oracles are rf-marca's code at `162ce66`, copied token for token into
the test file (rustfmt re-wrapped it), so «byte for byte» is checked
against the real helpers.

## 7. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-11-22 | Accepted with R-0017 | The owner's go-ahead |
| 2026-11-22 | Private fields, validated constructors | AC1/AC2: a surface with a non-finite coordinate "cannot be built" |

## Changelog

- 2026-11-22: created and implemented.
