# SPEC-0020 — Loading models from OBJ

- **Status:** Accepted (2026-11-22, with R-0020). Implemented; architect
  and owner review pending
- **Realizes:** R-0020
- **Author:** Claude (main session) with owner
- **Created:** 2026-11-22
- **Depends on:** SPEC-0018 (`Mesh`, `check_face`), SPEC-0017 (`Material`,
  `Layer`)
- **Module(s):** new `crates/motoreel/src/obj.rs` (`Obj`, `Axes`,
  `ObjError`, `ObjErrorKind`, `obj::layer_of`); `mesh::check_face` becomes
  `pub(crate)`; new `tests/r0020_obj.rs`; new `tools/blender/props.py`,
  `assets/props/*.obj`, `assets/LICENSES.md`; new example `gym_orbit`
  (reads `curl_orbit`'s modules, whose shared loop moves to
  `curl_orbit/studio.rs` with byte-identical output)

## 1. Motivation

Props modelled in Blender, rendered by motoreel (R-0020 §2).

## 2. Design

### 2.1 API

```rust
pub enum Axes { ZUp, YUp }
impl Obj {
    pub fn parse(text: &str, axes: Axes) -> Result<Obj, ObjError>;
    pub fn object_names(&self) -> impl Iterator<Item = &str>;
    pub fn meshes(&self, bind: impl Fn(&str) -> Option<Material>)
        -> Result<Vec<(String, Mesh)>, ObjError>;
    pub fn mesh(&self, bind: impl Fn(&str) -> Option<Material>)
        -> Result<Mesh, ObjError>;
}
pub struct ObjError { pub line: usize, pub kind: ObjErrorKind }
pub fn obj::layer_of(name: &str) -> Option<Layer>;
```

`parse` does all the geometry (positions mapped, faces validated and split);
`meshes`/`mesh` only bind names and renumber, so one parse serves several
palettes. Parts start with an unnamed one; parts with no faces are dropped
at the end of `parse`.

### 2.2 Faces

For each `f`: indices resolved (1-based, or negative from the last vertex
defined; 0 and undefined are `Index`). Then, with the positions already in
motoreel's frame:

1. If the polygon passes `mesh::check_face` (the R-0018 rule), it is one face.
2. Else `simplify`: repeatedly drop the first corner equal to its
   predecessor or collinear with its neighbours (exact `cross == 0`). Fewer
   than 3 left is `Degenerate`; a ring that now passes is one face.
3. Else ear-clip: the Newell normal picks the coordinate plane to project
   on (largest component, first on ties; a zero normal is `Degenerate`),
   flipped so the ring is counter-clockwise there. Repeatedly take the
   **first** corner (ring order) that turns left and whose triangle holds no
   other corner (on the boundary counts as inside); emit it in the ring's
   order and remove it. No ear is `NotSimple`.
4. Each triangle must pass `check_face`, or the face is `Degenerate`.

Every step is a pure function of the ring and the positions, in a fixed
order, with `+ − ×` and comparisons only: deterministic.

### 2.3 Axes

`ZUp`: `(x, y, z) → (x, z, 0 − y)` (determinant +1; `0 − y` keeps zeros
positive). `YUp`: identity. Applied as each `v` is read.

### 2.4 Materials and layers

Names are interned in first-`usemtl` order, slot 0 the empty name. `bind`
is called once per **used** name; `None` is `Unbound` at the name's first
face. `layer_of(name)` looks at the name's first word split at `_`/`.`:
`inner` → `Inner`, `skin` → `Skin`; when it answers, the layer replaces the
bound material's, otherwise the bound material is used as is.

### 2.5 Building a mesh

Mark the vertices and materials the faces use; number them in file and
name order; rewrite the faces. `Mesh::new` cannot fail on faces `parse`
accepted, but its error is still mapped to the face's line rather than
unwrapped.

### 2.6 Tooling

`tools/blender/props.py` (run headless with `--factory-startup`) builds
each prop from axis-aligned boxes and prisms with `bmesh`, each part its
own object preceded by its ink hull (grown by 0.01 m, reversed), and
exports with `forward_axis='Y', up_axis='Z'`, no normals or UVs, the
`.mtl` deleted and the `mtllib` line dropped. Two runs are byte-identical
(checked when the files were committed). CI does not run Blender.

## 3. Test plan

`tests/r0020_obj.rs`, one module per AC (see R-0020's table).

## 4. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-11-22 | Vertices renumbered in **file** order, not first-use order | First-use order made a written-out cube read back with its vertices permuted, so it was not `PartialEq` to `Mesh::cube` (AC1) |
| 2026-11-22 | `gym_orbit` reads `curl_orbit`'s modules by `#[path]`, the loop moved to `studio.rs` | No duplicated render loop; `curl_orbit`'s MP4 and frames keep their md5 (`c79eb3bc…`, frames `39fbd1c0…`) |
