//! The set: navy backdrop with its vignette, the warm spotlight, the
//! lifting platform (3D, through the finish) and the light pool and
//! shadow on it (flat marks projected onto the floor).

use std::f64::consts::TAU;

use motoreel::{Camera, Fill, Gradient, Prim2, Pt2, Stop, Style};
use motoreel_facetas::acabado::{fill_only, polygon};
use motoreel_facetas::vec3::{transform, V3};
use motoreel_facetas::{Layer, Material, Mesh, Part, Tones};

use crate::palette::{CALIDO, MADERA, MARINO_OSCURO, TINTA};

/// The frame, image units (= px), centred.
pub const VIEW: (f64, f64) = (1080.0, 1920.0);

/// The platform: a wood slab with its ink hull. Its top is the floor,
/// `y = 0`.
pub fn platform() -> Vec<Part> {
    let mut parts = Vec::new();
    let mut slab = |lo: V3, hi: V3, tones: Tones| {
        let g = 0.009;
        let hull = Mesh::cuboid(
            [lo[0] - g, lo[1] - g, lo[2] - g],
            [hi[0] + g, hi[1] + g, hi[2] + g],
        );
        let still = garust::Motor3::identity();
        parts.push(Part {
            mesh: hull,
            material: Material {
                tones,
                layer: Layer::ObjectOutline,
            },
            pose: still,
            bias: 0.0,
        });
        parts.push(Part {
            mesh: Mesh::cuboid(lo, hi),
            material: Material {
                tones,
                layer: Layer::Object,
            },
            pose: still,
            bias: 0.0,
        });
    };
    // One slab: wood beside rubber as separate boxes sorted wrong (a big
    // side face of one over the top of the other), the painter's sort's
    // long-face case; the 2D toy's rubber strips wait for a better sort.
    slab([-0.6, -0.1, -0.42], [0.6, 0.0, 0.42], MADERA);
    parts
}

/// A world point through `camera`, or `None` if culled.
fn project(camera: &Camera, p: V3) -> Option<Pt2> {
    let v = transform(&camera.pose.inverse().to_matrix(), p);
    camera
        .projection
        .project(&garust::pga::Point::new(v[0], v[1], v[2]))
}

/// A stop of `colour` at `alpha`: every fade keeps its colour and only
/// loses opacity, as the house's light pieces do.
fn stop(offset: f64, colour: motoreel::Rgb, alpha: f64) -> Stop {
    Stop {
        offset,
        colour,
        alpha,
    }
}

/// The vignette and the warm cone from above: screen-space, behind all.
pub fn backdrop(camera: &Camera) -> Vec<Prim2> {
    let (hw, hh) = (VIEW.0 / 2.0, VIEW.1 / 2.0);
    let frame = [
        Pt2 { x: -hw, y: -hh },
        Pt2 { x: hw, y: -hh },
        Pt2 { x: hw, y: hh },
        Pt2 { x: -hw, y: hh },
    ];
    let mut out = Vec::new();
    let centre = Pt2 { x: 0.0, y: 0.0 };
    if let Ok(g) = Gradient::radial(
        centre,
        1250.0,
        &[stop(0.4, MARINO_OSCURO, 0.0), stop(1.0, MARINO_OSCURO, 1.0)],
    ) {
        out.extend(polygon(
            &frame,
            Style {
                fill: Some(Fill::gradient(g, 1.0)),
                ..fill_only(MARINO_OSCURO, 1.0)
            },
        ));
    }
    let floor = project(camera, [0.0, 0.0, 0.0]).map_or(-500.0, |p| p.y);
    let cone = [
        Pt2 { x: -70.0, y: hh },
        Pt2 { x: 70.0, y: hh },
        Pt2 { x: 470.0, y: floor },
        Pt2 {
            x: -470.0,
            y: floor,
        },
    ];
    if let Ok(g) = Gradient::linear(
        Pt2 { x: 0.0, y: hh },
        Pt2 { x: 0.0, y: floor },
        &[stop(0.0, CALIDO, 0.20), stop(1.0, CALIDO, 0.05)],
    ) {
        out.extend(polygon(
            &cone,
            Style {
                fill: Some(Fill::gradient(g, 1.0)),
                ..fill_only(CALIDO, 1.0)
            },
        ));
    }
    out
}

/// A flat ellipse on the floor, projected.
fn floor_ellipse(camera: &Camera, c: V3, rx: f64, rz: f64) -> Vec<Pt2> {
    (0..32)
        .filter_map(|k| {
            let t = TAU * f64::from(k) / 32.0;
            project(camera, [c[0] + rx * t.cos(), 0.002, c[2] + rz * t.sin()])
        })
        .collect()
}

/// The warm pool of light on the platform and the toy's shadow in it.
pub fn floor_marks(camera: &Camera) -> Vec<Prim2> {
    let mut out = Vec::new();
    for (r, a) in [(0.62, 0.07), (0.46, 0.08)] {
        out.extend(polygon(
            &floor_ellipse(camera, [0.0; 3], r, r * 0.8),
            fill_only(CALIDO, a),
        ));
    }
    out.extend(polygon(
        &floor_ellipse(camera, [0.02, 0.0, 0.0], 0.27, 0.36),
        fill_only(TINTA, 0.5),
    ));
    out
}
