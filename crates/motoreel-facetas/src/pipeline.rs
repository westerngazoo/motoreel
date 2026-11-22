//! Parts → lit, culled, projected and depth-sorted faces → primitives.
//!
//! The painter's algorithm: every face is drawn whole, far to near, so a
//! nearer face covers a farther one. It is exact for convex parts that do
//! not interpenetrate, and wrong where two faces overlap in depth order
//! that no single sort can satisfy (a cycle, or a long face beside a short
//! one); for a low-poly toy at joints those cases are small.

use garust::Motor3;
use motoreel::{Camera, Prim2};

use crate::acabado::Acabado;
use crate::face::Face;
use crate::light::Light;
use crate::material::Material;
use crate::mesh::Mesh;
use crate::vec3::{cross, dot, normalize, scale, sub, transform, V3};

/// A mesh, what it is made of, and where it is.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    /// The shape, model space.
    pub mesh: Mesh,
    /// The surface.
    pub material: Material,
    /// World-from-model pose.
    pub pose: Motor3,
    /// Sort priority, world units: the part's faces sort as if this much
    /// nearer the camera. The painter's sort cannot resolve two solids
    /// that interpenetrate (the shorts around the hips, a shoe around the
    /// ankle) and draws a zigzag where they meet; a bias says which one
    /// wraps the other, as the 2D toy's draw order always did.
    pub bias: f64,
}

/// Every face of `parts` the camera can see, far to near.
///
/// Front faces are kept (back faces for an outline layer, the inverted
/// hull). A face with any vertex the projection culls is dropped whole.
/// Ties in depth fall back to [`Face::id`], so the order is total and the
/// output deterministic.
#[must_use]
pub fn faces<'a>(parts: &'a [Part], camera: &Camera, light: &Light) -> Vec<Face<'a>> {
    let view = camera.pose.inverse().to_matrix();
    let cam = camera.pose.to_matrix();
    let eye: V3 = [cam[3][0], cam[3][1], cam[3][2]];
    let mut out = Vec::new();
    for (pi, part) in parts.iter().enumerate() {
        let model = part.pose.to_matrix();
        let world: Vec<V3> = part
            .mesh
            .verts
            .iter()
            .map(|p| transform(&model, *p))
            .collect();
        let viewed: Vec<V3> = world.iter().map(|p| transform(&view, *p)).collect();
        let outline = part.material.layer.is_outline();
        for (ti, t) in part.mesh.tris.iter().enumerate() {
            let w = [world[t[0]], world[t[1]], world[t[2]]];
            let Some(normal) = normalize(cross(sub(w[1], w[0]), sub(w[2], w[0]))) else {
                continue;
            };
            let centre = scale(
                [
                    w[0][0] + w[1][0] + w[2][0],
                    w[0][1] + w[1][1] + w[2][1],
                    w[0][2] + w[1][2] + w[2][2],
                ],
                1.0 / 3.0,
            );
            let Some(to_eye) = normalize(sub(eye, centre)) else {
                continue;
            };
            let facing = dot(normal, to_eye) * if outline { -1.0 } else { 1.0 };
            if facing <= 0.0 {
                continue;
            }
            let v = [viewed[t[0]], viewed[t[1]], viewed[t[2]]];
            let image: Option<Vec<_>> = v
                .iter()
                .map(|p| {
                    camera
                        .projection
                        .project(&garust::pga::Point::new(p[0], p[1], p[2]))
                })
                .collect();
            let Some(image) = image else {
                continue;
            };
            out.push(Face {
                id: ((pi as u64) << 32) | ti as u64,
                world: w,
                view: v,
                image,
                normal,
                light_dir: light.dir(),
                light: light.term(normal),
                facing,
                depth: -(v[0][2] + v[1][2] + v[2][2]) / 3.0 - part.bias,
                material: &part.material,
                projection: camera.projection,
            });
        }
    }
    out.sort_by(|a, b| b.depth.total_cmp(&a.depth).then(a.id.cmp(&b.id)));
    out
}

/// The faces painted in order by `acabado`.
#[must_use]
pub fn paint(faces: &[Face], acabado: &dyn Acabado) -> Vec<Prim2> {
    let mut out = Vec::new();
    for face in faces {
        acabado.paint(face, &mut out);
    }
    out
}
