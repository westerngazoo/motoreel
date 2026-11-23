# Requirements

A **requirement** states *what* the project must do — a capability or property,
from the problem perspective, independent of implementation. Requirements are
the WHAT; [`specs/`](../specs/) are the HOW.

Every requirement is decided **together** (owner + Claude) before a spec is
written, and every requirement is owned by a `qa` agent run that verifies it.

## Process

1. **Discuss.** Owner and Claude agree the capability and its acceptance
   criteria. See [`CLAUDE.md`](../CLAUDE.md) §4.
2. **Record.** Create a file from [`TEMPLATE.md`](TEMPLATE.md), numbered
   `R-NNNN` (next free 4-digit id): `NNNN-short-name.md`.
3. **Accept.** When acceptance criteria are unambiguous, status → `Accepted`.
   Only then may a spec realize it.
4. **Realize.** One or more `SPEC-NNNN` in `specs/` implement the requirement.
5. **Verify.** The `qa` agent, scoped to this `R-NNNN`, confirms every
   acceptance criterion. Status → `Met`.

## Status values

`Draft` → `Accepted` → `Met` · (or `Superseded`)

## Relationship to specs

A requirement links forward to the spec(s) that realize it; a spec links back to
the requirement(s) it satisfies. The mapping is maintained in
[`ROADMAP.md`](../ROADMAP.md).

## Index

| Req | Title | Milestone | Status |
|-----|-------|-----------|--------|
| [R-0001](0001-track-ease.md) | Motor keyframe tracks (`Track` + `Ease` + evaluation) | M1 | Met |
| [R-0002](0002-scene-camera.md) | Scene, objects, motor-posed camera, projection | M1 | Met |
| [R-0003](0003-svg-sink.md) | `SvgSink` and the first rendered animation | M1 | Met |
| [R-0004](0004-physics-playback.md) | Physics playback: record a simulated rollout into motor tracks | M2 | Met |
| [R-0012](0012-paths-and-fills.md) | Paths and fills: cubic Bézier paths, nonzero fills, shape generators | MC | Accepted (In review) |
| [R-0016](0016-gradient-fills.md) | Gradient fills: linear and radial, 2–8 stops, dithered raster, native SVG | MC | Accepted (In review) |
| [R-0017](0017-interchangeable-finish.md) | Interchangeable finish («acabado»): surfaces + a finish trait; `Facetas` byte-identical to today's low poly | MC | Accepted (In progress: implemented, PR open) |
| [R-0018](0018-faceted-3d.md) | Faceted 3D: motor-posed low-poly meshes, back-face cull, painter's sort, painted by an R-0017 finish | MC | Accepted (In progress: implemented, PR open) |
| [R-0019](0019-x-ray.md) | The x-ray finish and its sweep: tinted glass skin, inner parts in a non-stacking glow, a screen-line sweep between two finishes | MC | Accepted (In progress: implemented, PR open) |
| [R-0020](0020-load-models.md) | Loading models: low-poly meshes from OBJ, one mesh per object, materials and x-ray layers by name, Blender's frame mapped | MC | Accepted (In progress: implemented, PR open) |
