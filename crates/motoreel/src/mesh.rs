//! Low-poly solid meshes (R-0018 AC1, SPEC-0018 §2.1): model-space vertices,
//! planar convex faces, and the materials the faces are made of.
//!
//! A mesh is posed like every shape, by its object's motor track
//! ([`crate::Shape::Mesh`]). Building one validates it, so the depth sort
//! and the finishes never meet a bent, concave or dangling face.

use garust::pga;

use crate::finish::Material;

/// Planarity tolerance: every vertex of a face lies within this fraction
/// of the face's extent of the plane through its first three vertices.
pub const PLANAR_TOL: f64 = 1e-9;

/// One face: vertex indices counter-clockwise seen from outside, and an
/// index into the mesh's materials.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshFace {
    /// Vertex indices, at least three, counter-clockwise from outside.
    pub indices: Vec<usize>,
    /// Which of the mesh's materials the face is made of.
    pub material: usize,
}

impl MeshFace {
    /// A face over `indices` made of material `material`.
    #[must_use]
    pub fn new(indices: Vec<usize>, material: usize) -> MeshFace {
        MeshFace { indices, material }
    }
}

/// A validated low-poly mesh.
#[derive(Clone, Debug, PartialEq)]
pub struct Mesh {
    vertices: Vec<pga::Point>,
    faces: Vec<MeshFace>,
    materials: Vec<Material>,
}

/// Why a [`Mesh`] could not be built. Face errors carry the face index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshError {
    /// A vertex has a non-finite coordinate; carries the vertex index.
    Vertex(usize),
    /// A face has fewer than three indices.
    TooFewIndices(usize),
    /// A face names a vertex that does not exist.
    IndexOutOfRange(usize),
    /// A face names a material that does not exist.
    MaterialOutOfRange(usize),
    /// A face's vertices are not on one plane (within [`PLANAR_TOL`]).
    NonPlanar(usize),
    /// A face is not strictly convex (a reflex or straight corner, a
    /// repeated vertex, or a polygon that winds more than once).
    NonConvex(usize),
}

impl std::fmt::Display for MeshError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MeshError::Vertex(i) => write!(f, "vertex {i}: a non-finite coordinate"),
            MeshError::TooFewIndices(i) => write!(f, "face {i}: fewer than three indices"),
            MeshError::IndexOutOfRange(i) => write!(f, "face {i}: a vertex index out of range"),
            MeshError::MaterialOutOfRange(i) => write!(f, "face {i}: a material out of range"),
            MeshError::NonPlanar(i) => write!(f, "face {i}: not planar"),
            MeshError::NonConvex(i) => write!(f, "face {i}: not strictly convex"),
        }
    }
}

impl std::error::Error for MeshError {}

/// A Euclidean triple.
pub(crate) type V3 = [f64; 3];

pub(crate) fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub(crate) fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The cross product, in rf-marca `lambert`'s operation order.
pub(crate) fn cross(u: V3, v: V3) -> V3 {
    [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ]
}

/// The (unnormalised) normal of the plane through `a`, `b`, `c`:
/// `(b − a) × (c − a)`, outward for a counter-clockwise face.
pub(crate) fn face_normal(a: V3, b: V3, c: V3) -> V3 {
    cross(sub(b, a), sub(c, a))
}

fn euclid(p: &pga::Point) -> V3 {
    let (x, y, z) = p.to_euclidean();
    [x, y, z]
}

impl Mesh {
    /// A mesh from vertices, faces and the materials the faces index.
    ///
    /// # Errors
    /// The first [`MeshError`] found, vertices first, then faces in order.
    pub fn new(
        vertices: Vec<pga::Point>,
        faces: Vec<MeshFace>,
        materials: Vec<Material>,
    ) -> Result<Mesh, MeshError> {
        let points: Vec<V3> = vertices.iter().map(euclid).collect();
        if let Some(i) = points.iter().position(|p| !p.iter().all(|c| c.is_finite())) {
            return Err(MeshError::Vertex(i));
        }
        for (i, face) in faces.iter().enumerate() {
            check_face(i, face, &points, materials.len())?;
        }
        Ok(Mesh {
            vertices,
            faces,
            materials,
        })
    }

    /// An axis-aligned cube of half-size `half` about the origin, one
    /// material on all six square faces. Trig-free.
    ///
    /// # Errors
    /// [`MeshError::Vertex`] for a non-finite `half`; a zero or negative
    /// one collapses or turns the faces, [`MeshError::NonConvex`].
    pub fn cube(half: f64, material: Material) -> Result<Mesh, MeshError> {
        let h = half;
        let vertices = (0..8)
            .map(|i| {
                let s = |bit: usize| if i & bit == 0 { -h } else { h };
                pga::Point::new(s(1), s(2), s(4))
            })
            .collect();
        let quads = [
            [0, 2, 3, 1], // −z
            [4, 5, 7, 6], // +z
            [0, 1, 5, 4], // −y
            [2, 6, 7, 3], // +y
            [0, 4, 6, 2], // −x
            [1, 3, 7, 5], // +x
        ];
        let faces = quads.iter().map(|q| MeshFace::new(q.to_vec(), 0)).collect();
        Mesh::new(vertices, faces, vec![material])
    }

    /// A regular octahedron with vertices at distance `r` on the axes, one
    /// material on all eight triangles. Trig-free.
    ///
    /// # Errors
    /// As [`Mesh::cube`].
    pub fn octahedron(r: f64, material: Material) -> Result<Mesh, MeshError> {
        let vertices = vec![
            pga::Point::new(r, 0.0, 0.0),
            pga::Point::new(-r, 0.0, 0.0),
            pga::Point::new(0.0, r, 0.0),
            pga::Point::new(0.0, -r, 0.0),
            pga::Point::new(0.0, 0.0, r),
            pga::Point::new(0.0, 0.0, -r),
        ];
        let tris = [
            [0, 2, 4],
            [2, 1, 4],
            [1, 3, 4],
            [3, 0, 4],
            [2, 0, 5],
            [1, 2, 5],
            [3, 1, 5],
            [0, 3, 5],
        ];
        let faces = tris.iter().map(|t| MeshFace::new(t.to_vec(), 0)).collect();
        Mesh::new(vertices, faces, vec![material])
    }

    /// The model-space vertices.
    pub fn vertices(&self) -> &[pga::Point] {
        &self.vertices
    }

    /// The faces.
    pub fn faces(&self) -> &[MeshFace] {
        &self.faces
    }

    /// The materials the faces index.
    pub fn materials(&self) -> &[Material] {
        &self.materials
    }
}

/// Validate face `i` against the vertex positions and the material count.
fn check_face(i: usize, face: &MeshFace, points: &[V3], materials: usize) -> Result<(), MeshError> {
    let idx = &face.indices;
    if idx.len() < 3 {
        return Err(MeshError::TooFewIndices(i));
    }
    if idx.iter().any(|&k| k >= points.len()) {
        return Err(MeshError::IndexOutOfRange(i));
    }
    if face.material >= materials {
        return Err(MeshError::MaterialOutOfRange(i));
    }
    let p: Vec<V3> = idx.iter().map(|&k| points[k]).collect();
    let n = face_normal(p[0], p[1], p[2]);
    let n_len = dot(n, n).sqrt();
    let extent = p
        .iter()
        .flat_map(|q| sub(*q, p[0]))
        .fold(0.0_f64, |m, c| m.max(c.abs()));
    // A zero normal is a straight corner at vertex 1: not strictly convex.
    if n_len.is_nan() || n_len <= 0.0 {
        return Err(MeshError::NonConvex(i));
    }
    if p.iter()
        .any(|q| (dot(n, sub(*q, p[0])) / n_len).abs() > PLANAR_TOL * extent)
    {
        return Err(MeshError::NonPlanar(i));
    }
    // Strictly convex and wound once: every other vertex lies strictly on
    // the inner side of every edge.
    let m = p.len();
    for e in 0..m {
        let (a, b) = (p[e], p[(e + 1) % m]);
        let edge = sub(b, a);
        let inner = (0..m)
            .filter(|&k| k != e && k != (e + 1) % m)
            .all(|k| dot(cross(edge, sub(p[k], a)), n) > 0.0);
        if !inner {
            return Err(MeshError::NonConvex(i));
        }
    }
    Ok(())
}
