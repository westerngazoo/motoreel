//! The seam between geometry and finish: one visible face, already lit,
//! projected and sorted, with everything a finish may want to know.

use motoreel::{Projection, Pt2};

use crate::material::Material;
use crate::vec3::{add, scale, sub, V3};

/// One triangle as a finish sees it.
///
/// Built by [`crate::pipeline::faces`]; a finish reads it and emits
/// motoreel primitives. `image` is a polygon so a composition (the x-ray
/// sweep) can clip it; `world` and `view` stay the full triangle.
#[derive(Clone, Debug, PartialEq)]
pub struct Face<'a> {
    /// Stable across frames: part index in the high bits, triangle index
    /// in the low ones. Seeds anything a finish randomizes.
    pub id: u64,
    /// The triangle in world space.
    pub world: [V3; 3],
    /// The triangle in view space (the camera looks along −z).
    pub view: [V3; 3],
    /// The projected outline, image space (y up). Three points unless a
    /// composition clipped it.
    pub image: Vec<Pt2>,
    /// The unit world-space normal (outward).
    pub normal: V3,
    /// The unit direction toward the key light, world space.
    pub light_dir: V3,
    /// Lambert term in `[0, 1]`.
    pub light: f64,
    /// How squarely the face looks at the camera: `n·v` in `[0, 1]`, 0 at
    /// grazing. (For an outline face, the same of the flipped normal.)
    pub facing: f64,
    /// Mean view depth, positive ahead of the camera, less the part's
    /// sort bias: the key the painter's sort uses.
    pub depth: f64,
    /// The surface.
    pub material: &'a Material,
    /// The projection, so a finish can place its own points on the face.
    pub projection: Projection,
}

impl Face<'_> {
    /// The point at barycentric `(u, v)` (weights `1 − u − v`, `u`, `v` on
    /// vertices 0, 1, 2), in view space.
    #[must_use]
    pub fn view_at(&self, u: f64, v: f64) -> V3 {
        let [a, b, c] = self.view;
        add(a, add(scale(sub(b, a), u), scale(sub(c, a), v)))
    }

    /// A view-space point projected to image space; `None` if culled.
    #[must_use]
    pub fn project(&self, p: V3) -> Option<Pt2> {
        self.projection
            .project(&garust::pga::Point::new(p[0], p[1], p[2]))
    }

    /// The world-space triangle area.
    #[must_use]
    pub fn area(&self) -> f64 {
        let [a, b, c] = self.world;
        0.5 * crate::vec3::length(crate::vec3::cross(sub(b, a), sub(c, a)))
    }
}
