//! The peel («pelar», R-0021, SPEC-0021 §2.2): a layer opens in facets.
//!
//! Each face lifts along its own outward unit normal and fades, eased by a
//! smoothstep of its own progress. Faces start one after another along an
//! axis (the stagger), keyed by their centroid's position on it, so every
//! part of a figure follows one schedule. This is the exploded view of
//! technical illustration (W. Li et al., "Automated Generation of
//! Interactive 3D Exploded View Diagrams", SIGGRAPH 2008), one face deep.
//!
//! A pure mesh-to-mesh function: no scene or object field changes. Std-only
//! and trig-free: `+ − × ÷`, `sqrt` and comparisons.

use garust::pga;

use crate::finish::MaterialError;
use crate::mesh::{dot, face_normal, Mesh, MeshError, MeshFace, V3};

/// A validated peel schedule.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Peel {
    lift: f64,
    axis: V3,
    span: (f64, f64),
    stagger: f64,
}

/// Why a [`Peel`] could not be built or applied. Each variant names its
/// field.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PeelError {
    /// `lift`: not finite; carries the value.
    Lift(f64),
    /// `axis`: zero length or not finite.
    Axis,
    /// `span`: an end not finite, or both ends equal.
    Span,
    /// `stagger`: outside `[0, 1)`; carries the value.
    Stagger(f64),
    /// A peeled face was refused (a guard: a translated valid face stays
    /// valid).
    Mesh(MeshError),
    /// A faded material was refused (a guard: the opacity stays in
    /// `[0, 1]`).
    Material(MaterialError),
}

impl std::fmt::Display for PeelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PeelError::Lift(v) => write!(f, "lift: {v} is not finite"),
            PeelError::Axis => write!(f, "axis: zero or not finite"),
            PeelError::Span => write!(f, "span: an end is not finite, or both are equal"),
            PeelError::Stagger(v) => write!(f, "stagger: {v} is not in [0, 1)"),
            PeelError::Mesh(e) => write!(f, "{e}"),
            PeelError::Material(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for PeelError {}

fn euclid(p: &pga::Point) -> V3 {
    let (x, y, z) = p.to_euclidean();
    [x, y, z]
}

/// `3s² − 2s³`: zero slope at both ends.
fn smoothstep(s: f64) -> f64 {
    s * s * (3.0 - 2.0 * s)
}

impl Peel {
    /// A peel that lifts each face `lift` (model units) along its normal.
    /// A face whose centroid sits at `span.0` along `axis` starts at
    /// `u = 0`; one at `span.1` starts at `u = stagger`; positions between
    /// start in proportion, beyond are clamped. Every face ends at `u = 1`.
    ///
    /// # Errors
    /// The [`PeelError`] naming the first bad field.
    pub fn new(
        lift: f64,
        axis: [f64; 3],
        span: (f64, f64),
        stagger: f64,
    ) -> Result<Peel, PeelError> {
        if !lift.is_finite() {
            return Err(PeelError::Lift(lift));
        }
        let len = dot(axis, axis).sqrt();
        if !(len.is_finite() && len > 0.0) {
            return Err(PeelError::Axis);
        }
        if !(span.0.is_finite() && span.1.is_finite()) || span.0 == span.1 {
            return Err(PeelError::Span);
        }
        if !(0.0..1.0).contains(&stagger) {
            return Err(PeelError::Stagger(stagger));
        }
        Ok(Peel {
            lift,
            axis: [axis[0] / len, axis[1] / len, axis[2] / len],
            span,
            stagger,
        })
    }

    /// The eased progress in `[0, 1]` at `u` of a face whose centroid sits
    /// at `key` along the axis.
    #[must_use]
    pub fn progress(&self, key: f64, u: f64) -> f64 {
        let along = ((key - self.span.0) / (self.span.1 - self.span.0)).clamp(0.0, 1.0);
        let start = self.stagger * along;
        let s = ((u - start) / (1.0 - self.stagger)).clamp(0.0, 1.0);
        smoothstep(s)
    }

    /// `mesh` peeled at `u`: every face on its own vertices, moved by
    /// `lift · e` along its outward unit normal, made of its material at
    /// `opacity · (1 − e)`, `e` its [`Peel::progress`]. Faces keep their
    /// order and indices' order, so surface ids do not change.
    ///
    /// # Errors
    /// [`PeelError::Mesh`] or [`PeelError::Material`], which valid input
    /// never produces.
    pub fn apply(&self, mesh: &Mesh, u: f64) -> Result<Mesh, PeelError> {
        let points: Vec<V3> = mesh.vertices().iter().map(euclid).collect();
        let mut vertices = Vec::new();
        let mut faces = Vec::with_capacity(mesh.faces().len());
        let mut materials = Vec::with_capacity(mesh.faces().len());
        for face in mesh.faces() {
            let p: Vec<V3> = face.indices.iter().map(|&i| points[i]).collect();
            let n = face_normal(p[0], p[1], p[2]);
            let unit = dot(n, n).sqrt();
            let centroid = p
                .iter()
                .fold([0.0; 3], |c, q| [c[0] + q[0], c[1] + q[1], c[2] + q[2]]);
            let k = p.len() as f64;
            let key = dot(
                [centroid[0] / k, centroid[1] / k, centroid[2] / k],
                self.axis,
            );
            let e = self.progress(key, u);
            let d = self.lift * e / unit;
            let first = vertices.len();
            for q in &p {
                vertices.push(pga::Point::new(
                    q[0] + d * n[0],
                    q[1] + d * n[1],
                    q[2] + d * n[2],
                ));
            }
            let material = mesh.materials()[face.material];
            materials.push(
                material
                    .with_opacity(material.opacity() * (1.0 - e))
                    .map_err(PeelError::Material)?,
            );
            faces.push(MeshFace::new(
                (first..first + p.len()).collect(),
                materials.len() - 1,
            ));
        }
        Mesh::new(vertices, faces, materials).map_err(PeelError::Mesh)
    }
}
