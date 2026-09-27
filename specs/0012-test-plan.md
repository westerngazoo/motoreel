# SPEC-0012 test plan — R-0012 paths and fills

- **Status:** QA loop step 3 (TDD red) done, then brought up to date with the owner's step-3 decisions (651e14d; §8 below). All 93 r0012 tests pass against the implementation on the branch. **Step 7 QA sign-off: PASS** (2026-09-27, on `dd4024c` + `2fd32f9`; §11).
- **Realizes:** the test half of R-0012 / SPEC-0012 §6
- **Author:** `qa` agent, scoped to R-0012
- **Date:** 2026-09-27
- **Suite:** `crates/motoreel/tests/r0012_paths_fills.rs`, with 90 tests in `mod ac1` … `mod ac12` (84 at step 3, plus 6 after the owner's decisions)
- **Goldens:** `crates/motoreel/tests/golden/r0012_paths.{ppm,svg}`, blessed and reviewed at step 5 (945235a).

Earlier suites kept their criterion → test map in the module doc, and no
test-plan file exists before this one. Here the suite's `mod acN` layout
is the map, and its module doc points to this file. This file holds the
table (§2) and what a module doc cannot: the ambiguities, the proposed
private unit tests, and the step-7 checklist.

## 1. How the suite is red, and why that differs from precedent

R-0006 (`ff71542`) and R-0007 (`097c95c`) went red **at run time**. Each
suite landed in the same commit as `unimplemented!()` stubs in `src/`, and
R-0006 also landed a 0-byte golden placeholder.

The qa agent writes test code only, so this suite lands **without stubs**
and is red **at compile time**:

```
error: couldn't read `crates/motoreel/tests/../examples/card/scene.rs`: No such file or directory
```

rustc stops at that missing module before name resolution. With that one
line removed, the suite yields 37 errors, which are exactly the SPEC-0012
API:

- unresolved `motoreel::path`, `motoreel::shapes`, `Fill`, `Seg` and `Subpath`;
- no field `fill` on `Style`;
- no `Prim2::Path` and no `Shape::Path`;
- no `Object::planar` and no `Object::path`.

Other consequences:

- **Every other target is a separate crate and is unaffected.** On
  1.98.1 (`rustup run stable`), all of these pass: r0001 (28), r0002 (16),
  r0003 (15), r0004 (21), r0006 (27), r0007 (24), r0009 (10), lib (21),
  doc (1), motoreel-typeset (36), and `cargo build --examples`.
- **A bare `cargo test --workspace` is red.** Cargo stops at the first
  target that fails to compile, and CI runs on every push. Do not push this
  commit alone. Push it together with the implementation, or add signature
  stubs at step 4 if a run-time red is wanted.
- **The goldens are read at run time,** with `fs::read` rather than
  `include_bytes!`. A missing or empty fixture then panics with the bless
  command, and no placeholder file is needed.

## 2. Criterion → test map

The layers are:

- **e2e** — the public API, a sink, and bytes.
- **src-scan** — a scrubbed grep over `src/`, with comments and strings
  removed.
- **manifest** — `Cargo.toml` itself.

| AC | Tests (`mod acN::…`) | Layer | What is pinned |
|---|---|---|---|
| AC1 (9) | `the_builder_produces_exactly_the_documented_data`, `reversed_walks_backwards_swapping_handles`, `map_touches_every_control_point_and_keeps_the_structure`, `split_cubic_is_de_casteljau_at_one_half`, `planar_and_path_build_the_same_shape_path`, `eval_emits_a_posed_path_carrying_its_style_verbatim`, `prim2_and_shape_gain_exactly_one_variant_each`, `every_prim2_match_in_both_sinks_is_exhaustive_by_name`, `the_shape_match_in_eval_is_exhaustive_by_name` | e2e, src-scan | §2.1 types and builder, `split_cubic` on 2- and 3-tuples, planar lift to z = 0, posing by track, style carried verbatim, and no catch-all arm (`_` or a bare binding) in any `Prim2`/`Shape` match in `svg.rs`, `ppm.rs` or `scene.rs` |
| AC2 (9) | `fill_only_paints_the_interior_and_no_stroke`, `stroke_only_leaves_the_interior_unpainted`, `with_both_paints_the_stroke_lands_on_top_of_the_fill`, `both_paints_are_exactly_fill_then_stroke`, `each_paint_composites_once_per_pixel_where_geometry_overlaps`, `neither_paint_leaves_pure_background`, `fill_alpha_is_clamped_and_a_dead_fill_leaves_the_stroke_alone`, `some_fill_is_ignored_by_every_other_variant_in_both_sinks`, `fill_is_plain_copy_data_and_the_default_is_none` | e2e | Fill-then-stroke as an exact identity: one path with both paints ≡ a fill-only prim then a stroke-only prim, and the reverse order differs. One composite per paint, even at winding 2 or on retraced chords (128, not 191). NaN, 0 or negative fill alpha paints nothing, and 1.5 ≡ 1. A `Some` fill leaves Point/Segment/Polyline/Edges/Text bytes unchanged in both sinks (Text in PPM under `text`) |
| AC3 (12) | `a_pentagram_fills_its_interior_with_no_seam`, `the_pentagram_svg_is_one_nonzero_path_through_the_tips`, `nested_squares_with_the_same_orientation_fill_solid`, `nested_squares_with_opposite_orientation_leave_a_hole`, `a_reversed_generator_inside_a_generator_is_a_hole`, `an_open_subpath_fills_exactly_as_if_closed`, `an_open_subpath_strokes_no_closing_segment`, `edge_order_and_single_loop_orientation_change_nothing`, `a_vertex_on_a_pixel_centre_row_is_counted_once`, `zero_length_edges_change_nothing`, `a_zero_area_subpath_fills_nothing`, `disjoint_subpaths_in_one_path_each_fill` | e2e | The pentagram interior is ≥ 0.5 px from the 10-vertex outline (§6's definition, which is not "winding 2"), is byte-exact fill colour, and >100 of the checked pixels sit on internal chords. Same-orientation nesting is solid with inner edges through pixel centres, so a seam would show as 128. Opposite orientation is a hole whose edge pixels are exactly 128. Open ≡ closed for fill, but no closing stroke. Half-open crossing rule. SVG `d` and `fill-rule` for every case |
| AC4 (11) | `orthographic_control_points_are_the_projected_model_points_bit_for_bit`, `pinhole_lines_project_their_end_points_exactly`, `pinhole_pieces_lie_within_tau_of_the_true_perspective_image`, `pinhole_ppm_outline_lies_within_tau_plus_a_tenth_of_a_pixel`, `pinhole_is_not_the_cubic_through_the_projected_control_points`, `pinhole_subdivision_stops_at_4096_pieces_per_authored_cubic`, `a_control_point_behind_the_camera_culls_the_whole_path`, `a_control_point_on_the_camera_plane_culls_the_whole_path`, `a_non_finite_control_point_culls_the_whole_path`, `empty_subpaths_are_dropped_and_an_empty_path_is_not_emitted`, `emitted_paths_uphold_the_prim2_invariants` | e2e | Ortho is `to_bits` equal, with one `C` per authored cubic. Pinhole is measured as geometric nearest-point distance in both directions, ≤ τ = 9·10⁻⁵ (10³ true samples per cubic, never same-parameter), at nearest depths 1.63 and 0.63. PPM: every fractional pixel's coverage is inverted to a distance, which must match the true curve within τ + 0.1 px + 1/510 + 10⁻³. A depth limit of 12 gives ≤ 4096 pieces (the case needs 18 893 without it). A handle alone behind the camera, at depth 0, or NaN/±∞ culls the whole path under both cameras, and a handle at depth 0.5 does not |
| AC5 (9) | `a_rectangle_on_pixel_boundaries_has_no_fringe`, `an_edge_through_pixel_centres_paints_exactly_128`, `an_axis_aligned_edge_at_any_sixteenth_offset_is_the_box_filtered_area`, `corner_error_is_at_most_a_quarter_px2_per_corner_on_the_exhaustive_grid`, `corner_error_bound_holds_at_random_non_dyadic_offsets`, `a_centre_on_an_edge_line_beyond_its_end_takes_the_outside_sign`, `hole_corner_error_is_bounded_with_the_inside_sign_tie_break`, `coverage_is_translation_equivariant`, `distant_geometry_in_the_same_path_moves_no_pixel` | e2e | The three §2.6 identities as full-frame byte equality: 17 offsets × 4 sides, with `round(255·area)`. The corner bound on all 278 784 grid rectangles plus 20 000 random non-dyadic ones, each within `1.0 + n/510`. Tie rule: a centre on an edge's line 0.25 px past its end is 0 or 64, never 191. Tie-break direction (§11): hole corners within `1.6 + n/510` on the same grid. Closed form per pixel: translation-equivariant, and unmoved by distant geometry in the same rows |
| AC6 (2) | `a_stroked_circle_lights_only_its_amended_band_and_all_of_its_core`, `chords_stay_within_a_tenth_of_a_pixel_of_random_cubics_including_cusps` | e2e | The amended band `R ± (r + 0.6 + 3·10⁻⁴R)` and the core `R ± (r − 0.5)` for R ∈ {10, 100, 500} and r ∈ {1, 2, 4}, over every pixel of a 1024² frame. Chord deviation read from the pixels: over 125 cubics (a true cusp, a loop, a collinear retrace, a point, a near-line), `|d_chords − d_curve| ≤ 0.1 + 1/510 + 10⁻³` |
| AC7 (20) | `the_circle_handle_is_the_pinned_constant_bit_for_bit`, `a_circle_is_four_counter_clockwise_quarters`, `circle_radial_error_is_within_3e_4_r_and_only_outward`, `an_arc_uses_one_cubic_per_started_quarter_turn`, `arc_endpoints_lie_on_the_circle_and_the_arc_runs_counter_clockwise`, `a_negative_sweep_is_the_same_region_traversed_counter_clockwise`, `a_sweep_beyond_a_full_turn_is_clamped_to_one_turn`, `a_sector_is_the_centre_a_line_to_the_arc_the_arc_and_a_close`, `a_rounded_rect_with_zero_radius_is_three_lines_and_a_close`, `rounded_rect_corners_are_quarter_circles_with_handle_k_rad`, `rounded_rect_radius_is_clamped_to_half_the_shorter_side`, `a_polygon_is_its_vertices_as_lines_closed_implicitly`, `a_polygon_keeps_a_clockwise_input_clockwise`, `a_polygon_drops_consecutive_duplicates_including_last_equal_to_first`, `a_polygon_with_fewer_than_three_remaining_vertices_is_empty`, `a_polygon_counts_remaining_vertices_not_distinct_ones`, `generators_are_counter_clockwise`, `no_generator_emits_a_zero_length_side`, `degenerate_inputs_return_an_empty_path_without_panicking`, `infinite_inputs_return_an_empty_path_for_every_generator` | e2e | K is bit-exact, read through `circle(0, 1)`'s first handle, so `K` need not be public. The radial error is ≤ 3·10⁻⁴R at 10⁵ samples, outward only, and ≥ 2.5·10⁻⁴R (which proves it is the 4-cubic circle). Arc: m = ⌈\|sweep\|/(τ/4)⌉, and always counter-clockwise (§2.9): a negative sweep runs from `start + sweep` to `start`, end points within 10⁻¹² relative, piece midpoints at `from + \|sweep\|·(i + ½)/m`, and it is the same region as `(start − s, +s)`. Circle, arc, sector and rounded rect have positive area for either sign of sweep. `polygon` keeps clockwise input clockwise, drops consecutive duplicates (last = first included), and gives empty below 3 remaining vertices; `[a, b, a, b]` is the zero-area 4-gon (§2.13, §11). The rad clamp is `assert_eq`-exact against the clamped call. `rad = 0` gives 3 lines and a close. 31 degenerate inputs return empty, and so do ±∞ in every argument of every generator (§2.13) |
| AC8 (7) | `the_path_element_grammar_is_pinned_byte_for_byte`, `a_stroke_only_path_writes_fill_none_and_no_fill_attributes`, `a_path_whose_stroke_paints_nothing_writes_stroke_none`, `neither_paint_writes_fill_none_and_stroke_none`, `a_non_finite_fill_alpha_writes_fill_none_and_paints_nothing`, `numbers_use_shortest_round_trip_display`, `ppm_and_svg_place_a_fill_boundary_within_one_pixel` | e2e | Whole-document equality for the fixture. Seven no-stroke cases: width 0, −1, NaN, ∞, and alpha 0, NaN, −0.5. A NaN, +∞ or −∞ fill alpha writes exactly the stroke-only element (`fill="none"`), and the PPM frame equals the stroke-only frame (§2.13; also AC2). `-0` and `0.30000000000000004` written as such. SVG outline points (four per piece) map by §2.3 to a pixel whose 3 × 3 window holds both a pixel ≥ 128 and one < 128 |
| AC9 (5) | `two_renders_are_byte_identical_in_both_sinks`, `the_ppm_golden_matches_byte_for_byte`, `the_svg_golden_matches_byte_for_byte`, `no_mul_add_or_hypot_in_path_flattening_or_fill_code`, `no_transcendental_in_the_raster_path_or_the_trig_free_generators` | e2e, src-scan | Identity for the golden scene and for a trig-bearing pinhole scene in motion. §2.10's scene is pinned in `golden_scene()`, and each golden is sanity-checked (six pixels, three `<path>`s) **before** comparison or bless, so a bad bless cannot pass. The `mul_add`/`hypot` grep covers `path.rs`, `shapes.rs`, `scene.rs`, and `ppm.rs` minus `blend_pixel`. The trig grep covers `ppm.rs` minus the text path, `path.rs`, and the bodies of `circle`, `rounded_rect` and `polygon` |
| AC10 (1 + existing) | `every_pre_existing_golden_is_byte_unchanged`; plus r0003 `ac3_golden…`, r0006 `ac3_golden…`, r0007 `ac5_labels_svg_golden_matches` and `ac5_r0003_golden_is_byte_unchanged`, and r0004 `ac6_r0003_golden_fixture_still_passes_untouched` | e2e | The three existing fixtures are pinned by length and FNV-1a-64 as of this commit, so a re-bless fails here. The render comparisons live in the owning suites |
| AC11 (2) | `the_manifest_declares_no_new_dependency`, `paths_and_shapes_are_std_only_core_modules` | manifest, src-scan | Dependencies exactly `garust` plus the optional `motoreel-typeset`, and `proptest` for dev. `pub mod path;` and `pub mod shapes;` are not `cfg`-gated. `path.rs` names no `prim`, `Pt2` or `pga::`, so there is no cycle (finding 4) |
| AC12 (6) | `the_card_is_a_three_second_portrait_clip`, `the_card_renders_every_frame_through_the_raster_sink`, `the_card_shows_a_panel_a_friction_circle_a_dot_on_its_rim_and_an_area`, `the_card_colours_live_in_the_example_not_the_engine`, `the_card_documents_its_stock_ffmpeg_command`, `stock_ffmpeg_encodes_the_card` | e2e | View (1.8, 3.2) and 3 s. 90 frames at 30 fps (270 × 480 preview) plus one full 1080 × 1920 frame. A hairline panel ≤ 2 px. A friction circle with fill alpha ≤ 0.5, stroked. A moving filled dot whose centre stays within 2 % of the rim at seven times. A filled lines-only area with a stroke-only curve after it. No card colour appears in `src/`. The ffmpeg encode checks `ftyp` and prints `AC12 skipped` when ffmpeg is absent |

## 3. Edge cases covered, beyond the golden path

- **Degenerate generator inputs.** r ≤ 0, −0, NaN or ±∞ in any argument
  of any generator, sweep 0 or −0, w or h ≤ 0, fewer than 3 vertices, a
  non-finite vertex, and a subnormal positive radius, which must still be
  a circle.
- **Negative sweeps.** Arc and sector are counter-clockwise for sweeps of
  −0.3, −2.0, −4.5 and −τ, and cover the same region as the positive
  sweep from `start + sweep`.
- **Polygon input.** Clockwise input kept; consecutive duplicates at the
  start, middle and end, last = first, and all doubled; a non-consecutive
  repeat kept; 1 or 2 remaining vertices give empty; `[a, b, a, b]`
  keeps all four.
- **Non-finite coordinates.** NaN, +∞ and −∞ in a handle, and in a line
  end of a second subpath, all cull the whole path under both cameras.
- **Zero-length edges.** A repeated vertex leaves fill and stroke bytes
  unchanged. A horizontal out-and-back line fills nothing.
- **Empty subpaths.** They are dropped. With none left, or with no
  subpaths at all, nothing is emitted.
- **Paint combinations.** Fill-only, stroke-only, both, and neither. NaN,
  ±∞, 0, negative and above-1 fill alpha. Fill alpha NaN, ±∞ or 0 with a
  live stroke, in both sinks.
- **Fills on other variants.** `Some` fill on all five non-path variants,
  in both sinks.
- **Nesting.** Same and opposite orientation, with inner edges through
  pixel centres. A reversed generator makes a ring. Disjoint subpaths
  each fill.
- **Order.** Subpath order and a lone loop's orientation don't matter.
- **Open subpaths.** An open subpath fills as closed but strokes no
  closing side.
- **AC5 byte 128.** Exact on both a vertical and a horizontal edge through
  pixel centres.
- **Corner bound.** Exhaustive 1/16 grid, `min(w, h) ≥ 1 px`, plus random
  non-dyadic offsets.
- **Tie rule.** Eight tie pixels, covering both edge directions at all four
  corner orientations.
- **AC6.** Cusp, loop, collinear retrace, point cubic and near-line cubic.
- **SVG numbers.** `-0`, 1e-7, 1e21 and 0.1 + 0.2.
- **Pinhole termination.** The 4096-piece bound, at nearest depth 10⁻⁵.

## 4. Unit tests vs e2e; the proposed private unit tests

Everything in the suite is e2e against the public API. Four claims need
the private `ppm.rs` flattening and fill code. They are proposed below as
`#[cfg(test)]` additions to `ppm.rs`'s existing `mod tests`, for the
implementer to land at step 5. The names follow SPEC-0012 §3 (`chords`,
`FLATTEN_MAX`). `foot` is §2.6's foot point, factored out so it can be
pinned, and `draw_fill` is §3's name. All four were run against the
throwaway probe (§7), and each one killed a mutant that nothing else did.

```rust
    // ==== R-0012 (QA proposal): private flattening and fill details ====

    /// B(t) in Bernstein form, test-local, so the check does not reuse the
    /// code under test.
    fn bez_ref(p: [Px; 4], t: f64) -> Px {
        let u = 1.0 - t;
        let (b0, b1, b2, b3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
        (
            b0 * p[0].0 + b1 * p[1].0 + b2 * p[2].0 + b3 * p[3].0,
            b0 * p[0].1 + b1 * p[1].1 + b2 * p[2].1 + b3 * p[3].1,
        )
    }

    // R-0012 AC6 / SPEC-0012 §2.5 and §6 — the meaningful test: over 3000
    // random cubics plus a true cusp, the parametric distance between each
    // uniform chord and its piece of curve is ≤ 0.1 px. (Asserting that
    // `chords` equals its own formula would only restate it.)
    #[test]
    fn chord_deviation_is_within_a_tenth_of_a_pixel_on_random_cubics_and_a_cusp() {
        let mut state = 0x0012_u64;
        let mut unit = || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut cubics: Vec<[Px; 4]> = (0..3000)
            .map(|_| std::array::from_fn(|_| (600.0 * unit() - 300.0, 600.0 * unit() - 300.0)))
            .collect();
        cubics.push([(-80.0, -80.0), (80.0, 80.0), (-80.0, 80.0), (80.0, -80.0)]);
        let mut worst = 0.0_f64;
        for p in cubics {
            let n = chords(p[0], p[1], p[2], p[3]);
            for i in 0..n {
                let (t0, t1) = (f64::from(i) / f64::from(n), f64::from(i + 1) / f64::from(n));
                let (a, b) = (bez_ref(p, t0), bez_ref(p, t1));
                for k in 0..=64 {
                    let u = f64::from(k) / 64.0;
                    let on_curve = bez_ref(p, t0 + u * (t1 - t0));
                    let (cx, cy) = (a.0 + u * (b.0 - a.0), a.1 + u * (b.1 - a.1));
                    let d = ((on_curve.0 - cx) * (on_curve.0 - cx)
                        + (on_curve.1 - cy) * (on_curve.1 - cy))
                        .sqrt();
                    worst = worst.max(d);
                }
            }
        }
        assert!(worst <= 0.1, "worst chord deviation {worst} px");
    }

    // R-0012 AC6 / §2.5 — the count's edges: 1 for a flat cubic, clamped at
    // FLATTEN_MAX beyond the claim, and 33 for §2.5's own worked example (a
    // 300 px quarter circle, L ≈ 138).
    #[test]
    fn chord_count_is_one_when_flat_and_clamps_at_flatten_max() {
        assert_eq!(chords((0.0, 0.0), (1.0, 1.0), (2.0, 2.0), (3.0, 3.0)), 1);
        assert_eq!(chords((5.0, 5.0), (5.0, 5.0), (5.0, 5.0), (5.0, 5.0)), 1);
        assert_eq!(
            chords((0.0, 0.0), (1e6, 0.0), (0.0, 0.0), (1e6, 0.0)),
            FLATTEN_MAX
        );
        let k = 0.552_284_749_830_793_6 * 300.0;
        assert_eq!(chords((300.0, 0.0), (300.0, k), (k, 300.0), (0.0, 300.0)), 33);
    }

    // R-0012 AC5 / §2.6, finding 2 — when t clamps, the foot point is the
    // endpoint itself. Here `a + 1·(b − a)` is 0.8999999999999999, not 0.9,
    // so a foot computed by the formula would miss the vertex.
    #[test]
    fn the_foot_point_is_the_endpoint_verbatim_when_t_clamps() {
        let (a, b) = ((0.2, 0.0), (0.9, 0.0));
        let one: f64 = 1.0;
        assert_ne!(a.0 + one * (b.0 - a.0), b.0, "the case must be the hard one");
        assert_eq!(foot(a, b, (5.0, 1.0)), b);
        assert_eq!(foot(a, b, (-5.0, 1.0)), a);
        assert_eq!(foot(a, b, (0.55, 3.0)), (0.2 + 0.5 * (0.9 - 0.2), 0.0));
    }

    // R-0012 AC2 / §2.6, finding 7 — a non-finite edge makes the fill paint
    // nothing: dropping one edge would open the loop and corrupt the winding
    // across whole rows. Called below `draw`, since eval never emits it.
    #[test]
    fn a_non_finite_edge_makes_the_fill_paint_nothing() {
        let dir = std::env::temp_dir().join("motoreel-r0012-nan-fill");
        let mut sink = PpmSink::with_view(&dir, (16, 16), (16.0, 16.0)).expect("sink");
        let q = |x: f64, y: f64| Pt2 { x, y };
        let bad = Subpath::new(q(-4.0, -4.0))
            .line_to(q(4.0, -4.0))
            .line_to(q(f64::NAN, 4.0))
            .line_to(q(-4.0, 4.0))
            .close();
        sink.canvas.draw_fill(&[bad], Fill::solid(Rgb::WHITE, 1.0));
        assert!(sink.canvas.pixels().iter().all(|&b| b == 0), "nothing painted");
    }
```

The corner bound can only be asserted at f64 precision (`Σcov ≤ w·h + 1.0`
with no `n/510` slack, and the per-pixel error reaching exactly 0.25) if the
fill exposes a pure per-pixel coverage function. §2.6's band pass keeps its
coverage in a scratch tile instead, so no such seam exists.

**Recommendation:** factor `fn fill_coverage(edges: &[(Px, Px)], c: Px) -> f64`
and move the exhaustive-grid sum there. The frame-level version in the suite
already holds the bound with the `n/510` slack.

## 5. What can only be verified in step 7

1. **Bless and review the goldens.** Run
   `MOTOREEL_BLESS=1 rustup run stable cargo test -p motoreel --test r0012_paths_fills ac9`,
   then review `r0012_paths.ppm` as a decoded image and `r0012_paths.svg` as
   text, and commit both. After that, the two `ac9::…golden…` tests
   compare.
2. **AC10: diff review of the old suites.** The only edits to r0001–r0009
   allowed are:
   - `fill: None` (or `..Style::default()`) in `Style` literals, including
     `r0004:1225`'s proptest strategy and `r0006`'s shorthand `style()`
     helper;
   - the forced `Prim2::Path` arm (§6, item 1).

   Also run `git diff main -- crates/motoreel/tests/golden/`, which must show
   nothing for the three existing fixtures.
3. **AC11: build, lint and test the kernel.** Run each of these:
   - `rustup run stable cargo build -p motoreel --no-default-features`
   - `… cargo clippy -p motoreel --no-default-features --all-targets -- -D warnings`
   - `… cargo test -p motoreel --no-default-features`

   **CI does not run these today:** `.github/workflows/checks.yml` has no
   `--no-default-features` step, although `crates/motoreel/Cargo.toml`
   says "CI builds both ways". Add the step, or run it by hand at sign-off.
   `cargo tree -p motoreel -e normal --no-default-features` must show
   `garust` alone.
4. **AC12: run the demo for real.** Run `cargo run --release --example card`,
   then the documented ffmpeg command on its 90 full-size frames. CI
   installs ffmpeg, so `ac12::stock_ffmpeg_encodes_the_card` runs there.
   Add a guard like R-0006's "AC6 encoded for real, and did not skip" that
   fails on `AC12 skipped`, or a probe regression passes silently.
5. **The whole gate** under `rustup run stable`: `cargo test --workspace`,
   `cargo clippy --workspace --all-targets -- -D warnings`, and
   `cargo fmt -p motoreel -p motoreel-typeset --check`. The new suite
   already passes clippy `-D warnings` and fmt against the probe, with and
   without `--no-default-features`.
6. **The downstream companion PRs** (guion-video-creator, and `guion`), per
   §2.11. They are outside this repository's gate.

## 6. Spec ambiguities and gaps found

Each item says what the suite does meanwhile. None blocks the red phase.
Items 1–12 were settled by the owner on 2026-09-27 (651e14d: R-0012 AC10
amended, SPEC-0012 §2.6, §2.9, §2.11 and §2.13); §8 records how the suite
followed. Item 16 was new, and is resolved in §11.

1. **AC10 is narrower than the code allows.** `prim_parts` in
   `tests/r0002_scene_camera.rs:61` and `tests/r0004_physics_playback.rs:227`
   is an exhaustive `match` over `Prim2` with no `_`. Adding `Prim2::Path`
   is therefore a compile error in two existing suites. R-0012 AC10 allows
   only "mechanical edits that the `Style` change forces on struct
   literals", and SPEC-0012 §2.11 says nothing about test helpers. R-0004
   and R-0007 made exactly this edit (the comment at r0004:231 says so).
   **Ask:** the owner sanctions "one added `Prim2::Path` arm in each
   `prim_parts`" as a second mechanical edit, listed in the PR.
   **Resolved:** AC10 amended to allow exactly this.
2. **§2.6's vertex claim is not what §2.6's rule does.** The corner
   analysis says a pixel centred exactly on a vertex has d = 0, "so the
   ramp gives cov = 0.5". Under the pinned classification, the ±δ probes at
   a vertex land exactly on the adjacent edge's line, and the half-open
   crossing rule excludes both of them at one corner orientation. That
   pixel then takes the `inside` branch. Measured on the probe:
   - convex corners give 128, 128, 128 and **0**;
   - hole corners give 128, 128, 128 and **255**;
   - tie pixels give 64 at three corners and **0** at the fourth.

   The 0.25 bound still holds (0 against 0.25, and 1 against 0.75), so AC5
   passes. But the text overstates the symmetry, and fills are not
   invariant under a 90° turn at exact-vertex configurations. **The suite**
   asserts no vertex-centred pixel, and allows {0, 64} for tie pixels.
   **Ask:** reword §2.6 ("0.5, or the inside value at the corner where
   both probes fall on the half-open boundary"), or change the rule.
   **Resolved:** §2.6 reworded; the rule and the bound are unchanged, so
   the suite is unchanged.
3. **Generator orientation is stated universally, but it cannot be CCW
   for every input.** §2.9 says "Orientation is counter-clockwise … stated
   once and tested" for every generator. Yet `sector` with a negative sweep
   is clockwise by its own construction (`M c`, `L` to the arc start, the
   arc, `Z`), and `polygon` either follows its input order or
   re-orients, which is unstated. **The suite** tests CCW only for
   positive sweeps and CCW input. **Ask:** state the rule for negative
   sweep and for clockwise `polygon` input.
   **Resolved (§2.9):** circle, arc, sector and rounded rect are always
   CCW, a negative sweep normalised to the CCW traversal; `polygon` keeps
   the caller's order. Tested (§8).
4. **`polygon` and zero-length sides.** "Generators never emit
   zero-length sides", but `polygon(&[a, a, b, c])` or a repeated closing
   vertex `[a, b, c, a]` passes zero-length sides through, unless polygon
   de-duplicates. If it does, does fewer than 3 distinct vertices mean an
   empty result? **The suite** does not test duplicate input.
   **Resolved (§2.13):** consecutive duplicates (last = first included)
   are dropped; fewer than 3 distinct vertices gives empty. Tested (§8),
   except the case in item 16.
5. **`split_cubic`'s signature.** §2.1's prose reads
   `split_cubic(p0, p1, p2, p3, mid)`, but §3 has
   `split_cubic(p: [P; 4], mid) -> ([P; 4], [P; 4])`. **The suite** uses
   §3's. **Resolved (§2.13):** §3's form.
6. **Generator parameter types.** They are not pinned. **The suite**
   assumes a `c: Pt2` centre by value and `polygon(&[Pt2])`. `Vec<Pt2>` or
   `impl IntoIterator` would need a call-site change. **Resolved
   (§2.13):** `polygon(pts: &[Pt2])`.
7. **Non-finite versus clamp.** `rounded_rect(…, rad = ∞)` and
   `arc(…, sweep = ±∞)` fall under both "non-finite arguments → empty" and
   the clamp rules. **The suite** expects empty, since non-finite is listed
   first. **Resolved (§2.13):** empty. Tested for every generator (§8).
8. **SVG with a non-paintable `Some` fill.** A NaN or negative fill alpha
   paints nothing in PPM, but §2.8 only maps `None` to `fill="none"`. The
   SVG would carry `fill-opacity="NaN"`, which a renderer may treat as 1,
   so the sinks would disagree. **The suite** does not test the SVG side.
   **Suggest** the stroke's rule: `fill="none"` when `unit(alpha) == 0`.
   **Resolved (§2.13):** a non-finite fill alpha paints nothing in PPM
   (+∞ included, overriding `unit`) and writes `fill="none"` in SVG.
   Tested (§8). A finite alpha ≤ 0 is not named by §2.13; the
   implementation also writes `fill="none"` for it, which the suite does
   not assert.
9. **The SVG `stroke="none"` predicate.** "Width not finite and > 0, or
   alpha 0, the PPM guard's own test". **The suite** reads this as
   including `unit(alpha) == 0`, so NaN and negative alpha also write
   `stroke="none"`. The implementation agrees and the tests pass.
10. **The exact `d` whitespace.** Single spaces, `" Z"`, subpaths joined
    by one space, and one line per element: this is QA's reading of the
    §2.8 template, which is wrapped for display. It is pinned by
    `ac8::the_path_element_grammar_is_pinned_byte_for_byte`. If the
    implementation differs, clarify §2.8 and do not loosen the test.
    **Resolved (§2.13):** exactly this whitespace.
11. **Golden-scene details left open by §2.10.** The suite pins them in
    `ac9::golden_scene()`, and bless freezes them:
    - the rounded rect's stroke alpha (taken as 1.0);
    - "no stroke" (width 0);
    - the square's vertex order and start (CCW from bottom-left, via
      `polygon(&square(..))`).
12. **The card example's layout.** §2.12 names only `examples/card/main.rs`.
    The suite includes `examples/card/scene.rs` via `#[path]`, requiring
    `pub fn scene() -> Scene`, which is the R-0003/R-0004 precedent. It also
    requires `main.rs` to contain `mod scene;`, `(1080, 1920)`, and an
    `ffmpeg -framerate 30 -i …frame_%05d.ppm -c:v libx264 -pix_fmt yuv420p`
    line. The output directory is not pinned. **Resolved (§2.13):** the
    split into `main.rs` and `scene.rs` with `pub fn scene() -> Scene`.
13. **Drop versus cull order.** When an empty subpath has a non-finite
    `start`, §2.3 does not say which comes first, dropping or culling.
    **The suite** does not test it.
14. **Sink-level non-finite input.** The stroke path has a `debug_assert!`
    tripwire, and §2.6 calls the fill's check "a tripwire" too. A test
    feeding NaN through `FrameSink::frame` would therefore panic in debug
    builds if the fill also asserts. **The suite** covers this through the
    private-level proposal in §4 instead.
15. **Pinhole termination.** §2.4's example ("232 of 830 pieces") gives no
    geometry. The suite's case, a ground-plane quarter circle at nearest
    depth 10⁻⁵, does force the depth limit on a spec-faithful probe.
16. **Resolved (§11): "fewer than 3 distinct vertices" when a vertex recurs
    non-consecutively.** §2.13: "Consecutive duplicate vertices,
    including the last equalling the first, are dropped … Fewer than 3
    distinct vertices remaining gives an empty `Vec`." For `[a, b, a, b]`
    nothing is consecutive, four vertices remain, and only two are
    distinct. Read literally ("distinct"), the result is empty. The
    implementation counts the remaining vertices and returns the
    zero-area 4-gon `a → b → a → b → close`, which has no zero-length side.
    Both readings are defensible, so **the suite does not assert this
    case**. **Ask:** the owner picks one; the one-line test follows.
    **Resolved:** the owner chose "remaining"; see §11.

## 7. How the suite itself was validated

A throwaway, spec-faithful probe of SPEC-0012 was written in QA's scratch
directory, following the R-0003 QA precedent ("confirmed against a
spec-faithful probe implementation"). It is not committed and is not a
design proposal. Against it:

- **83 of 84 tests pass** (the step-3 suite). The two golden tests fail
  with the bless message until blessed, and then all 84 pass. The suite
  runs in about 4 s (debug). §8 covers the follow-up against the real
  implementation.
- **Clippy `-D warnings` and rustfmt are clean** on 1.98.1, with and
  without `--no-default-features`.
- **Measured margins:**
  - chord sweep: worst 0.0955 px against 0.1;
  - corner grid: worst 0.9697 against 1.0, after slack;
  - random corners: worst 0.9367.
- **31 mutations were run**, each breaking one rule in the probe. 26 are
  killed by the e2e suite:

| Mutation | Killed by |
|---|---|
| inclusive tie rule | `ac5` tie-rule test and exhaustive corner grid |
| no boundary classification (seams) | pentagram, same-orientation nesting |
| stroke before fill | both AC2 order tests |
| chord-count coefficient 6.0, 5.0, 3.0 or 0.5 instead of 7.5 (6.0 is ε = 0.125 px) | AC6 chord sweep (and the circle band from 5.0 down) |
| no implicit close for fill | open-subpath fill test |
| fill composited per subpath | once-per-pixel test, both hole tests |
| even-odd (in the probes or in the interior) | pentagram, nesting, once-per-pixel |
| closed-interval crossing (in the row filter and in the winding sum) | `a_vertex_on_a_pixel_centre_row_is_counted_once` |
| pinhole without subdivision | both pinhole tolerance tests |
| depth limit 22 instead of 12 | `pinhole_subdivision_stops_at_4096_pieces_per_authored_cubic` |
| empty subpaths kept | AC4 empty-subpath test |
| rad not clamped; sweep not clamped | AC7 clamp tests |
| fill alpha not clamped | AC2 alpha test |
| SVG always writes a stroke; SVG tests `alpha == 0` only | AC8 stroke-none tests, AC3 SVG lines |
| clockwise circle | K, orientation and quarters tests |
| stroke draws an open subpath's closing chord | open-stroke test, AC6 sweep |
| ramp sign flipped | AC5 identities, corner tests, AC8 agreement |
| `_ =>` or `other =>` arm in `ppm.rs`; `_` arm in `scene.rs` | the AC1 exhaustiveness scans |

The five survivors are equivalent at frame level:

- **Skipping zero-length edges.** A NaN normal falls through harmlessly.
- **The verbatim foot point.** `a + 1·(b − a) = b` for every dyadic
  coordinate the suite uses. The §4 unit test kills it.
- **Tie-break by max `s`.** Ties with differing `s` do not arise under the
  strict rule on axis-aligned geometry.
- **A closed interval in only one** of the row filter and the winding sum
  (two mutants). The other copy of the half-open rule compensates.

The §4 proposals kill the verbatim-foot, NaN-edge-dropped and ε = 0.125
chord-count mutants at unit level.

**Toolchain:** `rustup` stable is installed and is 1.98.1, the default, so
every command above ran as `rustup run stable …`, matching CI's
`dtolnay/rust-toolchain@stable`.

## 8. Follow-up: the owner's step-3 decisions (2026-09-27)

After step 3, the owner settled the findings of §6 (651e14d), and the
implementation landed on the branch (1dc06a4, e6faddd, 945235a, 8291259).
One step-3 test encoded the superseded arc direction, and the new rules
made six more cases decidable. Test code only; `src/` is untouched.

**Changed (1):**
- `ac7::arc_endpoints_lie_on_the_circle_and_the_arc_turns_the_right_way`
  → `…_and_the_arc_runs_counter_clockwise`. The case
  `(c = (−3, 2), r = 0.75, start = −1, sweep = −4.5)` expected the arc to
  start at `start` and turn clockwise. Per §2.9 it now expects the start at
  angle `start + sweep`, the end at `start`, and piece midpoints at
  `start + sweep + |sweep|·(i + ½)/m`. Cases for sweeps of −τ and −0.3, and
  a positive-area check, were added.

**Checked and unchanged:** `a_sector_is_the_centre_a_line_to_the_arc_…`
(structural against `arc`, so consistent for either sign),
`a_sweep_beyond_a_full_turn_is_clamped_to_one_turn` (finite values only,
as §2.13 says) and `an_arc_uses_one_cubic_per_started_quarter_turn`.

**Extended (1):** `ac7::generators_are_counter_clockwise` now covers `arc`
and negative-sweep `arc`/`sector` (−2, −4.5, −τ).

**Added (6):**

| Test | AC | Spec |
|---|---|---|
| `ac7::a_negative_sweep_is_the_same_region_traversed_counter_clockwise` | AC7 | §2.9 |
| `ac7::a_polygon_keeps_a_clockwise_input_clockwise` | AC7 | §2.9 |
| `ac7::a_polygon_drops_consecutive_duplicates_including_last_equal_to_first` | AC7 | §2.9 (no zero-length sides), §2.13 |
| `ac7::a_polygon_with_fewer_than_three_remaining_vertices_is_empty` | AC7 | §2.13 |
| `ac7::infinite_inputs_return_an_empty_path_for_every_generator` | AC7 | §2.13 |
| `ac8::a_non_finite_fill_alpha_writes_fill_none_and_paints_nothing` | AC8, AC2 | §2.13 |

**Results** on the branch at 8291259 with the updated suite, under
`rustup run stable` (1.98.1):
- `cargo test -p motoreel --test r0012_paths_fills`: 90 passed, 0 failed,
  with and without `--no-default-features`;
- `cargo clippy -p motoreel --test r0012_paths_fills -- -D warnings`: clean;
- `cargo fmt -p motoreel -p motoreel-typeset --check`: clean.

**The new tests are not vacuous.** Each mutation below was applied to a
scratch copy of the implementation, never to `src/`:

| Mutation | Killed by |
|---|---|
| the pre-decision clockwise arc (`from = start`, signed θ) | the rewritten arc test, the negative-sweep test, `generators_are_counter_clockwise` |
| `polygon` keeps consecutive duplicates; keeps last = first | both duplicate tests |
| `polygon` re-orients clockwise input | `a_polygon_keeps_a_clockwise_input_clockwise` |
| SVG writes a `+∞` fill alpha; PPM paints a `+∞` fill alpha (as `unit` alone would) | `a_non_finite_fill_alpha_writes_fill_none_and_paints_nothing` |
| `rounded_rect` clamps an infinite `rad`; `arc` clamps an infinite `sweep` | `infinite_inputs_…`, `degenerate_inputs_…` |

**No new test disagrees with the implementation.** The one open reading
was §6 item 16, resolved in §11.

## 9. Recorded deviation: TDD order after step 3 (PR #3 review, finding 2)

The red suite (`229cc24`) precedes the implementation (`1dc06a4`), as
required. The owner's step-3 decisions (`651e14d`: counter-clockwise
normalisation of a negative sweep, `polygon` dedup, infinite inputs,
non-finite fill alpha) landed **before** the implementation, but the tests
encoding them landed **after** it, in `0cfc591`. As a result:

- the commits `1dc06a4`…`8291259` carry one failing test, the red suite's
  clockwise-arc expectation, which those decisions made stale;
- the new decision tests were never red against a missing implementation.
  Instead they were shown to bite by nine targeted mutations (§8).

The history is left as it is: it is pushed, and rewriting it would buy
nothing the mutation evidence does not already show. **Practice from now
on:** when an owner decision changes the spec during step 3 or 4, the
suite is updated to red for that decision *before* implementation begins.

## 10. AC12 in CI must encode, not skip (PR #3 review, finding 3)

`ffmpeg_with_libx264` routes every probe failure through `skip_ac12(why)`.
There are four reasons: `which` cannot run, no ffmpeg on PATH, ffmpeg
cannot run, or ffmpeg has no libx264. The helper behaves in one of two
ways:

- **`MOTOREEL_REQUIRE_FFMPEG=1`:** it panics, so the test fails. CI's AC12
  step sets this.
- **Otherwise:** it prints `AC12 skipped: …` and the test passes, so a
  machine without ffmpeg still runs the suite green.

Before this change, a probe that could not spawn `which` returned `None`
silently, and the CI grep guard passed without an encode. The same gap
still exists in `tests/r0006_ppm_sink.rs`, but R-0012 AC10 freezes that
file, so the fix belongs to its own change.

## 11. Owner decisions after the PR #3 review (2026-09-27)

Both points the architect left open are decided, and each now has a test.

| Decision | Test | Evidence |
|---|---|---|
| **The `(d, s)` tie-break is reversed**: at equal distance the inside sign `+d` wins (§2.6) | `ac5::hole_corner_error_is_bounded_with_the_inside_sign_tie_break` | Square holes cut from a pixel-aligned cell, all 278 784 grid cases. Worst 1.541 px² (was 2.24), bound `1.6 + n/510`. **Mutation check:** restoring the old direction fails it on its first rectangle at 1.62 |
| **`polygon` counts vertices remaining, not distinct** (§2.13) | `ac7::a_polygon_counts_remaining_vertices_not_distinct_ones` | `[a, b, a, b]` is exactly `a → b → a → b → close`. The existing empty-case test is renamed to match and is unchanged otherwise |

Nothing else moved. The convex corner grid is still 0.984, the strict
half-plane test still passes, and every golden, including
`r0012_paths.{ppm,svg}` with its square hole, is byte-identical.

### Step 7 sign-off

QA verdict **PASS** on `dd4024c`. Every AC1–AC12 is met by at least one
passing test that asserts it. The gates were clean: workspace 296/296,
`--no-default-features` 249/249, clippy with both feature sets, and fmt.
One gap was closed with test code only (`2fd32f9`):
`ac4::pinhole_is_not_the_cubic_through_the_projected_control_points`
pins AC4's negative clause. In both tilted-circle scenes the naive cubic
strays 6.7·10⁻³ and 4.7·10⁻² from the true image, against τ = 9·10⁻⁵.
AC12's real encode runs in CI under the must-encode guard (§10), because
the QA container has no ffmpeg.
