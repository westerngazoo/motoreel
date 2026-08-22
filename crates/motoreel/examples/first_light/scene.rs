//! The RFC-012 §3.5 screw demo, join-line omitted (M3): a unit square
//! rides a full screw (one turn about z while rising 2) and a point
//! orbits it. Shared verbatim by `tests/r0003_svg_sink.rs` via `#[path]`,
//! so the shipped demo and the determinism test cannot drift apart.
//! Engine items are named through `motoreel::` paths only — the one
//! spelling valid from both the example crate and the test crate.

use std::f64::consts::TAU;

use garust::{Motor3, Pga3};
use motoreel::{Camera, Object, Rgb, Scene, Style, Track};

/// Build the first-light scene: 4 seconds, pinhole camera 6 back.
pub fn scene() -> Scene {
    // Full-turn caution (SPEC-0003 §2.7): rotor(TAU) is antipodal —
    // author the screw as quarter-turn keys placed exactly on the
    // one-parameter screw (z-rotation and z-translation commute).
    let screw = Track::keys((0..=4).map(|k| {
        let s = f64::from(k) / 4.0; // derived, not accumulated
        let pose =
            Motor3::translator(0.0, 0.0, 2.0 * s) * Motor3::rotor(TAU * s, Pga3::basis(0b0011));
        (4.0 * s, pose)
    }))
    .expect("static demo keys are strictly increasing and non-empty");

    let orbit = Track::spin(TAU / 4.0, Pga3::basis(0b0011), 4.0)
        .expect("static demo spin parameters are valid");

    let square = Object::polyline(vec![
        flat_point(-0.5, -0.5),
        flat_point(0.5, -0.5),
        flat_point(0.5, 0.5),
        flat_point(-0.5, 0.5),
        flat_point(-0.5, -0.5), // closed: first vertex repeated
    ])
    .with_style(Style {
        stroke: Rgb {
            r: 0xe0,
            g: 0xe0,
            b: 0xe0,
        },
        width: 0.02,
        alpha: 1.0,
    })
    .with_track(screw);

    let orbiter = Object::point(flat_point(2.0, 0.0))
        .with_style(Style {
            stroke: Rgb {
                r: 0xff,
                g: 0x4d,
                b: 0x00,
            },
            width: 0.06,
            alpha: 1.0,
        })
        .with_track(orbit);

    let mut scene = Scene::new(4.0);
    scene.camera = Camera::pinhole(Motor3::translator(0.0, 0.0, 6.0), 2.0);
    scene.add(square);
    scene.add(orbiter);
    scene
}

/// A PGA point in the z = 0 plane (demo geometry is authored flat).
fn flat_point(x: f64, y: f64) -> garust::pga::Point {
    garust::pga::Point::new(x, y, 0.0)
}
