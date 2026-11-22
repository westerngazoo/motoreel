# R-0018 — Faceted 3D («3D de facetas»)

- **Status:** Draft
- **Milestone:** MC (the creator pipeline)
- **Owner:** Gustavo Delgadillo (westerngazoo)
- **Created:** 2026-11-22
- **Depends on:** R-0017 (surfaces and finishes), R-0002 (`Camera`,
  `Projection::{Pinhole, Orthographic}`, the cull rule), R-0001 (`Track`,
  motor keyframes), R-0012 (`Prim2::Path`, nonzero fills)
- **Realized by:** — (SPEC-0018, to be written after acceptance)
- **Architecture:** guion-video-creator
  [ADR-0006](https://github.com/westerngazoo/guion-video-creator/blob/main/docs/decisions/0006-layered-architecture-kit-and-finishes.md)
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

motoreel must render **low-poly solid meshes** posed by garust 3D PGA
motors. It does this through the 2D pipeline it already has, so gradients,
antialiasing, both sinks and determinism keep working with no new raster
code.

Concretely:

1. **A mesh is data.** It holds model-space vertices (`pga::Point`) and faces
   as index lists. Each face is a planar convex polygon, wound
   counter-clockwise seen from outside, with a material id. A mesh is posed
   by its object's `Track`, the same motors every other shape rides.
2. **The camera already exists.** Each face is carried into view space with
   the composed motor `Scene::eval` already uses. It is culled under R-0002's
   rule and projected by the existing `Projection`.
3. **Hidden surfaces by the painter's algorithm.** Back faces are dropped.
   The survivors are sorted far to near by a total, deterministic key and
   handed to a R-0017 finish as one batch, which keeps the depth order. The
   output is ordinary `Prim2::Path`s.

## 2. Rationale

The house reels already draw 3D by hand, in 2D:

- `brand/rf-marca/src/tres.rs` (rotorf-sico) is a kart in three-quarter
  view. It has its own `P3` vector maths, an oblique parallel `Cam`, a
  "draw a face only if it faces the camera" rule, and a tone taken from the
  normal against the 135° light.
- The «juguete» toy (`fbf/motor/src/juguete.rs`) is a 2D side view, so every
  camera move is faked. A frozen-moment orbit or a slow 3/4 turn of the toy
  cannot be drawn today without redrawing the figure per angle.

motoreel was built for this. Every pose is a `Motor3` (R-0001), the camera
is a motor (R-0002), and `project` already culls behind the near plane.
What is missing is a solid. `Shape::Edges` (R-0004) draws wireframes, but no
shape has faces, depth order or shading. This requirement adds the smallest
piece that closes the gap, and it reuses everything downstream: R-0016
gradients, R-0012 antialiased fills, R-0006/R-0003 sinks.

**Why not a z-buffer.** A depth buffer works per pixel, but motoreel's sinks
consume vector `Prim2`. The SVG sink would lose its vector output, and the PPM
sink would need a second raster path with its own antialiasing. The painter's
algorithm keeps the 2D pipeline as the only raster code. It is exact for the
house's content (convex, non-interpenetrating low-poly parts) and documents
its limits (§4).

## 3. Acceptance criteria

- **AC1. The mesh.** A new additive `Shape` variant (name is the spec's call)
  holds vertices, faces and per-face material ids. Construction validates:
  - each face has at least 3 indices, all in range;
  - each face is planar within a stated tolerance;
  - each face is convex;
  - vertices are finite.

  A violation is a typed error that names the face index. The existing
  `Shape` variants and their behaviour are unchanged.
- **AC2. Posed by motors.** A mesh object rides its `Track` like every shape.
  At key times the posed vertices equal the motor applied to the model
  vertices (R-0001's "key times return key motors exactly"). No Euler angles
  and no matrices outside the projection (project-specifics).
- **AC3. Culling.**
  - A face with any vertex failing `Projection::project` is culled whole
    (R-0002's rule; no clipping against the near plane in this requirement).
  - A back face is dropped, under the orthographic camera by the sign of its
    view-space normal's z, and under the pinhole camera by the sign of
    `(face point − eye) · normal`. A face seen exactly edge-on is dropped.
- **AC4. Depth order is total and deterministic.**
  - Faces are emitted far to near by a key the spec fixes (recommended: the
    face's maximum view depth, then its minimum, then its mesh-local index).
    The comparison is `f64::total_cmp`, so ties never depend on sort
    stability or thread scheduling.
  - The sort is within one mesh object. Objects keep R-0002's draw order,
    insertion order (see §5 OQ-1).
- **AC5. The finish paints the faces.** The surviving faces become R-0017
  surfaces:
  - outline = the projected polygon;
  - normal = the view-space face normal;
  - depth = the sort key;
  - id = a stable hash of (object index, face index);
  - material = from the face's id.

  They go to the object's finish as **one batch, in depth order**. With
  `Facetas`, the output is one `Prim2::Path` per face, in that order.
- **AC6. Nothing already rendered moves.**
  - Every golden fixture is byte-unchanged and every existing test passes
    unedited.
  - **Equivalence with 2D:** a one-face mesh facing an orthographic camera,
    painted with `Facetas`, yields a `Prim2` equal (`PartialEq`, exact
    `f64`) to the same polygon built with `Object::planar` and the style
    `Facetas` produces. 3D adds nothing a flat piece did not already have.
  - **Downstream gate:** rotorf-sico `tools/golden-reels.sh` stays green. No
    existing reel uses meshes, so this is a regression guard, not a
    migration.
- **AC7. Seams do not leak.** Two coplanar faces that share an edge, painted
  opaque with `Facetas`, leave no pixel along the shared edge darker than
  both faces' colours in the PPM sink. This is the 1-px self-stroke rule rf-marca
  measured: up to 8 levels darker without it.
- **AC8. Gradients and finishes compose.** A face whose material asks for a
  gradient (a R-0016 paint chosen by the finish) carries it through the
  surface frame. Under orthographic projection the image-space gradient is
  the exact image of the model one. Under pinhole it is affine, as R-0016
  AC4 already states.
- **AC9. Determinism.**
  - Two renders give byte-identical frames in both sinks, also on several
    threads.
  - A checked-in golden per sink shows a cube and an octahedron, posed by a
    trig-free motor (a translation plus a quarter turn whose rotor uses only
    `sqrt(2)/2`, a correctly rounded `sqrt`), under
    orthographic projection. It matches byte for byte on any host, under
    R-0003 §2.6's trig-free rule.
- **AC10. A demo encodes.** `cargo run --example` renders a 120-frame turn of
  a low-poly figure under the pinhole camera, and stock ffmpeg encodes it
  from the PPM frames (R-0006's guarantee).
- **AC11. The kernel stays thin.** Zero new dependencies. The
  `--no-default-features` build still passes. Meshes live in the core.

## 4. Constraints & non-goals

- **Not interpenetrating or cyclically overlapping faces.** The painter's
  algorithm cannot order three faces that overlap in a cycle, or two that
  cross. This is documented, not hidden: the author splits the part. A BSP
  tree (Fuchs, Kedem, Naylor, 1980) would fix it and is left for a later
  requirement if a reel needs it.
- **Not near-plane clipping.** A face crossing the near plane is culled
  whole, like every primitive under R-0002.
- **Not smooth shading, textures, shadows or reflections.** Flat faces, one
  normal each. Soft looks come from the finish (R-0017) and from gradients
  (R-0016), not from per-vertex normals.
- **Not mesh loading.** Meshes are built in code, by generators the spec may
  add (box, prism, low-poly capsule). No OBJ or glTF import here.
- **Not an oblique projection.** rf-marca's `tres.rs` uses an oblique
  parallel camera. Porting it would need a third `Projection`, which is an
  open question (§5), not part of this requirement.

## 5. Open questions

1. **Sort scope.** Is the sort within one mesh object, or across every mesh
   in a "depth group"? A toy built from several objects (a body and a
   dumbbell) would then interleave correctly. *Recommendation:* within one
   object for now. The kit builds one toy as one mesh with parts, which keeps
   `Scene::eval`'s draw-order contract untouched.
2. **An oblique projection.** Add `Projection::Oblique` so `tres.rs` can move
   onto meshes byte-identically? rotorf-sico `docs/infra.md` §2b already
   lists it as "for later, little value today". *Recommendation:* not now.
3. **Where finishes attach.** Is the finish a field of the mesh object, or
   chosen per render (`Scene` level), so one scene renders in two looks?
   *Recommendation:* per object, with a scene-level default.
4. **Low-poly generators in motoreel or in the kit.** Should the capsule,
   prism and faceted sphere generators live in motoreel or in the kit?
   *Recommendation:* the kit. motoreel ships the cube and the octahedron its
   own tests need, nothing more.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-11-22 | Draft written under the owner's approval of the direction («dale con tu recomendación»). The acceptance criteria await his OK | ADR-0006 |
| 2026-11-22 | **Painter's algorithm, not a z-buffer** (proposed) | Keeps `Prim2` and the existing sinks as the only raster path, which also keeps SVG output vector. Newell, Newell & Sancha 1972 |
| 2026-11-22 | **Faces become R-0017 surfaces** (proposed) | One painting layer for 2D and 3D. A flat piece and a camera-facing face are the same thing (AC6 equivalence) |

## Prior art

- M. E. Newell, R. G. Newell, T. L. Sancha, "A Solution to the Hidden
  Surface Problem", *Proc. ACM Annual Conference*, 1972: depth-sorted
  painting.
- I. E. Sutherland, R. F. Sproull, R. A. Schumacker, "A Characterization of
  Ten Hidden-Surface Algorithms", *ACM Computing Surveys* 6(1), 1974: why
  depth sort fails on cyclic overlap.
- H. Fuchs, Z. M. Kedem, B. F. Naylor, "On Visible Surface Generation by A
  Priori Tree Structures", *SIGGRAPH '80*: BSP, the rejected exact
  alternative.
- E. Catmull, *A Subdivision Algorithm for Computer Display of Curved
  Surfaces*, PhD thesis, Utah, 1974: the z-buffer, rejected (§2).
- garust PGA `Motor3` (RFC-001), already the only motion representation here.
- In-house: rotorf-sico `brand/rf-marca/src/tres.rs` (back-face rule, tone
  from the normal against the 135° light).

## Test plan

| AC | Unit tests (`tests/r0018_mesh.rs`) | E2E |
|----|-----------------------------------|-----|
| AC1 | One test per error variant (index out of range, non-planar, non-convex, fewer than 3, non-finite) plus a valid cube | — |
| AC2 | The posed vertices of a keyed mesh equal the key motor's action at key times | — |
| AC3 | A cube seen down an axis keeps exactly one face (orthographic) and up to three (pinhole). A face behind the eye is culled whole | — |
| AC4 | Shuffled face order gives the same output. Equal-depth faces order by index. A threaded render equals a serial one | — |
| AC5 | Surface fields per face. One `Prim2::Path` per surviving face under `Facetas` | — |
| AC6 | Planar equivalence `PartialEq`. The existing goldens unchanged | rotorf-sico `tools/golden-reels.sh` |
| AC7 | Pixel scan along a shared edge in a PPM frame | — |
| AC8 | Gradient frame through orthographic projection is exact | — |
| AC9 | Golden PPM and SVG, cube and octahedron | — |
| AC10 | — | 120-frame example encodes with stock ffmpeg |
| AC11 | The existing `--no-default-features` build test | — |

## Changelog

- 2026-11-22: created as Draft. The owner approved the direction; the
  acceptance criteria await his OK.
