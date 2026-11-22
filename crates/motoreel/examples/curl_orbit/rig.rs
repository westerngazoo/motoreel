//! The fbf vinyl toy as mesh objects, curling two dumbbells.
//!
//! Proportions are the spike's (PR #8), taken from rotorf-sico
//! `fbf/motor/src/juguete.rs` and `curl.rs` (metres: upper arm 0.30,
//! forearm 0.32, head radius 0.145), with the limb radii × [`CHUNK`]: a
//! profile drawing's capsule reads thick, a 3D one of the same radius
//! reads thin from the front. The figure faces +x with its right side
//! toward +z. Each part is one object; the forearm, hand and dumbbell ride
//! one motor track about the elbow's axis, so the curl is a screw the
//! track interpolates, not a pose rebuilt per frame.

use garust::{Motor3, Pga3};
use motoreel::{Ease, Material, Object, Track};

use crate::kit::{add, lerp, normalize, scale, Shape, V3};
use crate::palette::{
    acero, amarillo, azul, biceps, hueso, pupila, short, tenis, tinta_cuerpo, tinta_objeto,
};

/// The ink hull's offset (m): about 5 px at this framing.
const INK: f64 = 0.009;
/// Limb radii over the 2D toy's.
const CHUNK: f64 = 1.25;
/// Facets around a limb.
const SIDES: usize = 8;
/// Latitude bands per capsule end.
const BANDS: usize = 2;

/// When the curl stops (seconds), and the elbow's flexion from straight
/// at the start and at the freeze (degrees). 90° is the load's longest
/// lever arm: the moment worth freezing.
pub const FREEZE: f64 = 1.5;
const ELBOW: (f64, f64) = (10.0, 90.0);

/// Accelerating into the freeze, so it cuts the motion at speed:
/// `u^1.5`, anchored at 0 and 1.
fn into_the_freeze(u: f64) -> f64 {
    u * u.sqrt()
}

/// A right-handed turn of `radians` about +z (carries +x toward +y; the
/// sign is the spike's, pinned there by a test).
fn turn_z(radians: f64) -> Motor3 {
    Motor3::rotor(radians, Pga3::basis(0b0011))
}

/// The forearm turned `degrees` about the elbow `e`'s z axis:
/// `T(e) · R_z · T(−e)`.
fn about_elbow(e: V3, degrees: f64) -> Motor3 {
    Motor3::translator(e[0], e[1], e[2])
        .compose(&turn_z(degrees.to_radians()))
        .compose(&Motor3::translator(-e[0], -e[1], -e[2]))
}

/// The parts, in insertion order; the depth sort orders them per frame.
#[derive(Default)]
struct Rig {
    objects: Vec<Object>,
}

impl Rig {
    /// A part and its ink hull: `build(grow)` makes the shape with every
    /// radius grown by `grow`.
    fn outlined(
        &mut self,
        build: impl Fn(f64) -> Shape,
        material: Material,
        ink: Material,
        track: &Track,
    ) {
        self.objects
            .push(Object::mesh(build(INK).hull().mesh(ink)).with_track(track.clone()));
        self.plain(build(0.0), material, track);
    }

    /// A part with no outline.
    fn plain(&mut self, shape: Shape, material: Material, track: &Track) {
        self.objects
            .push(Object::mesh(shape.mesh(material)).with_track(track.clone()));
    }
}

/// A capsule with the 2D toy's radii made chunky.
fn limb(a: V3, b: V3, ra: f64, rb: f64, grow: f64) -> Shape {
    Shape::capsule(a, b, ra * CHUNK + grow, rb * CHUNK + grow, SIDES, BANDS)
}

/// The toy, both elbows on the curl track.
pub fn figure() -> Vec<Object> {
    let still = Track::hold(Motor3::identity());
    let mut r = Rig::default();
    let (body, prop) = (tinta_cuerpo(), tinta_objeto());
    for side in [1.0, -1.0] {
        let z = |v: f64| v * side;
        // The leg, the shorts over the thigh, the shoe and its sole.
        let (ankle, knee, hip) = (
            [0.0, 0.09, z(0.115)],
            [0.01, 0.50, z(0.115)],
            [0.0, 0.92, z(0.105)],
        );
        r.outlined(|g| limb(ankle, knee, 0.055, 0.068, g), azul(), body, &still);
        r.outlined(|g| limb(knee, hip, 0.072, 0.095, g), azul(), body, &still);
        let mid = lerp(knee, hip, 0.5);
        r.outlined(|g| limb(mid, hip, 0.085, 0.105, g), short(), body, &still);
        let (heel, toe) = ([-0.08, 0.055, z(0.115)], [0.16, 0.045, z(0.115)]);
        r.outlined(
            |g| Shape::capsule(heel, toe, 0.06 + g, 0.05 + g, SIDES, BANDS),
            tenis(),
            body,
            &still,
        );
        let sole_c = [0.04, 0.014, z(0.115)];
        let sole = Shape::capsule(
            [-0.09, 0.014, z(0.115)],
            [0.18, 0.014, z(0.115)],
            0.05,
            0.045,
            SIDES,
            1,
        )
        .scaled([1.0, 0.3, 1.15], sole_c);
        r.plain(sole, amarillo(), &still);

        // The arm: the upper arm hangs still; the forearm, hand and
        // dumbbell are built hanging and ride the elbow's track.
        let s = [0.0, 1.37, z(0.245)];
        let e = [-0.01, 1.08, z(0.29)];
        let wrist = [-0.01, 0.78, z(0.31)];
        let hand = [-0.01, 0.75, z(0.315)];
        let fore = Track::keys([
            (0.0, about_elbow(e, ELBOW.0)),
            (FREEZE, about_elbow(e, ELBOW.1)),
        ])
        .unwrap_or_else(|err| panic!("the curl's keys are valid: {err}"))
        .ease(Ease::Custom(into_the_freeze));
        r.outlined(|g| limb(s, e, 0.06, 0.045, g), azul(), body, &still);
        r.outlined(|g| limb(e, wrist, 0.043, 0.04, g), azul(), body, &fore);
        r.outlined(
            |g| Shape::sphere(hand, 0.06 + g, SIDES, 4),
            azul(),
            body,
            &fore,
        );
        let along = |d: f64| add(hand, [0.0, 0.0, d]);
        r.outlined(
            |g| Shape::prism(along(-0.11), along(0.11), 0.018 + g, 6),
            acero(),
            prop,
            &fore,
        );
        for sign in [1.0, -1.0] {
            r.outlined(
                |g| {
                    Shape::prism(
                        along(sign * (0.062 - g)),
                        along(sign * (0.115 + g)),
                        0.07 + g,
                        6,
                    )
                },
                acero(),
                prop,
                &fore,
            );
        }

        // Inside: the bones of the limbs, and the biceps along the front
        // of the upper arm.
        r.plain(
            Shape::capsule(ankle, knee, 0.02, 0.02, 6, 1),
            hueso(),
            &still,
        );
        r.plain(
            Shape::capsule(knee, hip, 0.024, 0.024, 6, 1),
            hueso(),
            &still,
        );
        r.plain(Shape::capsule(s, e, 0.02, 0.018, 6, 1), hueso(), &still);
        r.plain(Shape::capsule(e, wrist, 0.016, 0.014, 6, 1), hueso(), &fore);
        let origin = add(s, [0.05, -0.06, 0.0]);
        let insertion = add(e, [0.04, 0.05, 0.0]);
        r.plain(
            Shape::capsule(origin, insertion, 0.024, 0.016, 6, 1),
            biceps(),
            &still,
        );
    }

    // The pelvis in shorts, the chunky torso, the neck.
    r.outlined(
        |g| {
            Shape::capsule(
                [0.0, 0.97, -0.12],
                [0.0, 0.97, 0.12],
                0.14 + g,
                0.14 + g,
                10,
                BANDS,
            )
        },
        short(),
        body,
        &still,
    );
    let torso_c = [0.0, 1.17, 0.0];
    r.outlined(
        |g| {
            Shape::capsule(
                [0.0, 0.99, 0.0],
                [0.0, 1.33, 0.0],
                0.13 + g,
                0.16 + g,
                10,
                BANDS,
            )
            .scaled([0.95, 1.0, 1.4], torso_c)
        },
        azul(),
        body,
        &still,
    );
    r.outlined(
        |g| {
            Shape::capsule(
                [0.0, 1.40, 0.0],
                [0.01, 1.55, 0.0],
                0.06 + g,
                0.06 + g,
                SIDES,
                1,
            )
        },
        azul(),
        body,
        &still,
    );

    // The big head, its headband, two eyes looking forward.
    let c = [0.03, 1.68, 0.0];
    let rh = 0.145;
    r.outlined(|g| Shape::sphere(c, rh + g, 10, 5), azul(), body, &still);
    r.outlined(
        |g| {
            Shape::band(
                add(c, [0.0, 0.02, 0.0]),
                add(c, [0.0, 0.065, 0.0]),
                0.155 + g,
                10,
            )
        },
        amarillo(),
        body,
        &still,
    );
    for side in [1.0, -1.0] {
        let dir = normalize([0.93, -0.03, 0.36 * side]);
        let eye = add(c, scale(dir, 0.9 * rh));
        r.plain(
            Shape::sphere(eye, 0.036, SIDES, 4).scaled([0.45, 1.1, 1.0], eye),
            tenis(),
            &still,
        );
        let pupil = add(c, scale(dir, 0.99 * rh));
        r.plain(
            Shape::sphere(pupil, 0.018, 6, 3).scaled([0.5, 1.0, 1.0], eye),
            pupila(),
            &still,
        );
    }

    // Inside the trunk: the spine and the collarbones.
    r.plain(
        Shape::capsule([0.0, 0.95, 0.0], [0.0, 1.52, 0.0], 0.022, 0.02, 6, 1),
        hueso(),
        &still,
    );
    r.plain(
        Shape::capsule([0.0, 1.40, -0.22], [0.0, 1.40, 0.22], 0.014, 0.014, 6, 1),
        hueso(),
        &still,
    );
    r.objects
}
