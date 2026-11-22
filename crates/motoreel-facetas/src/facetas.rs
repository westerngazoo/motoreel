//! «Facetas»: flat-shaded low poly, the toy's look in 3D.
//!
//! One colour per face from its Lambert term on the material's three
//! tones, and an ink outline from the inverted hull (the outline layers).

use motoreel::{Prim2, Rgb};

use crate::acabado::{polygon, sealed, Acabado};
use crate::face::Face;
use crate::material::Layer;

/// The faceted finish.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Facetas {
    /// The outline colour (`monito-tinta`).
    pub ink: Rgb,
    /// The seam stroke, image units (≈ 1 px).
    pub seam: f64,
}

impl Acabado for Facetas {
    fn paint(&self, face: &Face, out: &mut Vec<Prim2>) {
        let colour = match face.material.layer {
            Layer::Inside => return,
            Layer::BodyOutline | Layer::ObjectOutline => self.ink,
            Layer::Body | Layer::Object => face.material.tones.at(face.light),
        };
        out.extend(polygon(&face.image, sealed(colour, 1.0, self.seam)));
    }
}
