//! «Pintura»: brush strokes that follow the surface and the light.
//!
//! Prior art: A. Hertzmann, «Painterly rendering with curved brush strokes
//! of multiple sizes», SIGGRAPH 1998. Taken from it: several brush sizes,
//! painted coarse to fine; strokes laid along the isophotes (perpendicular
//! to the light gradient) instead of across them; finer strokes only where
//! the coarse layer is a poor match. Departures, because this paints a
//! moving 3D scene rather than one photograph:
//!
//! - the strokes live **on the surface**, in the face's barycentric
//!   coordinates, seeded by the face id: they move with the toy and stay
//!   put from frame to frame (Hertzmann's strokes are re-placed per image,
//!   which flickers in video);
//! - the stroke count comes from the world-space area, not the screen
//!   area, so zooming or orbiting never adds or removes a stroke;
//! - "where the coarse layer is a poor match" has no reference image to
//!   compare against, so it is approximated by where shading changes
//!   fastest: the terminator and the silhouette;
//! - each face is underpainted flat first, so no background shows through
//!   gaps between strokes;
//! - a stroke fades out as its face turns edge-on, so faces crossing the
//!   silhouette during an orbit do not pop.

use motoreel::{Prim2, Pt2};

use crate::acabado::{fill_only, polygon, sealed, Acabado};
use crate::face::Face;
use crate::material::Layer;
use crate::vec3::{cross, dot, normalize, sub, V3};

/// One brush size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Brush {
    /// Stroke width at its widest, world units.
    pub width: f64,
    /// Stroke length, world units.
    pub length: f64,
    /// Strokes per brush footprint of surface (1 ≈ edge to edge).
    pub density: f64,
    /// How far a stroke's tone may stray along the material ramp.
    pub tone_jitter: f64,
    /// Stroke opacity.
    pub alpha: f64,
    /// Whether the density follows the "poor match" weight (fine layers)
    /// or is uniform (the coarse layer).
    pub refine: bool,
}

/// The painterly finish.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pintura {
    /// The fixed seed: the same seed paints the same strokes.
    pub seed: u64,
    /// Brushes, painted in order (coarse first).
    pub brushes: [Brush; 2],
    /// The underpainting's seam stroke, image units.
    pub seam: f64,
}

/// Samples along a stroke's centreline.
const SAMPLES: usize = 7;

impl Acabado for Pintura {
    fn paint(&self, face: &Face, out: &mut Vec<Prim2>) {
        if !matches!(face.material.layer, Layer::Body | Layer::Object) {
            return;
        }
        let tones = face.material.tones;
        out.extend(polygon(
            &face.image,
            sealed(tones.at(face.light - 0.08), 1.0, self.seam),
        ));
        let Some(frame) = StrokeFrame::of(face) else {
            return;
        };
        let fade = smoothstep(0.0, 0.35, face.facing);
        // Shading changes fastest at the terminator (k near the base) and
        // the silhouette (facing near 0): the stand-in for Hertzmann's
        // per-pixel error between the canvas and the reference.
        let detail = (1.0 - face.facing)
            .max(1.0 - 2.0 * (face.light - 0.55).abs())
            .clamp(0.0, 1.0);
        for (li, brush) in self.brushes.iter().enumerate() {
            let weight = if brush.refine { detail } else { 1.0 };
            // Hertzmann picks the largest brush that still fits the detail;
            // here a face smaller than the brush shrinks it (down to a
            // third), so a dumbbell head is not one smear.
            let fit = ((2.0 * face.area()).sqrt() / brush.length).clamp(0.35, 1.0);
            let size = (brush.width * fit, brush.length * fit);
            let expected = face.area() / (size.0 * size.1) * brush.density * weight;
            let mut rng = Rng::new(self.seed, face.id, li as u64);
            let n = (expected + rng.unit()).floor() as usize;
            for _ in 0..n {
                let stroke = Stroke::draw(&mut rng);
                let tone = tones.at(face.light + stroke.tone * brush.tone_jitter);
                if let Some(pts) = frame.ribbon(face, &stroke, size) {
                    out.extend(polygon(&pts, fill_only(tone, brush.alpha * fade)));
                }
            }
        }
    }
}

/// The face's stroke directions in barycentric terms: along the isophote
/// and across it, so a stroke laid in `(u, v)` lies on the surface.
struct StrokeFrame {
    along: (f64, f64),
    across: (f64, f64),
}

impl StrokeFrame {
    fn of(face: &Face) -> Option<StrokeFrame> {
        let [a, b, c] = face.world;
        let (e1, e2) = (sub(b, a), sub(c, a));
        // Isophote: the surface direction along which N·L stays constant.
        let along = normalize(cross(face.normal, face.light_dir)).or_else(|| normalize(e1))?;
        let across = cross(face.normal, along);
        Some(StrokeFrame {
            along: to_bary(along, e1, e2)?,
            across: to_bary(across, e1, e2)?,
        })
    }

    /// The stroke as a closed ribbon in image space, or `None` if any
    /// point is culled.
    fn ribbon(&self, face: &Face, s: &Stroke, (width, length): (f64, f64)) -> Option<Vec<Pt2>> {
        let (ca, sa) = (s.angle.cos(), s.angle.sin());
        let dir = (
            self.along.0 * ca + self.across.0 * sa,
            self.along.1 * ca + self.across.1 * sa,
        );
        let perp = (
            self.across.0 * ca - self.along.0 * sa,
            self.across.1 * ca - self.along.1 * sa,
        );
        let len = length * s.length;
        let mut left = Vec::with_capacity(SAMPLES);
        let mut right = Vec::with_capacity(SAMPLES);
        for i in 0..SAMPLES {
            let t = i as f64 / (SAMPLES - 1) as f64;
            let along = len * (t - 0.5);
            let bow = s.bend * len * 0.18 * (1.0 - (2.0 * t - 1.0).powi(2));
            let half = 0.5 * width * (0.35 + 0.65 * (std::f64::consts::PI * t).sin());
            let at = |side: f64| {
                let k = bow + side * half;
                (
                    s.u + dir.0 * along + perp.0 * k,
                    s.v + dir.1 * along + perp.1 * k,
                )
            };
            let (lu, lv) = at(1.0);
            let (ru, rv) = at(-1.0);
            left.push(face.project(face.view_at(lu, lv))?);
            right.push(face.project(face.view_at(ru, rv))?);
        }
        right.reverse();
        left.extend(right);
        Some(left)
    }
}

/// A world direction `d` in the plane of edges `e1`, `e2`, as barycentric
/// increments `(α, β)` with `d = α·e1 + β·e2`.
fn to_bary(d: V3, e1: V3, e2: V3) -> Option<(f64, f64)> {
    let (a, b, c) = (dot(e1, e1), dot(e1, e2), dot(e2, e2));
    let det = a * c - b * b;
    if !det.is_finite() || det.abs() <= 1e-18 {
        return None;
    }
    let (p, q) = (dot(d, e1), dot(d, e2));
    Some(((c * p - b * q) / det, (a * q - b * p) / det))
}

/// One stroke's random choices, all from the face's own stream.
struct Stroke {
    u: f64,
    v: f64,
    angle: f64,
    bend: f64,
    length: f64,
    tone: f64,
}

impl Stroke {
    fn draw(rng: &mut Rng) -> Stroke {
        // Uniform in the triangle: fold the unit square's far half back.
        let (mut u, mut v) = (rng.unit(), rng.unit());
        if u + v > 1.0 {
            (u, v) = (1.0 - u, 1.0 - v);
        }
        Stroke {
            u,
            v,
            angle: 0.3 * rng.signed(),
            bend: rng.signed(),
            length: 0.7 + 0.6 * rng.unit(),
            tone: rng.signed(),
        }
    }
}

/// `0` below `a`, `1` above `b`, smooth between.
fn smoothstep(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// SplitMix64 (Steele, Lea & Flood 2014, the `java.util.SplittableRandom`
/// mixer): one small stream per (seed, face, brush), so a face's strokes do
/// not depend on which other faces are visible this frame.
struct Rng(u64);

impl Rng {
    fn new(seed: u64, face: u64, brush: u64) -> Rng {
        let mut r = Rng(seed
            ^ face.wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ brush.wrapping_mul(0xD1B5_4A32_D192_ED03));
        r.next();
        r
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `[0, 1)`.
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Uniform in `[−1, 1)`.
    fn signed(&mut self) -> f64 {
        2.0 * self.unit() - 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bary_round_trips() {
        let (e1, e2) = ([2.0, 0.0, 0.0], [1.0, 1.0, 0.0]);
        let (a, b) = to_bary([0.0, 3.0, 0.0], e1, e2).unwrap();
        assert!((a * 2.0 + b - 0.0).abs() < 1e-12 && (b - 3.0).abs() < 1e-12);
        assert!(to_bary([1.0, 0.0, 0.0], e1, e1).is_none());
    }

    #[test]
    fn the_stream_is_fixed_by_its_seed() {
        let a: Vec<u64> = (0..4)
            .map({
                let mut r = Rng::new(7, 3, 1);
                move |_| r.next()
            })
            .collect();
        let b: Vec<u64> = (0..4)
            .map({
                let mut r = Rng::new(7, 3, 1);
                move |_| r.next()
            })
            .collect();
        let c: Vec<u64> = (0..4)
            .map({
                let mut r = Rng::new(7, 4, 1);
                move |_| r.next()
            })
            .collect();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
