//! Loading low-poly meshes from Wavefront OBJ text (R-0020, SPEC-0020).
//!
//! Props are modelled outside motoreel (Blender, `tools/blender/`) and
//! rendered here, so the bytes of a frame never depend on the modelling
//! tool. This module reads the geometry subset of OBJ (Wavefront
//! Technologies, *Advanced Visualizer* manual, appendix B1, "Object Files"):
//! `v`, `f`, `o` and `usemtl`. Attributes motoreel does not draw (`vn`,
//! `vt`, `vp`, `s`, `g`, `mtllib`) are skipped; anything else is refused
//! with its line number rather than silently ignored.
//!
//! **Faces.** A face that is planar and strictly convex (the
//! [`crate::Mesh`] rule) is kept whole, so a quad stays one quad. Any other
//! face loses its repeated and collinear corners and is split by ear
//! clipping (D. Eberly, "Triangulation by Ear Clipping", Geometric Tools,
//! 2002), always taking the first ear in the face's own vertex order, so
//! the same text gives the same triangles.
//!
//! **Axes and units.** Positions are metres. [`Axes::ZUp`] is Blender's
//! native frame (+z up, −y the front) and maps `(x, y, z)` to motoreel's
//! y-up frame as `(x, z, −y)`, a proper rotation, so windings and normals
//! survive. [`Axes::YUp`] is the identity, for files already y-up.
//!
//! **Materials.** A face is made of the material named by the last
//! `usemtl` before it (the empty name before any). The caller binds each
//! name to a [`Material`]; a name whose first word is `inner` or `skin`
//! (split at `_` or `.`) sets the [`Layer`], so a model can mark its
//! inside for the x-ray finish.
//!
//! Std-only and pure: numbers parse with `str::parse::<f64>`, which rounds
//! correctly, and nothing depends on hashing or allocation order.

use garust::pga;

use crate::finish::{Layer, Material};
use crate::mesh::{check_face, cross, sub, Mesh, MeshError, MeshFace, V3};

/// The up axis of an OBJ file (SPEC-0020 §2.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axes {
    /// Blender's frame: +z up, −y toward the viewer. `(x, y, z)` becomes
    /// `(x, z, 0 − y)`.
    ZUp,
    /// Already motoreel's frame: +y up. The identity.
    YUp,
}

impl Axes {
    /// `p` in motoreel's frame. `0 − y` rather than `−y`, so a zero stays
    /// a positive zero.
    fn map(self, [x, y, z]: V3) -> V3 {
        match self {
            Axes::ZUp => [x, z, 0.0 - y],
            Axes::YUp => [x, y, z],
        }
    }
}

/// Why an OBJ file could not be loaded: the 1-based line and the cause.
#[derive(Clone, Debug, PartialEq)]
pub struct ObjError {
    /// The 1-based line the problem is on.
    pub line: usize,
    /// What is wrong there.
    pub kind: ObjErrorKind,
}

/// The cause of an [`ObjError`].
#[derive(Clone, Debug, PartialEq)]
pub enum ObjErrorKind {
    /// A statement this loader does not read (`l`, `curv`, …); carries
    /// the keyword.
    Unsupported(String),
    /// A `v` with other than 3 coordinates (or 6, with a colour); carries
    /// the count.
    VertexArity(usize),
    /// A token that is not a number or an index; carries it.
    Number(String),
    /// A coordinate that parsed as infinite or NaN.
    NonFinite,
    /// A face index of 0, or one naming a vertex not yet defined; carries
    /// the index as written.
    Index(i64),
    /// A face with fewer than three vertices; carries the count.
    TooFewIndices(usize),
    /// A face with no area once repeated and collinear corners are gone.
    Degenerate,
    /// A face the ear clipper cannot split: it crosses itself.
    NotSimple,
    /// An `o` or `usemtl` with no name.
    MissingName,
    /// A material name the caller did not bind; carries it. The line is
    /// its first face.
    Unbound(String),
    /// The mesh the faces make was refused (a guard: parsing already
    /// checks every face). The line is the face's.
    Mesh(MeshError),
}

impl std::fmt::Display for ObjError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: ", self.line)?;
        match &self.kind {
            ObjErrorKind::Unsupported(k) => write!(f, "unsupported statement `{k}`"),
            ObjErrorKind::VertexArity(n) => write!(f, "a vertex with {n} coordinates"),
            ObjErrorKind::Number(t) => write!(f, "`{t}` is not a number"),
            ObjErrorKind::NonFinite => write!(f, "a non-finite coordinate"),
            ObjErrorKind::Index(i) => write!(f, "vertex index {i} does not exist"),
            ObjErrorKind::TooFewIndices(n) => write!(f, "a face with {n} vertices"),
            ObjErrorKind::Degenerate => write!(f, "a face with no area"),
            ObjErrorKind::NotSimple => write!(f, "a face that crosses itself"),
            ObjErrorKind::MissingName => write!(f, "a statement with no name"),
            ObjErrorKind::Unbound(m) => write!(f, "material `{m}` is not bound"),
            ObjErrorKind::Mesh(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ObjError {}

/// One face after triangulation: vertex indices into the file's vertices,
/// a material index into its names, and the line it came from.
#[derive(Clone, Debug, PartialEq)]
struct Face {
    indices: Vec<usize>,
    material: usize,
    line: usize,
}

/// One `o` object: its name and faces.
#[derive(Clone, Debug, PartialEq)]
struct Part {
    name: String,
    faces: Vec<Face>,
}

/// A parsed OBJ file: positions in motoreel's frame, objects in file
/// order, material names in order of first `usemtl`.
#[derive(Clone, Debug, PartialEq)]
pub struct Obj {
    vertices: Vec<V3>,
    parts: Vec<Part>,
    materials: Vec<String>,
}

fn err(line: usize, kind: ObjErrorKind) -> ObjError {
    ObjError { line, kind }
}

/// The layer a material name asks for, if any: its first word (split at
/// `_` or `.`) is `inner` or `skin`.
#[must_use]
pub fn layer_of(name: &str) -> Option<Layer> {
    match name.split(['_', '.']).next() {
        Some("inner") => Some(Layer::Inner),
        Some("skin") => Some(Layer::Skin),
        _ => None,
    }
}

impl Obj {
    /// Parses `text`, carrying positions into motoreel's frame by `axes`.
    ///
    /// # Errors
    /// The first [`ObjError`] in file order.
    pub fn parse(text: &str, axes: Axes) -> Result<Obj, ObjError> {
        let mut obj = Obj {
            vertices: Vec::new(),
            parts: vec![Part {
                name: String::new(),
                faces: Vec::new(),
            }],
            materials: vec![String::new()],
        };
        let mut material = 0;
        for (k, raw) in text.lines().enumerate() {
            let line = k + 1;
            let content = raw.split('#').next().unwrap_or_default();
            let mut words = content.split_whitespace();
            let Some(keyword) = words.next() else {
                continue;
            };
            let rest: Vec<&str> = words.collect();
            match keyword {
                "v" => obj.vertices.push(axes.map(vertex(line, &rest)?)),
                "f" => {
                    let poly = obj.indices(line, &rest)?;
                    for indices in split(&poly, &obj.vertices).map_err(|e| err(line, e))? {
                        obj.current().faces.push(Face {
                            indices,
                            material,
                            line,
                        });
                    }
                }
                "o" => obj.parts.push(Part {
                    name: name(line, &rest)?,
                    faces: Vec::new(),
                }),
                "usemtl" => material = obj.material(name(line, &rest)?),
                "vn" | "vt" | "vp" | "s" | "g" | "mtllib" => {}
                other => return Err(err(line, ObjErrorKind::Unsupported(other.to_owned()))),
            }
        }
        obj.parts.retain(|p| !p.faces.is_empty());
        Ok(obj)
    }

    /// The names of the objects that have faces, in file order (the
    /// unnamed one before any `o` is `""`).
    pub fn object_names(&self) -> impl Iterator<Item = &str> {
        self.parts.iter().map(|p| p.name.as_str())
    }

    /// One mesh per object that has faces, in file order, each named.
    /// A mesh holds only the vertices and materials its faces use, in file
    /// and `usemtl` order.
    ///
    /// # Errors
    /// [`ObjErrorKind::Unbound`] for a used name `bind` returns `None` for.
    pub fn meshes(
        &self,
        bind: impl Fn(&str) -> Option<Material>,
    ) -> Result<Vec<(String, Mesh)>, ObjError> {
        let bound = self.bind(&bind)?;
        self.parts
            .iter()
            .map(|p| Ok((p.name.clone(), self.build(p.faces.iter(), &bound)?)))
            .collect()
    }

    /// Every face of the file as one mesh.
    ///
    /// # Errors
    /// As [`Obj::meshes`].
    pub fn mesh(&self, bind: impl Fn(&str) -> Option<Material>) -> Result<Mesh, ObjError> {
        let bound = self.bind(&bind)?;
        self.build(self.parts.iter().flat_map(|p| &p.faces), &bound)
    }

    fn current(&mut self) -> &mut Part {
        let last = self.parts.len() - 1; // never empty: one part from the start
        &mut self.parts[last]
    }

    fn material(&mut self, name: String) -> usize {
        match self.materials.iter().position(|m| *m == name) {
            Some(i) => i,
            None => {
                self.materials.push(name);
                self.materials.len() - 1
            }
        }
    }

    /// The face's vertex indices, 0-based; relative (negative) indices
    /// count back from the last vertex defined.
    fn indices(&self, line: usize, words: &[&str]) -> Result<Vec<usize>, ObjError> {
        if words.len() < 3 {
            return Err(err(line, ObjErrorKind::TooFewIndices(words.len())));
        }
        let n = self.vertices.len();
        words
            .iter()
            .map(|w| {
                let first = w.split('/').next().unwrap_or_default();
                let i: i64 = first
                    .parse()
                    .map_err(|_| err(line, ObjErrorKind::Number((*w).to_owned())))?;
                let at = match i {
                    1.. => usize::try_from(i - 1).ok(),
                    ..=-1 => usize::try_from(i.unsigned_abs())
                        .ok()
                        .and_then(|b| n.checked_sub(b)),
                    0 => None,
                };
                at.filter(|&a| a < n)
                    .ok_or_else(|| err(line, ObjErrorKind::Index(i)))
            })
            .collect()
    }

    /// Each used material bound and layered, in name order; unused names
    /// (the empty default, when every face has a `usemtl`) stay `None`.
    fn bind(
        &self,
        bind: &impl Fn(&str) -> Option<Material>,
    ) -> Result<Vec<Option<Material>>, ObjError> {
        let faces = || self.parts.iter().flat_map(|p| &p.faces);
        self.materials
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let Some(first) = faces().find(|f| f.material == i) else {
                    return Ok(None);
                };
                let material = bind(name)
                    .ok_or_else(|| err(first.line, ObjErrorKind::Unbound(name.clone())))?;
                Ok(Some(match layer_of(name) {
                    Some(layer) => material.with_layer(layer),
                    None => material,
                }))
            })
            .collect()
    }

    /// A mesh of `faces`: the vertices they use in file order, the
    /// materials they use in `usemtl` order, so a file written from a mesh
    /// reads back as that mesh.
    fn build<'a>(
        &self,
        faces: impl Iterator<Item = &'a Face> + Clone,
        bound: &[Option<Material>],
    ) -> Result<Mesh, ObjError> {
        let mut vertex_map = vec![None; self.vertices.len()];
        let mut material_map = vec![None; bound.len()];
        for face in faces.clone() {
            face.indices.iter().for_each(|&v| vertex_map[v] = Some(0));
            material_map[face.material] = Some(0);
        }
        let mut vertices = Vec::new();
        for (v, slot) in vertex_map.iter_mut().enumerate() {
            if slot.is_some() {
                let [x, y, z] = self.vertices[v];
                *slot = Some(vertices.len());
                vertices.push(pga::Point::new(x, y, z));
            }
        }
        let mut materials = Vec::new();
        for (m, slot) in material_map.iter_mut().enumerate() {
            if slot.is_some() {
                *slot = Some(materials.len());
                // `bind` returned `Some` for every material a face uses.
                materials.extend(bound[m]);
            }
        }
        let (mut out, mut lines) = (Vec::new(), Vec::new());
        for face in faces {
            let indices = face.indices.iter().filter_map(|&v| vertex_map[v]).collect();
            let material = material_map[face.material].unwrap_or(usize::MAX);
            out.push(MeshFace::new(indices, material));
            lines.push(face.line);
        }
        Mesh::new(vertices, out, materials).map_err(|e| {
            let face = match e {
                MeshError::Vertex(_) => 0,
                MeshError::TooFewIndices(i)
                | MeshError::IndexOutOfRange(i)
                | MeshError::MaterialOutOfRange(i)
                | MeshError::NonPlanar(i)
                | MeshError::NonConvex(i) => i,
            };
            err(lines.get(face).copied().unwrap_or(0), ObjErrorKind::Mesh(e))
        })
    }
}

/// The three coordinates of a `v`; a trailing colour (three more) is
/// accepted and ignored.
fn vertex(line: usize, words: &[&str]) -> Result<V3, ObjError> {
    if words.len() != 3 && words.len() != 6 {
        return Err(err(line, ObjErrorKind::VertexArity(words.len())));
    }
    let mut p = [0.0; 3];
    for (c, w) in p.iter_mut().zip(words) {
        *c = w
            .parse::<f64>()
            .map_err(|_| err(line, ObjErrorKind::Number((*w).to_owned())))?;
        if !c.is_finite() {
            return Err(err(line, ObjErrorKind::NonFinite));
        }
    }
    Ok(p)
}

/// The rest of an `o` or `usemtl` line, which must not be empty.
fn name(line: usize, words: &[&str]) -> Result<String, ObjError> {
    if words.is_empty() {
        return Err(err(line, ObjErrorKind::MissingName));
    }
    Ok(words.join(" "))
}

/// Does `poly` pass the [`Mesh`] face rule as it is?
fn whole(poly: &[usize], points: &[V3]) -> bool {
    check_face(0, &MeshFace::new(poly.to_vec(), 0), points, 1).is_ok()
}

/// `poly` as faces the [`Mesh`] rule accepts: itself if it already does,
/// else its ear-clipped triangles (SPEC-0020 §2.2).
fn split(poly: &[usize], points: &[V3]) -> Result<Vec<Vec<usize>>, ObjErrorKind> {
    if whole(poly, points) {
        return Ok(vec![poly.to_vec()]);
    }
    let ring = simplify(poly, points);
    if ring.len() < 3 {
        return Err(ObjErrorKind::Degenerate);
    }
    if whole(&ring, points) {
        return Ok(vec![ring]);
    }
    let tris = ear_clip(ring, points)?;
    if tris.iter().all(|t| whole(t, points)) {
        Ok(tris)
    } else {
        Err(ObjErrorKind::Degenerate)
    }
}

/// `poly` without repeated corners (equal positions) and without corners
/// on a straight line between their neighbours, until none is left.
fn simplify(poly: &[usize], points: &[V3]) -> Vec<usize> {
    let mut ring = poly.to_vec();
    loop {
        let m = ring.len();
        if m < 3 {
            return ring;
        }
        let drop = (0..m).find(|&k| {
            let (a, b, c) = (
                points[ring[(k + m - 1) % m]],
                points[ring[k]],
                points[ring[(k + 1) % m]],
            );
            a == b || cross(sub(b, a), sub(c, b)) == [0.0; 3]
        });
        match drop {
            Some(k) => {
                ring.remove(k);
            }
            None => return ring,
        }
    }
}

/// Ear clipping in the coordinate plane the face's Newell normal is most
/// aligned with, always the first ear in ring order. Each triangle keeps
/// the face's winding.
fn ear_clip(mut ring: Vec<usize>, points: &[V3]) -> Result<Vec<Vec<usize>>, ObjErrorKind> {
    let normal = newell(&ring, points);
    let drop_axis = (0..3).fold(0, |best, a| {
        if normal[a].abs() > normal[best].abs() {
            a
        } else {
            best
        }
    });
    if normal[drop_axis] == 0.0 {
        return Err(ObjErrorKind::Degenerate);
    }
    // The 2D projection, turned so the face winds counter-clockwise.
    let (u, v) = match drop_axis {
        0 => (1, 2),
        1 => (2, 0),
        _ => (0, 1),
    };
    let flip = normal[drop_axis] < 0.0;
    let at = |i: usize| {
        let p = points[i];
        if flip {
            [p[v], p[u]]
        } else {
            [p[u], p[v]]
        }
    };
    let turn = |a: [f64; 2], b: [f64; 2], c: [f64; 2]| {
        (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
    };
    let mut out = Vec::new();
    while ring.len() > 3 {
        let m = ring.len();
        let ear = (0..m).find(|&k| {
            let (ia, ib, ic) = (ring[(k + m - 1) % m], ring[k], ring[(k + 1) % m]);
            let (a, b, c) = (at(ia), at(ib), at(ic));
            turn(a, b, c) > 0.0
                && ring.iter().all(|&j| {
                    let p = at(j);
                    p == a
                        || p == b
                        || p == c
                        || turn(a, b, p) < 0.0
                        || turn(b, c, p) < 0.0
                        || turn(c, a, p) < 0.0
                })
        });
        let k = ear.ok_or(ObjErrorKind::NotSimple)?;
        out.push(vec![ring[(k + m - 1) % m], ring[k], ring[(k + 1) % m]]);
        ring.remove(k);
    }
    out.push(ring);
    Ok(out)
}

/// The Newell normal of a polygon (M. E. Newell; see F. S. Hill,
/// *Computer Graphics*, 1990): robust for non-planar rings, summed in ring
/// order.
fn newell(ring: &[usize], points: &[V3]) -> V3 {
    let m = ring.len();
    let mut n = [0.0; 3];
    for k in 0..m {
        let (p, q) = (points[ring[k]], points[ring[(k + 1) % m]]);
        n[0] += (p[1] - q[1]) * (p[2] + q[2]);
        n[1] += (p[2] - q[2]) * (p[0] + q[0]);
        n[2] += (p[0] - q[0]) * (p[1] + q[1]);
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::dot;

    #[test]
    fn newell_of_a_unit_square_is_twice_its_area_up() {
        let pts = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        assert_eq!(newell(&[0, 1, 2, 3], &pts), [0.0, 0.0, 2.0]);
        assert!(dot(newell(&[3, 2, 1, 0], &pts), [0.0, 0.0, 1.0]) < 0.0);
    }

    #[test]
    fn simplify_drops_repeats_and_straight_corners() {
        let pts = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 1.0, 0.0],
            [2.0, 1.0, 0.0],
        ];
        assert_eq!(simplify(&[0, 1, 2, 3, 4], &pts), vec![0, 2, 4]);
    }
}
