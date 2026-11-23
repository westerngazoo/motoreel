# R-0022 — Depth-order performance («rendimiento del orden de profundidad»)

- **Status:** Accepted, In progress (implemented; PR open)
- **Milestone:** MC (the creator pipeline)
- **Owner:** Gustavo Delgadillo (westerngazoo)
- **Created:** 2026-11-23
- **Depends on:** R-0018 (meshes, the object-level painter's order),
  R-0021 (the enclosure rule and the peel), R-0017 (surfaces, finishes)
- **Realized by:** [SPEC-0022](../specs/0022-depth-order-performance.md)
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

`Scene::eval_with` must order and paint the house's 3D figures fast enough
for interactive use: a continuous orbit of the three-layer toy at 60 fps,
which leaves about 8 ms of geometry per frame in a mid-range phone's
browser (WebAssembly). It must get there **without changing one output
byte**: the order R-0018 and R-0021 define stays the order, bit for bit,
for every scene.

It also ships, **opt-in**, a way to hand a canvas fewer primitives: runs of
consecutive opaque paths of one style merged into one path.

## 2. Rationale

The fitAI spike (`~/code/lab/2026-11-fitai-puente-3d`, README "Findings",
2026-11-22/23) renders motoreel scenes in WebAssembly for a Flutter app.
It measured the toy lifter at 19 ms a frame under `Faceted`, 48 ms under
`XRay` and 200 ms half peeled, in the browser, on a machine whose load
average stood between 44 and 92 (so 2–3× pessimistic), and found 60 % of
the time in motoreel's object-level order: the separating-plane tests
between every pair of overlapping parts, each over every face plane and
every vertex of both. Merging consecutive opaque primitives of one style
cut Flutter's paint recording from 20 to 6.8 ms. The same order is most of
a reel's geometry time, so a faster order also renders the reels faster.

The order is not free to change. R-0018's goldens, R-0021's enclosure rule
and every reel's frames depend on which separating plane decides a pair
and on how a cycle breaks. A faster order that draws one face in a
different slot is a different picture.

## 3. Acceptance criteria

- **AC1. The same order, bit for bit.** For every scene the new code gives
  the same surfaces, and so the same primitives, as the code R-0021 left.
  A differential test keeps that code as a test-only oracle and compares
  the two, as exact text (every `f64` exactly, `-0.0` included), on:
  - at least 1 600 random cases: 400 groups of up to a dozen objects
    (cubes, octahedra, boxes, parts enclosed off-centre in a shell,
    inverted ink hulls, peeled shells, non-convex objects, boxes face to
    face, exact duplicates, random layers), each under 4 random cameras,
    pinhole or orthographic;
  - the house anatomy (skin with its ink hulls, muscle, bone) and two
    dumbbells, whole and half peeled, at 8 azimuths, pinhole and
    orthographic;
  - and the primitives `Faceted` and `XRay` paint from both orders.
- **AC2. Nothing already rendered moves.** Every golden is byte-unchanged
  and every existing test (R-0001 to R-0021) passes unedited. The three 3D
  reels (`curl_orbit`, `gym_orbit`, `anatomy_peel`) render the same PPM
  frames, byte for byte, before and after, and the bench's fingerprint of
  every frame's primitives is the same before and after.
- **AC3. Determinism.** Two renders, and four threads rendering at once,
  give byte-identical primitives (the toy, whole and half peeled, at 8
  azimuths).
- **AC4. Faster, measured.** On the bench (AC5), on one machine, the base
  and the change run back to back (A/B), each frame the fastest of 5
  rounds, the median over frames:
  - the three-layer toy orbiting under `Faceted`: **≥ 3×** faster;
  - the same under `XRay`: **≥ 3×**;
  - the same half peeled: **≥ 4×**;
  - every frame of each 3D reel (`curl_orbit`, `gym_orbit`, and
    `anatomy_peel`'s paint): **≥ 2×**.

  The decision log records the figures and the machine's load.
- **AC5. A reproducible bench.** `cargo bench -p motoreel` times every
  frame of the three reels and a full-circle orbit of the toy (the
  spike's case) through the examples' own scenes, with no new dependency:
  per phase the median, p95 and maximum, the load average at start and
  end, and a fingerprint of the primitives.
- **AC6. Coalescing is opt-in.** `motoreel::coalesce(prims)` merges each
  run of consecutive `Prim2::Path`s that share one opaque style (a flat
  fill at alpha 1, a stroke opaque or not drawn) into one path, its
  subpaths in order. Nothing in the render path calls it. On the toy:
  - every piece keeps its style and its place in the order;
  - the paths drop at least 2×;
  - translucent paths are never merged;
  - in the PPM sink, under 1 % of the pixels change: on antialiased edges,
    or on a seam inside a run by at most 2 levels (a sink composites a
    path's coverage once; separate paths composite one over another).
- **AC7. The kernel stays thin.** Zero new dependencies (normal or dev);
  the `--no-default-features` build passes; the 3D code stays trig-free.

## 4. Constraints & non-goals

- **Not a different order.** Skipping pairs that do not overlap in depth,
  a tighter screen-space overlap test, or dropping objects a finish hides
  would each change which pairs constrain the sort, and so the output.
  They stay out unless a later requirement changes the order on purpose.
- **Not frame coherence.** Reusing last frame's order when the camera moves
  a little cannot be exact without testing every pair again (the first
  separating plane decides, and any pair can change it), which is the cost
  it would save.
- **Not a `Renderer` with scratch buffers.** Measured: about 15 500
  allocations a frame on the toy, nine in ten of them the per-face
  outlines that leave as output (the surfaces, then the finish's paths).
  Reusing the rest would save under a tenth and add public API.
- **Not threads inside one frame.** The target is one WebAssembly thread;
  reels already render frames in parallel.

## 5. Open questions

None.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-11-23 | **Accepted** and **In progress** on the owner's brief, relayed by the coordinator: «R-0022 rendimiento del orden de profundidad», byte-identical output, a differential test, a bench, measured targets | The spike's findings; the owner authorised merging once CI is green and the ACs are met |
| 2026-11-23 | **Exact by construction, not by tolerance.** Every side of a plane is the same expression on the same numbers; the first separating plane in the same order decides; only how far a vertex list is read changes | A tolerance-based shortcut would be a different order on some scene; the differential test would then have to bless it |
| 2026-11-23 | **Each object's own sides of its own planes, once a frame** | They were recomputed for every pair. Then the other object is read only for the side that can separate, stopping at the first vertex that rules it out |
| 2026-11-23 | **An exact point sandwich** (`sandwich.rs`) | With the order fixed, posing the vertices was a quarter of the toy's frame. garust's `transform` walks 16 × 16 blade pairs twice; walking only the blades a motor and a point can hold, in the same order, with garust's own table, is bit-identical and 2.4× faster. Proptested against `transform`. It belongs upstream in garust (an exact `Motor::apply_point`); it lives here until it is there |
| 2026-11-23 | **`Motor::apply_each` tried and dropped** | It only shares the reverse across a batch: no measurable gain |
| 2026-11-23 | **No harness crate** | Three acceptance tests pin the dev-dependencies to proptest alone (R-0003 AC5, R-0006 AC7, R-0012 AC11). A frame of a reel is the unit that matters, and its spread over a timeline is the figure to report, which std `Instant` and medians give |
| 2026-11-23 | **AC4 met.** A/B on an Apple M5 (10 cores), base `d6fb31f` (the bench, old order) against the change, back to back, twice, load average 4.9–18; ms per frame, the fastest of 5 rounds, median over frames: toy orbit `Faceted` 11.36 → 2.46 (4.6×), `XRay` 11.46 → 2.52 (4.5×), half peeled 43.55 → 4.59 (9.5×); `curl_orbit` 8.32 → 2.01 (4.1×), `gym_orbit` 8.89 → 2.24 (4.0×), `anatomy_peel` paint 2.32 → 0.86 (2.7×; its peeling phase 44.40 → 4.57, 9.7×). The second pass agreed within 0.05 ms. The anatomy's scene build (the peel applied, 4.4 ms a peeling frame) is not the order and did not move. Fingerprints equal in every run. A `wasm32-unknown-unknown` probe (`--no-default-features`, LTO) grows 144 669 → 146 093 bytes | `cargo bench -p motoreel`; both trees' bench binaries run by one script |
| 2026-11-23 | **Coalescing measured, not assumed** | The spike saw identical pixels in Flutter. In the PPM sink a merged run is composited once, so antialiased pixels change: 0.5–0.7 % of the toy's frame, up to 88 levels on a silhouette pixel, and a handful of seam pixels by 1–2 levels. Hence opt-in |

## Prior art

- Newell, Newell & Sancha 1972 (the separating-plane order R-0018 uses).
- The separating-axis theorem for convex polyhedra (Gottschalk, Lin &
  Manocha, "OBBTree", SIGGRAPH '96): only face planes are tested here,
  as R-0018 defined; the early exit is theirs.
- klein (Jeremy Ong), specialised PGA kernels for point sandwiches; here
  without the closed form, so the rounding stays garust's.
- The fitAI spike, `~/code/lab/2026-11-fitai-puente-3d` (profile,
  coalescing).

## Test plan

| AC | Tests |
|----|-------|
| AC1 | `solid_reference::differential::random_scenes_order_the_same` (proptest, 400 seeds × 4 cameras), `solid_reference::differential::the_anatomy_orders_the_same_all_round`; `sandwich::tests::*` (the sandwich against `transform`, bit for bit) |
| AC2 | The existing suites and goldens; E2E: the three reels' PPM frames' md5 before and after, and the bench fingerprints |
| AC3 | `r0022_depth_order::ac3::two_renders_and_four_threads_are_byte_identical` |
| AC4 | `cargo bench -p motoreel`, A/B against the base commit (decision log) |
| AC5 | The bench itself |
| AC6 | `r0022_depth_order::ac6::*`; `coalesce::tests::*` |
| AC7 | R-0017 `ac9::no_new_dependency`, R-0003 AC5, R-0018 `ac9::the_3d_code_is_trig_free`; CI's `--no-default-features` step |

## Changelog

- 2026-11-23: created, accepted and implemented on the owner's brief.
