# Blender model pipeline

Blender **models**; motoreel **renders** (R-0020). The OBJ files in
`assets/props/` are the committed inputs; this script is how they were
made, so a change to a prop is a change to `props.py` and a re-export, never
a hand edit of the OBJ.

```bash
/Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup \
    --python tools/blender/props.py -- assets/props
```

Run it twice into two folders and `cmp` the files: the output is
byte-identical for a given Blender version (fixed parameters, `bmesh` only,
no randomness, factory settings). CI does not run Blender; the tests in
`crates/motoreel/tests/r0020_obj.rs` load the committed files.

## Conventions the loader relies on

| What | Rule |
|------|------|
| Frame | Blender's own: +z up, −y the front, metres. Exported with `forward_axis='Y'`, `up_axis='Z'`; motoreel reads it with `Axes::ZUp` |
| Parts | One Blender object per convex part, named `NNb_name`; parts touch but never cross, so R-0018 orders them as wholes |
| Outline | `NNa_name_ink` before each outlined part: the part grown by 0.01 m with its faces reversed |
| Materials | The name is the binding key. A first word `skin` or `inner` sets the x-ray layer (`skin_shroud`, `inner_stack`, `skin_ink`) |
| Faces | Quads and convex n-gons; motoreel keeps them whole and ear-clips anything else |
| Budget | At most 1 500 triangles per prop, hulls included |

## The props

| File | Parts | Faces | Triangles (with hulls) |
|------|-------|-------|------------------------|
| `power_rack.obj` | 17 + 17 hulls | 212 | 440 |
| `cable_machine.obj` | 18 + 9 hulls | 186 | 420 |
| `flat_bench.obj` | 6 + 6 hulls | 72 | 144 |
| `dumbbell.obj` | 3 + 3 hulls | 52 | 136 |
| `barbell.obj` | 11 + 11 hulls | 252 | 744 |

## Third-party models

Only CC0, with an entry in `assets/LICENSES.md` (source URL, author, date).
Import, rename the materials to the house names, split into convex parts,
and export with the same call as `Prop.export` in `props.py`.
