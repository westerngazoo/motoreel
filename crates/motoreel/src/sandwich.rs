//! A motor applied to a point, bit for bit what
//! [`pga::Point::transform`] gives, at under half its cost (R-0022,
//! SPEC-0022 §2.3).
//!
//! `transform` is garust's sandwich `M · p · M̃`: two geometric products
//! through the algebra's Cayley table, each walking all 16 × 16 blade
//! pairs and skipping the zero coefficients (garust-core
//! `transform::sparse_product`, "bit-identical to `*`"). A motor can only
//! be non-zero on its 8 even blades, a point on its 4 trivectors, and
//! their product on its 8 odd blades. [`Sandwich`] walks just those, in
//! the same ascending order, with the same zero tests and garust's own
//! table, so every coefficient receives the same terms in the same order
//! and rounds the same way. Anything off those blades (a hand-built
//! motor or point) goes through `transform` itself.
//!
//! The faces of a mesh are posed this way once per vertex per frame, which
//! made the sandwich a quarter of a frame's geometry once the depth order
//! stopped dominating it. A klein-style closed form (garust's
//! `apply_point_fast`) is faster still but rounds differently, and would
//! move every pixel of every 3D golden.

use garust::{pga, Algebra, Motor3, Pga3Sig};

/// The blades a motor may be non-zero on: grades 0, 2 and 4.
const EVEN: [usize; 8] = [
    0b0000, 0b0011, 0b0101, 0b0110, 0b1001, 0b1010, 0b1100, 0b1111,
];
/// The blades a point may be non-zero on: grade 3.
const POINT: [usize; 4] = [0b0111, 0b1011, 0b1101, 0b1110];
/// The blades `M · p` may be non-zero on: grades 1 and 3.
const ODD: [usize; 8] = [
    0b0001, 0b0010, 0b0100, 0b0111, 0b1000, 0b1011, 0b1101, 0b1110,
];

/// One motor, ready to move many points.
pub(crate) struct Sandwich {
    motor: Motor3,
    /// The versor's coefficients and its reverse's.
    m: [f64; 16],
    rev: [f64; 16],
    /// Whether the versor is zero off its even blades.
    even: bool,
}

impl Sandwich {
    pub(crate) fn new(motor: &Motor3) -> Sandwich {
        let versor = motor.versor();
        let m = versor.coeffs;
        Sandwich {
            motor: *motor,
            m,
            rev: versor.reverse().coeffs,
            even: (0..16).all(|i| EVEN.contains(&i) || m[i] == 0.0),
        }
    }

    /// `p.transform(&motor).to_euclidean()`, bit for bit.
    pub(crate) fn apply(&self, p: &pga::Point) -> (f64, f64, f64) {
        let c = p.multivector().coeffs;
        if !(self.even && (0..16).all(|i| POINT.contains(&i) || c[i] == 0.0)) {
            return p.transform(&self.motor).to_euclidean();
        }
        let mp = product(&self.m, &EVEN, &c, &POINT);
        let out = product(&mp, &ODD, &self.rev, &EVEN);
        // `pga::Point::to_euclidean`, verbatim.
        let w = out[0b0111];
        (-out[0b1110] / w, out[0b1101] / w, -out[0b1011] / w)
    }
}

/// The geometric product of `a` and `b`, given that `a` is zero off the
/// blades `ai` and `b` off `bi` (both ascending): garust's sparse product
/// with the pairs that can only be zero left out.
fn product(a: &[f64; 16], ai: &[usize], b: &[f64; 16], bi: &[usize]) -> [f64; 16] {
    let table = <Pga3Sig as Algebra>::CAYLEY;
    let mut out = [0.0; 16];
    for &i in ai {
        let ca = a[i];
        if ca == 0.0 {
            continue;
        }
        for &j in bi {
            let cb = b[j];
            if cb == 0.0 {
                continue;
            }
            let (idx, sign) = table[i * 16 + j];
            let term = ca * cb;
            if sign > 0 {
                out[usize::from(idx)] += term;
            } else if sign < 0 {
                out[usize::from(idx)] -= term;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use garust::Pga3;
    use proptest::prelude::*;

    fn bits((x, y, z): (f64, f64, f64)) -> [u64; 3] {
        [x.to_bits(), y.to_bits(), z.to_bits()]
    }

    fn motor(q: [f64; 4], t: [f64; 3], u: [f64; 3]) -> Option<Motor3> {
        let n = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
        (n > 1e-3).then(|| {
            Motor3::translator(t[0], t[1], t[2])
                .compose(&Motor3::from_unit_quaternion(
                    q[0] / n,
                    q[1] / n,
                    q[2] / n,
                    q[3] / n,
                ))
                .compose(&Motor3::translator(u[0], u[1], u[2]))
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(2000))]

        /// Composed motors, as the scene builds them, on points anywhere.
        #[test]
        fn a_posed_point_is_bit_identical_to_transform(
            q in prop::array::uniform4(-1.0..1.0_f64),
            t in prop::array::uniform3(-50.0..50.0_f64),
            u in prop::array::uniform3(-5.0..5.0_f64),
            p in prop::array::uniform3(-100.0..100.0_f64),
        ) {
            let Some(m) = motor(q, t, u) else { return Ok(()) };
            let point = pga::Point::new(p[0], p[1], p[2]);
            prop_assert_eq!(
                bits(Sandwich::new(&m).apply(&point)),
                bits(point.transform(&m).to_euclidean())
            );
        }
    }

    /// Zero coefficients are skipped exactly where garust skips them: the
    /// identity, pure translations and axis turns have many.
    #[test]
    fn motors_with_zero_blades_agree() {
        let s = std::f64::consts::FRAC_1_SQRT_2;
        let motors = [
            Motor3::identity(),
            Motor3::translator(1.5, 0.0, -2.0),
            Motor3::from_unit_quaternion(s, 0.0, s, 0.0),
            Motor3::from_unit_quaternion(0.0, 1.0, 0.0, 0.0),
            Motor3::translator(0.0, 0.0, 4.6)
                .compose(&Motor3::from_unit_quaternion(s, s, 0.0, 0.0)),
        ];
        for m in &motors {
            for p in [[0.0, 0.0, 0.0], [-0.0, 1.0, -2.5], [0.3, -0.7, 1e-12]] {
                let point = pga::Point::new(p[0], p[1], p[2]);
                assert_eq!(
                    bits(Sandwich::new(m).apply(&point)),
                    bits(point.transform(m).to_euclidean())
                );
            }
        }
    }

    /// A point or a motor with a coefficient off its blades takes garust's
    /// path, so it still agrees.
    #[test]
    fn anything_off_the_blades_falls_back() {
        let odd = Pga3::basis(0b0001) * 0.25;
        let point =
            pga::Point::from_multivector(pga::Point::new(1.0, 2.0, 3.0).multivector() + odd);
        let m = Motor3::translator(0.5, -1.0, 2.0);
        assert_eq!(
            bits(Sandwich::new(&m).apply(&point)),
            bits(point.transform(&m).to_euclidean())
        );
        let skewed = Motor3::from_versor(m.versor() + odd);
        let plain = pga::Point::new(1.0, 2.0, 3.0);
        assert_eq!(
            bits(Sandwich::new(&skewed).apply(&plain)),
            bits(plain.transform(&skewed).to_euclidean())
        );
    }
}
