# SPEC-0022 — Depth-order performance

- **Status:** Accepted (2026-11-23, with R-0022). Implemented; architect
  and owner review pending
- **Realizes:** R-0022
- **Author:** Claude (main session) with owner
- **Created:** 2026-11-23
- **Depends on:** SPEC-0018 (`solid.rs`, the object order), SPEC-0021 (the
  enclosure rule)
- **Module(s):**
  - `src/solid.rs`: `Plane`, `Sides`, `OwnSides`; `node`, `order`,
    `draws_first`, `separated`, `inside` rewritten to read lazily
  - new `src/sandwich.rs`: `Sandwich`, the exact point sandwich
  - new `src/solid_reference.rs` (`#[cfg(test)]`): the R-0021 order,
    verbatim, and the differential test
  - new `src/coalesce.rs`: `coalesce`, exported
  - `src/shapes.rs` (`polygon`) and `src/finish.rs` (`points`): the same
    results with fewer allocations
  - new `benches/{common/mod.rs, orbit.rs, anatomy.rs}`; the 3D examples'
    scenes moved into `scene.rs` so the benches read them in place
  - new `tests/r0022_depth_order.rs`

## 1. Motivation

R-0022: make the painter's order fast enough for an interactive orbit,
without changing its output.

## 2. Design

### 2.1 Where the time went

Profiled on the bench's toy orbit (`sample` on macOS, a build with
`-C llvm-args=-inline-threshold=0` so the order's functions keep their
frames), before the change: about 80 % of a frame in `solid::surfaces`'s
pair loop. For each pair of objects whose image boxes overlap,
`draws_first` walked the face planes of both, and `separated` computed the
side of **every** vertex of **both** objects for each plane before deciding
(a min/max fold), so a pair cost `(F_a + F_b) · (V_a + V_b)` side
evaluations however early the answer was clear. The toy has 1 132 such
pairs a frame.

### 2.2 Lazy sides (`solid.rs`)

`separated(n, p0, a, b)` decides from four facts: whether all of `a` keeps
to the negative side (`a_hi ≤ tol`), to the positive side (`a_lo ≥ −tol`),
and the same two for `b`. `a` behind is `A.neg ∧ B.pos`, in front is
`A.pos ∧ B.neg`, and the eye's side of the plane turns that into the
order. The rewrite keeps the facts and changes how they are found:

1. **Per plane, once a frame** (`Plane`): `|n|`, whether it is usable, and
   the eye's side; the expressions R-0018 used, in the same order.
2. **The owner's sides, once a frame** (`OwnSides`): a plane's own object's
   `neg` and `pos`, computed the first time a pair asks and kept for every
   other pair. Most planes of a convex part keep `neg` only.
3. **The other object, only as far as needed** (`Sides::of`): only the side
   that can complete a separation is read (`pos` if the owner keeps `neg`,
   `neg` if it keeps `pos`), and the scan stops at the first vertex that
   rules it out, which for a plane facing away from the other part is
   usually the first.
4. **Invalid or eye-on-plane planes** are skipped before any vertex is
   read; they never separated.

Each fact is an "all vertices" predicate, so the order the vertices are
read in, and stopping once it is false, cannot change it. NaN keeps the
old meaning: the range fold ignored NaN, and `s > tol` and `s < −tol` are
false for NaN. `inside` keeps its `s ≤ tol` (where NaN fails) over the
precomputed `|n|`. The first separating plane, planes of `a` then planes
of `b`, still decides; `inside` and the mean-depth fallback, the Kahn
order and its cycle rule are untouched. The surfaces then move out of the
nodes instead of being cloned.

Counted on the toy (one frame): about 330 000 side evaluations in the
pair tests and 230 000 in `inside`, against 6.3 to 7.4 million for the
same anatomy before.

### 2.3 The exact sandwich (`sandwich.rs`)

With the order fixed, posing the vertices was about a quarter of the
toy's frame. `pga::Point::transform` is `M · p · M̃`: two calls to
garust-core's `sparse_product`, which walks every one of the 16 × 16 blade
pairs and skips zero coefficients. A motor can only be non-zero on its 8
even blades, a point on its 4 trivectors, and `M · p` on its 8 odd blades,
so `Sandwich::apply` walks only those, **in the same ascending order, with
the same zero tests and garust's own `CAYLEY` table**: each output
coefficient receives the same terms in the same order and rounds the same
way. A point or a motor with anything off those blades takes `transform`.
`to_euclidean` is garust's, verbatim. 69 ns against 167 ns a vertex.

This leans on `sparse_product`'s summation order (ascending `i`, then
`j`), which garust documents as bit-identical to its dense `*`. The
proptest `sandwich::tests::a_posed_point_is_bit_identical_to_transform`
pins it, so a change upstream fails here first. The kernel belongs in
garust as an exact `Motor::apply_point`; when it lands there, this file
goes.

### 2.4 Coalescing (`coalesce.rs`)

`coalesce(prims: Vec<Prim2>) -> Vec<Prim2>` folds each `Prim2::Path` into
the previous output primitive when that is a `Path` with an equal `Style`
and the style is opaque: a `Paint::Flat` fill at alpha 1, and a stroke at
alpha 1 or not drawn (width or alpha 0). Only neighbours merge, so the
painter's order holds and every subpath keeps its style. Nothing in
`Scene::eval*` calls it.

Pixels: a sink fills a path's subpaths as one nonzero union and strokes
them once (union by max), while separate paths composite one over the
next. Inside a run of one opaque colour that is the same colour; it is not
the same coverage on antialiased pixels two pieces both touch. Measured
on the toy at 540 × 960 under `Faceted`: 0.45–0.68 % of the pixels change,
up to 88 levels on silhouette pixels, and 2–20 pixels a frame off any edge
(seams inside a run), by at most 2 levels. The paths drop 2.1–2.5× (the
three layers; the spike's 3.8× was the skin alone).

### 2.5 The oracle and the differential test

`solid_reference.rs` is `solid.rs` at `08cbb90` with two edits (it shares
`Posed`; its unit test stays in `solid.rs`), compiled only under
`cfg(test)`. Its `differential` module compares `solid::surfaces` with it
as `Debug` text (every `f64` exact, `-0.0` included), then the primitives
both paint under `Faceted` and `XRay`:

- `random_scenes_order_the_same`: proptest over a `u64` seed driving a
  splitmix64 generator, 400 cases × 4 cameras (a quarter orthographic):
  up to 12 objects among cubes, octahedra, boxes, an off-centre part in a
  shell, an inverted hull of the previous object, a peeled shell, a
  non-convex pair of boxes in one mesh, two boxes sharing a face plane,
  and an exact duplicate, each in a random layer;
- `the_anatomy_orders_the_same_all_round`: the committed anatomy and two
  dumbbells, whole and half peeled, 8 azimuths, pinhole and orthographic.

A file of its own because `src/` must stay flat: R-0012's colour test reads
every entry of `src/` as a file.

### 2.6 The bench

`benches/common/mod.rs` (std only): every frame of a timeline after one
untimed warm-up round; `rounds` timed runs a frame
(`MOTOREEL_BENCH_ROUNDS`, 5), the fastest kept (load only adds time); per
phase the median, p95 and maximum; `uptime` at start and end; FNV-1a over
the primitives' `Debug` text. Arguments filter sections
(`cargo bench -p motoreel --bench anatomy -- toy`).

- `orbit.rs`: `curl_orbit` and `gym_orbit`, `orbit::frame` per frame,
  phases `facetas`, `barrido`, `rayos-x`;
- `anatomy.rs`: `anatomy_peel`, `build` (the scene for the instant: peel,
  binding) and `paint` (`eval_with` and the set) apart, phases `facetas`,
  `pelado`, `musculo`, `barrido`, `rayos-x`; and `toy-orbit`, the toy held
  at one instant while the camera turns 360° in 72 steps under `Faceted`,
  `XRay` and half peeled.

The examples' `scene.rs` files hold what each reel shows (framing, scene,
and for the anatomy its `scene`/`paint` split), so the reels and the bench
read the same code.

### 2.7 Small exact savings

- `shapes::polygon` keeps a corner when it differs from the previous point
  (for finite points, the corners `Vec::dedup` keeps) and sizes its segment
  list once, instead of copying the points first.
- `finish::points` (surface validation) iterates a fixed array per segment
  instead of allocating a `Vec` for each.
- `node` transforms each vertex once and projects with
  `project_euclidean` (what `project` calls), and reserves its face list.

### 2.8 Measured and rejected

| Technique | Outcome |
|---|---|
| Screen-space box rejection before plane tests | Already there (R-0018); a tighter test would change the pair set, so the order |
| Bounding spheres to settle planes in bulk | Not adopted: the remaining cost is about one vertex read per plane; spheres of elongated parts (a bone in its limb) settle few planes, and a rigorous rounding margin is needed to stay exact |
| Skipping pairs apart in depth | Changes the order (a separating plane may say otherwise for parts whose boxes overlap but images do not) |
| Frame coherence | Cannot be exact without re-testing every pair |
| A `Renderer` keeping scratch buffers | ~15 500 allocations a frame on the toy, ~90 % the outlines that leave as output |
| `Motor::apply_each` | No gain over `transform` per vertex |
| criterion | The dev graph is pinned to proptest by three acceptance tests |

## 3. Code outline

```rust
fn separated(plane: &Plane, own: impl FnOnce() -> Sides, other: &[V3], a_owns: bool)
    -> Option<bool>
{
    let eye = plane.eye?;
    let own = own();                       // cached per frame
    if !(own.neg || own.pos) { return None; }
    let want = Sides { neg: own.pos, pos: own.neg };
    let other = Sides::of(plane, other, want); // stops at the first counter-example
    let (a, b) = if a_owns { (own, other) } else { (other, own) };
    let a_behind = if a.neg && b.pos { true } else if a.pos && b.neg { false } else { return None };
    Some(eye == a_behind)
}
```

## 4. Non-goals

R-0022 §4.

## 5. Open questions

None.

## 6. Acceptance criteria → tests

R-0022's test plan.

## 7. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-11-23 | Accepted with R-0022 | The owner's brief |
| 2026-11-23 | The oracle is a verbatim copy, test-only | A differential test needs the old algorithm in the same build; a copy frozen at `08cbb90` is the plainest oracle, and the comment forbids optimising it |
| 2026-11-23 | Fastest of N rounds per frame | The machine was shared (load 8–47 during the work); load only adds time |

## Changelog

- 2026-11-23: created and implemented.
