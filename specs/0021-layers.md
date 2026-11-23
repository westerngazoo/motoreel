# SPEC-0021 — Layers and the peel

- **Status:** Accepted (2026-11-22, with R-0021). Implemented; architect
  and owner review pending
- **Realizes:** R-0021
- **Author:** Claude (main session) with owner
- **Created:** 2026-11-22
- **Depends on:** SPEC-0018 (`solid.rs`, the object order), SPEC-0019
  (`XRay`, `Sweep`), SPEC-0020 (`Obj`)
- **Module(s):** `crates/motoreel/src/solid.rs` (enclosure, cycle rule);
  new `src/peel.rs` (`Peel`, `PeelError`); `src/finish.rs`
  (`Material::mix`); new `tests/r0021_layers.rs`, golden
  `tests/golden/r0021_layers.ppm`; new `tools/blender/anatomy.py`,
  `tools/blender/house.py` (shared with `props.py`, whose output is
  unchanged), `assets/anatomy/`; new example `anatomy_peel`; the orbit
  examples' shared loop split into `studio.rs` (looks, camera, light, the
  parallel loop) and `orbit.rs` (the curl timeline's frame)

## 1. Motivation

Skin, muscle and bone on one rig, taken away in order (R-0021 §2).

## 2. Design

### 2.1 Nested order (`solid.rs`)

`draws_first(a, b)` now returns `(a_first, enclosed)`:

1. A separating face plane, as before → `(order, false)`.
2. `inside(b, a)`: every view vertex of `b` satisfies
   `n · (v − p0) / |n| ≤ SEPARATION_TOL` for every face plane `(n, p0)` of
   `a` → `b` first, `enclosed = true`; symmetrically `inside(a, b)`.
3. Else the mean-depth fallback → `(order, false)`.

An inverted hull's normals point inward, so no part is ever `inside` one;
hulls keep the fallback, as R-0018 tuned them.

`order` counts, per node, its pending constraints and those of them that
are enclosures. Free nodes (nothing pending) go farthest first, as before.
In a cycle, the farthest node with **no pending enclosure** is drawn and
its constraints dropped; only if every remaining node waits on an
enclosure is the farthest of all taken. A shell therefore never goes
before a part it encloses.

### 2.2 `Peel` (`peel.rs`)

`Peel::new(lift, axis, span, stagger)`: `lift` finite; `axis` finite,
non-zero (stored unit); `span` finite, `span.0 ≠ span.1`; `stagger` in
`[0, 1)`. `progress(key, u)`:

```
along = clamp((key − span.0) / (span.1 − span.0), 0, 1)
s     = clamp((u − stagger·along) / (1 − stagger), 0, 1)
e     = s²(3 − 2s)
```

`apply(mesh, u)`: for each face in order, `n = (v1 − v0) × (v2 − v0)`,
`key = centroid · axis`, `d = lift · e / |n|`; the face gets fresh vertices
`v + d·n` and its own material at `opacity · (1 − e)`. Face order and ids
are unchanged, so at `u = 0` the surfaces equal the unpeeled mesh's.

### 2.3 `Material::mix`

`t` clamped (NaN → 0); per slot `i < len`, tone `round((1 − t)a + t·b)` per
channel with `b` the other ramp's tone `min(i, its len − 1)`; unused slots
repeat the last; opacity `(1 − t)·a + t·b` clamped to `[0, 1]`; seam and
layer kept.

### 2.4 Assets and rig

`anatomy.py` ports the curl example's kit (lathe capsules, spheres,
prisms, bands, boxes) to Python, builds in motoreel's frame and stores
`(x, −z, y)` in Blender. Objects are `segment.part`; `joints.tsv` lists
`segment, parent, x, y, z` (the joint in motoreel's frame, metres). Muscle
and bone capsules keep at most one cone ring. Each inner part lies inside
a skin part of its own segment (tested).

### 2.5 The example

Per frame the scene is rebuilt: platform, floor marks, then one depth
group of skin (peeled; ink hulls fade with a zero-lift peel), muscle
(`Layer::Skin` by binding, the biceps mixed toward the load colour),
bone (the forearm bones mixed likewise) and the two dumbbells (R-0020's
prop), then the lever paths. Forearms, hands and dumbbells ride one curl
track per side; the rest holds. Looks: `Faceted`, then a `Sweep` to
`XRay`, then `XRay`.

## 3. Test plan

`tests/r0021_layers.rs`, one module per AC (R-0021's table).

## 4. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-11-22 | `Peel::apply` returns `Result` with guard variants rather than unwrapping | No panics on production paths; the guards name what cannot happen |
| 2026-11-22 | The shared example loop split into `studio.rs` and `orbit.rs` | The anatomy example has its own timeline; reading `orbit.rs` would compile an unused frame function |
