//! The set: the navy backdrop and its vignette, the warm spotlight (both
//! screen-space), the lifting platform (meshes) and the pool of light and
//! the toy's shadow on it (flat paths posed onto the floor).

use std::f64::consts::{FRAC_1_SQRT_2, TAU};

use garust::{pga, Motor3};
use motoreel::shapes::polygon;
use motoreel::{Camera, Fill, Gradient, Object, Prim2, Pt2, Rgb, Stop, Style, Track};

use crate::kit::{Shape, V3};
use crate::palette::{goma, madera, tinta_objeto, CALIDO, MARINO_OSCURO, TINTA};

/// The frame, image units (= px), centred.
pub const VIEW: (f64, f64) = (1080.0, 1920.0);

/// A box and its ink hull: the platform's pieces.
fn slab(lo: V3, hi: V3, material: motoreel::Material, out: &mut Vec<Object>) {
    let g = 0.009;
    let hull = Shape::cuboid(
        [lo[0] - g, lo[1] - g, lo[2] - g],
        [hi[0] + g, hi[1] + g, hi[2] + g],
    );
    out.push(Object::mesh(hull.hull().mesh(tinta_objeto())));
    out.push(Object::mesh(Shape::cuboid(lo, hi).mesh(material)));
}

/// The platform: a wood slab between two rubber strips, three separate
/// boxes. The spike had to merge them into one slab because a face-level
/// sort drew a strip's big side over the wood's top; the object-level
/// order draws them right. Its top is the floor, `y = 0`.
pub fn platform() -> Vec<Object> {
    let mut out = Vec::new();
    slab([-0.6, -0.1, -0.42], [0.6, 0.0, 0.42], madera(), &mut out);
    slab([-0.9, -0.1, -0.42], [-0.6, -0.02, 0.42], goma(), &mut out);
    slab([0.6, -0.1, -0.42], [0.9, -0.02, 0.42], goma(), &mut out);
    out
}

/// A flat ellipse posed onto the floor (the model plane `z = 0` turned
/// onto `y = 0.002`), as an ordinary path object.
fn floor_ellipse(c: V3, rx: f64, rz: f64, colour: Rgb, alpha: f64) -> Object {
    let ring: Vec<Pt2> = (0..32)
        .map(|k| {
            let t = TAU * f64::from(k) / 32.0;
            Pt2 {
                x: rx * t.cos(),
                y: rz * t.sin(),
            }
        })
        .collect();
    let onto_floor = Motor3::translator(c[0], 0.002, c[2]).compose(&Motor3::from_unit_quaternion(
        FRAC_1_SQRT_2,
        FRAC_1_SQRT_2,
        0.0,
        0.0,
    ));
    Object::planar(polygon(&ring))
        .with_style(Style {
            stroke: colour,
            width: 0.0,
            alpha: 0.0,
            fill: Some(Fill::solid(colour, alpha)),
        })
        .with_track(Track::hold(onto_floor))
}

/// The warm pool of light on the platform and the toy's shadow in it.
pub fn floor_marks() -> Vec<Object> {
    vec![
        floor_ellipse([0.0; 3], 0.62, 0.5, CALIDO, 0.07),
        floor_ellipse([0.0; 3], 0.46, 0.37, CALIDO, 0.08),
        floor_ellipse([0.02, 0.0, 0.0], 0.27, 0.36, TINTA, 0.5),
    ]
}

/// A screen-space polygon, filled.
fn screen(pts: &[Pt2], style: Style) -> Option<Prim2> {
    let subpaths = polygon(pts);
    (!subpaths.is_empty()).then_some(Prim2::Path { subpaths, style })
}

/// A fill-only style.
pub fn fill_only(colour: Rgb, alpha: f64) -> Style {
    Style {
        stroke: colour,
        width: 0.0,
        alpha: 0.0,
        fill: Some(Fill::solid(colour, alpha)),
    }
}

/// A full-height bar `w` wide at image `x`, filled.
pub fn bar(x: f64, w: f64, colour: Rgb, alpha: f64) -> Option<Prim2> {
    let (top, bottom) = (VIEW.1 / 2.0, -VIEW.1 / 2.0);
    let pts = [
        Pt2 {
            x: x - w / 2.0,
            y: bottom,
        },
        Pt2 {
            x: x + w / 2.0,
            y: bottom,
        },
        Pt2 {
            x: x + w / 2.0,
            y: top,
        },
        Pt2 {
            x: x - w / 2.0,
            y: top,
        },
    ];
    screen(&pts, fill_only(colour, alpha))
}

/// The whole frame, filled.
pub fn whole(colour: Rgb, alpha: f64) -> Option<Prim2> {
    bar(0.0, VIEW.0, colour, alpha)
}

fn stop(offset: f64, colour: Rgb, alpha: f64) -> Stop {
    Stop {
        offset,
        colour,
        alpha,
    }
}

/// The vignette and the warm cone from above: screen-space, behind all.
pub fn backdrop(camera: &Camera) -> Vec<Prim2> {
    let hh = VIEW.1 / 2.0;
    let mut out = Vec::new();
    if let Ok(g) = Gradient::radial(
        Pt2 { x: 0.0, y: 0.0 },
        1250.0,
        &[stop(0.4, MARINO_OSCURO, 0.0), stop(1.0, MARINO_OSCURO, 1.0)],
    ) {
        out.extend(whole(MARINO_OSCURO, 1.0).map(|p| match p {
            Prim2::Path { subpaths, style } => Prim2::Path {
                subpaths,
                style: Style {
                    fill: Some(Fill::gradient(g, 1.0)),
                    ..style
                },
            },
            other => other,
        }));
    }
    let origin = pga::Point::new(0.0, 0.0, 0.0).transform(&camera.pose.inverse());
    let floor = camera.projection.project(&origin).map_or(-500.0, |p| p.y);
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
        out.extend(screen(
            &cone,
            Style {
                fill: Some(Fill::gradient(g, 1.0)),
                ..fill_only(CALIDO, 1.0)
            },
        ));
    }
    out
}
