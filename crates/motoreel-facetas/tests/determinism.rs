//! The same scene in the same finish gives the same primitives, call after
//! call; the painted finish's strokes come from its seed; the faceted
//! finish hides what is inside.

use garust::Motor3;
use motoreel::{Prim2, Rgb};
use motoreel_facetas::{
    faces, paint, Acabado, Brush, Facetas, Layer, Light, Material, Mesh, Orbit, Part, Pintura,
    RayosX, Tones,
};

const T: Tones = Tones {
    light: Rgb {
        r: 200,
        g: 220,
        b: 255,
    },
    base: Rgb {
        r: 40,
        g: 180,
        b: 230,
    },
    shadow: Rgb {
        r: 20,
        g: 100,
        b: 160,
    },
};

fn scene() -> Vec<Part> {
    vec![Part {
        mesh: Mesh::capsule([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], 0.3, 0.2, 8, 2),
        material: Material {
            tones: T,
            layer: Layer::Body,
        },
        pose: Motor3::identity(),
        bias: 0.0,
    }]
}

fn render(acabado: &dyn Acabado, azimuth: f64) -> Vec<Prim2> {
    let parts = scene();
    let orbit = Orbit {
        target: [0.0, 0.5, 0.0],
        distance: 4.0,
        azimuth,
        elevation: 0.1,
        focal: 1500.0,
    };
    let light = Light::new([0.5, 0.8, 0.3], 0.3).expect("a valid light");
    paint(&faces(&parts, &orbit.camera(), &light), acabado)
}

const PINTURA: Pintura = Pintura {
    seed: 7,
    brushes: [
        Brush {
            width: 0.05,
            length: 0.1,
            density: 1.0,
            tone_jitter: 0.2,
            alpha: 0.9,
            refine: false,
        },
        Brush {
            width: 0.02,
            length: 0.05,
            density: 1.0,
            tone_jitter: 0.2,
            alpha: 0.9,
            refine: true,
        },
    ],
    seam: 1.0,
};

#[test]
fn every_finish_is_repeatable() {
    let facetas = Facetas {
        ink: Rgb::BLACK,
        seam: 1.0,
    };
    let rayos = RayosX {
        glass: Rgb::WHITE,
        solid: facetas,
    };
    for acabado in [&facetas as &dyn Acabado, &PINTURA, &rayos] {
        let a = render(acabado, 0.4);
        assert!(!a.is_empty());
        assert_eq!(a, render(acabado, 0.4));
    }
}

#[test]
fn a_different_seed_paints_different_strokes() {
    let other = Pintura { seed: 8, ..PINTURA };
    assert_ne!(render(&PINTURA, 0.4), render(&other, 0.4));
}

#[test]
fn the_inside_is_hidden_by_the_faceted_finish() {
    let mut parts = scene();
    parts[0].material.layer = Layer::Inside;
    let orbit = Orbit {
        target: [0.0, 0.5, 0.0],
        distance: 4.0,
        azimuth: 0.0,
        elevation: 0.0,
        focal: 1500.0,
    };
    let light = Light::new([0.0, 1.0, 0.0], 0.3).expect("a valid light");
    let facetas = Facetas {
        ink: Rgb::BLACK,
        seam: 1.0,
    };
    assert!(paint(&faces(&parts, &orbit.camera(), &light), &facetas).is_empty());
}
