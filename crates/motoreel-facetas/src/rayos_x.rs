//! «Rayos X»: the body turns to cyan glass and what is inside lights up.
//!
//! The house's signature move («igual por fuera, distinto por dentro»)
//! in 3D: body faces become a faint fill with bright facet edges, inside
//! faces (bone, the biceps line) glow in their own tones, and props keep
//! the faceted look so the scene stays put around the figure.

use motoreel::{Fill, Prim2, Rgb, Style};

use crate::acabado::{polygon, sealed, Acabado};
use crate::face::Face;
use crate::facetas::Facetas;
use crate::material::Layer;

/// The x-ray finish.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayosX {
    /// The glass colour (`monito-cian`).
    pub glass: Rgb,
    /// How props and their outlines are painted.
    pub solid: Facetas,
}

impl RayosX {
    /// Body fill opacity: glass you can see through.
    const GLASS_FILL: f64 = 0.10;
    /// Body facet-edge opacity and width (px).
    const GLASS_EDGE: (f64, f64) = (0.30, 1.6);
    /// The glow around an inside face: opacity and width (px).
    const GLOW: (f64, f64) = (0.16, 12.0);
}

impl Acabado for RayosX {
    fn paint(&self, face: &Face, out: &mut Vec<Prim2>) {
        match face.material.layer {
            Layer::Object | Layer::ObjectOutline => self.solid.paint(face, out),
            Layer::BodyOutline => {}
            Layer::Body => {
                let style = Style {
                    stroke: self.glass,
                    width: Self::GLASS_EDGE.1,
                    alpha: Self::GLASS_EDGE.0,
                    fill: Some(Fill::solid(self.glass, Self::GLASS_FILL)),
                };
                out.extend(polygon(&face.image, style));
            }
            Layer::Inside => {
                let tones = face.material.tones;
                let glow = Style {
                    stroke: tones.base,
                    width: Self::GLOW.1,
                    alpha: Self::GLOW.0,
                    fill: None,
                };
                out.extend(polygon(&face.image, glow));
                let core = tones.at(0.55 + 0.45 * face.light);
                out.extend(polygon(&face.image, sealed(core, 1.0, self.solid.seam)));
            }
        }
    }
}
