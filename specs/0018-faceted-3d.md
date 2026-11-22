# SPEC-0018 — Faceted 3D («3D de facetas»)

- **Status:** Accepted (2026-11-22, on the owner's «dale con el 3D»).
  Implemented in the R-0017/R-0018/R-0019 PR; architect and owner review
  pending
- **Realizes:** R-0018
- **Author:** Claude (main session) with owner
- **Created:** 2026-11-22
- **Depends on:** SPEC-0017 (surfaces, finishes), SPEC-0002 (camera,
  projection, cull), SPEC-0001 (tracks), SPEC-0012 (`polygon`, paths)
- **Module(s):**
  - new `crates/motoreel/src/mesh.rs`: `Mesh`, `MeshFace`, `MeshError`,
    `PLANAR_TOL`, `Mesh::cube`, `Mesh::octahedron`
  - new `crates/motoreel/src/solid.rs`: posing, culling, the two-level
    painter's order, surfaces (crate-private)
  - `object.rs`: `Shape::Mesh`, `Object::mesh`
  - `scene.rs`: `Look`, `Scene::eval_with`, `Scene::surfaces`; `eval`
    routed through the same walk with no look
  - `camera.rs`: `Camera::view_direction`
  - `gradient.rs`: `map_frame` made public
  - new `tests/r0018_mesh.rs`, goldens `tests/golden/r0018_solids.{ppm,svg}`
  - new `examples/curl_orbit/` (the E2E demo, R-0019's too)

## 1. Motivation

R-0018 adds low-poly solids posed by motors and painted through a finish,
with no new raster code. The spike (PR #8) showed the look and its two
production defects: zigzags where solids meet, and large faces of separate
boxes sorting over their neighbours.

## 2. Design

### 2.1 The mesh

`Mesh::new(vertices: Vec<pga::Point>, faces: Vec<MeshFace>, materials:
Vec<Material>)`. A `MeshFace` is `{ indices, material }`. Validation, in
order: finite vertices (`Vertex(i)`); per face: ≥ 3 indices
(`TooFewIndices`), in range (`IndexOutOfRange`), material in range
(`MaterialOutOfRange`), every vertex within `PLANAR_TOL · extent` of the
plane through the first three (`NonPlanar`), and **strictly convex, wound
once**: every other vertex strictly inside every edge (`NonConvex`).
Materials live on the mesh, so a face's material id is self-contained.

### 2.2 Posing and culling (AC2, AC3)

Per mesh object in a group: `to_view = camera⁻¹ ∘ track(t)`. Each vertex is
`p.transform(&to_view)`, projected with `Projection::project`, exactly the
arithmetic `Scene::eval` uses for paths (which is what makes AC6's
equivalence exact). A face with any unprojectable vertex is culled whole.
Its normal is `(v1 − v0) × (v2 − v0)` in view space, rf-marca's operation
order. Back and edge-on faces are dropped: orthographic `n.z ≤ 0`, pinhole
`v0 · n ≥ 0` (the eye is the view origin).

### 2.3 The surfaces (AC5)

outline = `polygon(projected vertices)`; facing = `Normal(n)`; depth = the
face's maximum view depth; id = `(object index << 32) | face index`;
material = the mesh's; frame = the images of the first three vertices.

### 2.4 The painter's order (AC4)

A **depth group** is a maximal run of consecutive mesh objects
(R-0018 OQ-1, resolved). Its order has two levels:

1. **Objects as wholes.** For every pair whose image boxes touch, a
   separating plane is sought among both objects' face planes: one object's
   vertices all on one side (within `1e-9`), the other's on the other side.
   The object on the eye's side is drawn second (orthographic: the sign of
   `n.z`; pinhole: the eye's signed distance). With no such plane
   (interpenetrating parts, a bone in its limb, an ink hull around its
   part, or the rare edge-to-edge separation), the object whose visible
   faces have the larger mean depth is drawn first. A topological sort
   (Kahn) picks, among the free objects, the farthest (max depth, then min
   depth, then index); a cycle is broken at the farthest remaining object.
2. **Faces within an object**, far to near: max depth, then min depth,
   then face index, by `total_cmp`.

Drawing objects whole is what removes the joint zigzag; ordering them by
separating planes is what sorts a large face beside a small box.

### 2.5 Painting (AC5) and the look

```rust
pub struct Look<'a> { pub finish: &'a dyn Finish, pub light: Light, pub seed: u64 }
impl Scene {
    pub fn eval(&self, t) -> Vec<Prim2>;                  // meshes emit nothing
    pub fn eval_with(&self, t, look: &Look) -> Vec<Prim2>;
    pub fn surfaces(&self, t) -> Vec<Vec<Surface>>;       // per group
}
```

One walk serves both: non-mesh objects go through `project_shape`
unchanged; a group's batch goes to `look.finish.paint` at the slot of its
first object. The finish is per render, not a field, because `Object` and
`Scene` are built by struct literal downstream (R-0018 OQ-3, resolved).
`Camera::view_direction` turns a world-fixed light into the view-space
`Light`.

### 2.6 Seams (AC7)

`Faceted` strokes each opaque face in its own colour. On an oblique shared
edge a 1 px stroke covers half a pixel each side, which leaves a few levels
(measured ≤ 6 summed over the channels); 2 px closes it exactly. 3D callers
pass 2 px; 2D keeps rf-marca's 1 px.

### 2.7 Compatibility (AC6)

Byte-unchanged goldens and example outputs. One existing test edited by one
arm: R-0012's `prim2_and_shape_gain_exactly_one_variant_each`, a tripwire
written to fail on a new `Shape` variant. The `Shape` match in `scene.rs`
stays exhaustive by name (R-0012's source scan), so the mesh filter uses
`if let`.

### 2.8 Determinism (AC9)

Only `+ − × ÷`, `sqrt`, comparisons and `total_cmp` in `mesh.rs` and
`solid.rs`; no `HashMap`, no thread-dependent state. The golden poses use a
`sqrt(2)/2` quarter turn and a camera built from two Pythagorean rotors
(`0.96, 0.28`), also trig-free.

## 3. Code outline

See `solid.rs`: `surfaces` = `node` per object → `order` → faces in order.

## 4. Non-goals

BSP splitting of crossing faces; near-plane clipping; smooth shading;
mesh import; an oblique projection (OQ-2, not now); generators beyond the
cube and the octahedron (OQ-4: the kit).

**Known limits.** Two crossing solids are drawn whole, one over the other
(no zigzag, but not their true intersection). `Faceted`'s `lambert` flips
a normal with `z < 0`, which under a wide pinhole can relight a front face
near the frame edge.

## 5. Open questions

None; R-0018 §5 records the resolutions.

## 6. Acceptance criteria → tests (`tests/r0018_mesh.rs`)

| AC | Tests |
|----|-------|
| AC1 | `ac1::the_cube_and_the_octahedron_are_valid`, `ac1::every_error_names_its_face`, `ac1::a_non_finite_vertex_is_named` |
| AC2 | `ac2::key_times_return_the_key_poses` |
| AC3 | `ac3::a_cube_seen_down_an_axis_keeps_one_face`, `ac3::a_pinhole_sees_at_most_three_faces_of_a_cube`, `ac3::a_face_behind_the_eye_is_culled_whole` |
| AC4 | `ac4::shuffled_faces_paint_the_same_frame`, `ac4::equal_depths_order_by_index`, `ac4::a_threaded_render_equals_a_serial_one`; the ray-cast checks `order::large_faces_of_separate_boxes_sort_correctly_all_around` (108 views; a face-level sort fails it), `order::a_crowd_of_boxes_sorts_correctly`, `order::interpenetrating_solids_are_drawn_whole`, `order::an_ink_hull_is_drawn_before_its_part`, `order::an_enclosed_part_is_drawn_before_its_shell`; unit `solid::tests::the_nearer_of_two_separated_cubes_is_drawn_second` |
| AC5 | `ac5::surfaces_carry_the_face` |
| AC6 | `ac6::a_camera_facing_face_equals_a_planar_path`, `ac6::eval_skips_meshes_and_keeps_the_rest`; the existing suites and goldens |
| AC7 | `ac7::the_self_stroke_closes_the_shared_edge` |
| AC8 | `ac8::the_frame_is_the_exact_image_of_the_face` |
| AC9 | `ac9::both_solids_are_on_the_frame`, `ac9::two_renders_and_four_threads_are_byte_identical`, `ac9::the_ppm_golden_matches_byte_for_byte`, `ac9::the_svg_golden_matches_byte_for_byte`, `ac9::the_3d_code_is_trig_free` |
| AC10 | `ac10::a_120_frame_turn_encodes_with_stock_ffmpeg`; `cargo run --release -p motoreel --example curl_orbit` |
| AC11 | `ac11::meshes_need_no_feature`; R-0017 `ac9::no_new_dependency`; CI's `--no-default-features` step |

## 7. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-11-22 | Accepted with R-0018 | The owner's go-ahead |
| 2026-11-22 | Mean visible depth as the fallback key | See R-0018's decision log: nearest point put an ink hull over its part |

## Changelog

- 2026-11-22: created and implemented.
