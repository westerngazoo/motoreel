//! The fbf vinyl toy as 3D parts, curling two dumbbells.
//!
//! Joint positions and lengths are the 2D toy's (rotorf-sico
//! `fbf/motor/src/juguete.rs` and `curl.rs`, metres: upper arm 0.30,
//! forearm 0.32, head radius 0.145). Limb radii are the 2D toy's ×
//! [`CHUNK`]: a profile drawing's capsule reads as a thick limb, a 3D
//! capsule of the same radius reads thin from the front, so the licence is
//! taken here. The torso is widened sideways, which the profile never had
//! to decide. The figure faces +x with its right side toward +z; the elbow
//! is the only joint that moves, as a garust motor about the elbow's axis.

use garust::Motor3;
use motoreel_facetas::camera::turn_z;
use motoreel_facetas::vec3::{add, lerp, normalize, scale, transform, V3};
use motoreel_facetas::{Layer, Material, Mesh, Part, Tones};

use crate::palette::{ACERO, AMARILLO, AZUL, BICEPS, HUESO, PUPILA, SHORT, TENIS};

/// The ink hull's offset (m): ≈ 5 px at this framing, the 2D toy's +5 px.
const INK: f64 = 0.009;
/// Limb radii over the 2D toy's.
const CHUNK: f64 = 1.25;
/// Facets around a limb.
const SIDES: usize = 8;
/// Latitude bands per capsule end.
const RINGS: usize = 2;

/// The parts, collected with their ink hulls.
#[derive(Default)]
struct Rig {
    parts: Vec<Part>,
}

impl Rig {
    /// A solid and, behind it, its ink hull: `build(grow)` makes the mesh
    /// with every radius grown by `grow`. `bias` is the sort priority
    /// ([`Part::bias`]): which of two touching parts wraps the other.
    fn outlined(
        &mut self,
        build: impl Fn(f64) -> Mesh,
        tones: Tones,
        layer: Layer,
        pose: Motor3,
        bias: f64,
    ) {
        let hull = if layer == Layer::Object {
            Layer::ObjectOutline
        } else {
            Layer::BodyOutline
        };
        self.parts.push(Part {
            mesh: build(INK),
            material: Material { tones, layer: hull },
            pose,
            bias,
        });
        self.plain(build(0.0), tones, layer, pose, bias);
    }

    /// A part with no outline.
    fn plain(&mut self, mesh: Mesh, tones: Tones, layer: Layer, pose: Motor3, bias: f64) {
        self.parts.push(Part {
            mesh,
            material: Material { tones, layer },
            pose,
            bias,
        });
    }
}

/// The rotation of the forearm about the elbow's z axis by `elbow`
/// radians: `T(e) · R_z · T(−e)`, positive bringing the hand forward and up.
fn about_elbow(e: V3, elbow: f64) -> Motor3 {
    Motor3::translator(e[0], e[1], e[2])
        .compose(&turn_z(elbow))
        .compose(&Motor3::translator(-e[0], -e[1], -e[2]))
}

/// A capsule with the 2D toy's radii made chunky.
fn limb(a: V3, b: V3, ra: f64, rb: f64, grow: f64) -> Mesh {
    Mesh::capsule(a, b, ra * CHUNK + grow, rb * CHUNK + grow, SIDES, RINGS)
}

/// The toy with both elbows flexed `elbow` radians from straight.
pub fn figure(elbow: f64) -> Vec<Part> {
    let still = Motor3::identity();
    let mut r = Rig::default();
    let body = Layer::Body;
    for side in [1.0, -1.0] {
        let z = |v: f64| v * side;
        // The leg, the shorts over the thigh, the shoe and its sole.
        let (ankle, knee, hip) = (
            [0.0, 0.09, z(0.115)],
            [0.01, 0.50, z(0.115)],
            [0.0, 0.92, z(0.105)],
        );
        r.outlined(
            |g| limb(ankle, knee, 0.055, 0.068, g),
            AZUL,
            body,
            still,
            0.0,
        );
        r.outlined(|g| limb(knee, hip, 0.072, 0.095, g), AZUL, body, still, 0.0);
        let mid = lerp(knee, hip, 0.5);
        r.outlined(
            |g| limb(mid, hip, 0.085, 0.105, g),
            SHORT,
            body,
            still,
            0.06,
        );
        let (heel, toe) = ([-0.08, 0.055, z(0.115)], [0.16, 0.045, z(0.115)]);
        r.outlined(
            |g| Mesh::capsule(heel, toe, 0.06 + g, 0.05 + g, SIDES, RINGS),
            TENIS,
            body,
            still,
            0.08,
        );
        let sole_c = [0.04, 0.014, z(0.115)];
        let sole = Mesh::capsule(
            [-0.09, 0.014, z(0.115)],
            [0.18, 0.014, z(0.115)],
            0.05,
            0.045,
            SIDES,
            1,
        )
        .scaled([1.0, 0.3, 1.15], sole_c);
        r.plain(sole, AMARILLO, body, still, 0.05);

        // The arm: the upper arm hangs still; the forearm, hand and
        // dumbbell are built hanging and posed by the elbow motor.
        let s = [0.0, 1.37, z(0.245)];
        let e = [-0.01, 1.08, z(0.29)];
        let wrist = [-0.01, 0.78, z(0.31)];
        let hand = [-0.01, 0.75, z(0.315)];
        let fore = about_elbow(e, elbow);
        r.outlined(|g| limb(s, e, 0.06, 0.045, g), AZUL, body, still, 0.04);
        r.outlined(|g| limb(e, wrist, 0.043, 0.04, g), AZUL, body, fore, 0.05);
        r.outlined(
            |g| Mesh::sphere(hand, 0.06 + g, SIDES, 3),
            AZUL,
            body,
            fore,
            0.07,
        );
        let along = |d: f64| add(hand, [0.0, 0.0, d]);
        r.outlined(
            |g| Mesh::prism(along(-0.11), along(0.11), 0.018 + g, 6),
            ACERO,
            Layer::Object,
            fore,
            0.06,
        );
        for sign in [1.0, -1.0] {
            r.outlined(
                |g| {
                    Mesh::prism(
                        along(sign * (0.062 - g)),
                        along(sign * (0.115 + g)),
                        0.07 + g,
                        6,
                    )
                },
                ACERO,
                Layer::Object,
                fore,
                0.06,
            );
        }

        // Inside: the bones of the limbs, and the biceps from the front
        // of the shoulder to just below the elbow on the forearm.
        let inside = Layer::Inside;
        r.plain(
            Mesh::capsule(ankle, knee, 0.02, 0.02, 6, 1),
            HUESO,
            inside,
            still,
            0.0,
        );
        r.plain(
            Mesh::capsule(knee, hip, 0.024, 0.024, 6, 1),
            HUESO,
            inside,
            still,
            0.0,
        );
        r.plain(
            Mesh::capsule(s, e, 0.02, 0.018, 6, 1),
            HUESO,
            inside,
            still,
            0.0,
        );
        for dx in [0.013, -0.013] {
            r.plain(
                Mesh::capsule(
                    add(e, [dx, -0.02, 0.0]),
                    add(wrist, [dx, 0.0, 0.0]),
                    0.011,
                    0.011,
                    6,
                    1,
                ),
                HUESO,
                inside,
                fore,
                0.0,
            );
        }
        let origin = add(s, [0.055, -0.05, 0.0]);
        let insertion = transform(&fore.to_matrix(), add(e, [0.045, -0.06, 0.0]));
        r.plain(
            Mesh::capsule(origin, insertion, 0.02, 0.015, 6, 1),
            BICEPS,
            inside,
            still,
            0.03,
        );
    }

    // The pelvis in shorts, the chunky torso, the neck.
    r.outlined(
        |g| {
            Mesh::capsule(
                [0.0, 0.97, -0.12],
                [0.0, 0.97, 0.12],
                0.14 + g,
                0.14 + g,
                10,
                RINGS,
            )
        },
        SHORT,
        body,
        still,
        0.05,
    );
    let torso_c = [0.0, 1.17, 0.0];
    r.outlined(
        |g| {
            Mesh::capsule(
                [0.0, 0.99, 0.0],
                [0.0, 1.33, 0.0],
                0.13 + g,
                0.16 + g,
                10,
                RINGS,
            )
            .scaled([0.95, 1.0, 1.4], torso_c)
        },
        AZUL,
        body,
        still,
        0.0,
    );
    r.outlined(
        |g| {
            Mesh::capsule(
                [0.0, 1.40, 0.0],
                [0.01, 1.55, 0.0],
                0.06 + g,
                0.06 + g,
                SIDES,
                1,
            )
        },
        AZUL,
        body,
        still,
        -0.02,
    );

    // The big head, its headband, two eyes looking forward.
    let c = [0.03, 1.68, 0.0];
    let rh = 0.145;
    r.outlined(|g| Mesh::sphere(c, rh + g, 10, 4), AZUL, body, still, 0.02);
    r.outlined(
        |g| {
            Mesh::band(
                add(c, [0.0, 0.02, 0.0]),
                add(c, [0.0, 0.065, 0.0]),
                0.155 + g,
                10,
            )
        },
        AMARILLO,
        body,
        still,
        0.05,
    );
    for side in [1.0, -1.0] {
        let Some(dir) = normalize([0.93, -0.03, 0.36 * side]) else {
            continue;
        };
        let eye = add(c, scale(dir, 0.9 * rh));
        r.plain(
            Mesh::sphere(eye, 0.036, SIDES, 3).scaled([0.45, 1.1, 1.0], eye),
            TENIS,
            body,
            still,
            0.06,
        );
        r.plain(
            Mesh::sphere(add(c, scale(dir, 0.99 * rh)), 0.018, 6, 2).scaled([0.5, 1.0, 1.0], eye),
            PUPILA,
            body,
            still,
            0.08,
        );
    }

    // Inside the trunk: the spine and the collarbones.
    r.plain(
        Mesh::capsule([0.0, 0.95, 0.0], [0.0, 1.52, 0.0], 0.022, 0.02, 6, 1),
        HUESO,
        Layer::Inside,
        still,
        0.0,
    );
    r.plain(
        Mesh::capsule([0.0, 1.40, -0.22], [0.0, 1.40, 0.22], 0.014, 0.014, 6, 1),
        HUESO,
        Layer::Inside,
        still,
        0.0,
    );
    r.parts
}
