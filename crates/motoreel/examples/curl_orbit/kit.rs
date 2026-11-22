//! Low-poly generators: the kit's job, not motoreel's (R-0018 OQ-4).
//!
//! Each builds a [`Mesh`] of planar convex faces: quads between coaxial
//! rings, triangle fans at the poles, n-gon caps. Every face is oriented
//! outward by one rule, valid because every solid here is convex: its
//! normal points away from the solid's vertex centroid. `hull` reverses
//! the winding, so back-face culling keeps the **far** half: the ink
//! outline drawn behind a part (the inverted hull).

use std::f64::consts::{FRAC_PI_2, TAU};

use garust::pga;
use motoreel::{Material, Mesh, MeshFace};

/// A point or direction, `[x, y, z]`, y up.
pub type V3 = [f64; 3];

pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn scale(a: V3, s: f64) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub fn normalize(a: V3) -> V3 {
    let n = dot(a, a).sqrt();
    scale(a, 1.0 / n)
}

pub fn lerp(a: V3, b: V3, t: f64) -> V3 {
    add(a, scale(sub(b, a), t))
}

/// Unscaled geometry: vertices and faces, before a material and validation.
#[derive(Clone, Debug)]
pub struct Shape {
    verts: Vec<V3>,
    faces: Vec<Vec<usize>>,
}

/// Two unit vectors perpendicular to the unit `w` and to each other.
fn frame(w: V3) -> (V3, V3) {
    let helper = if w[1].abs() < 0.9 {
        [0.0, 1.0, 0.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let u = normalize(cross(w, helper));
    (u, cross(w, u))
}

impl Shape {
    /// A solid of revolution about `a → b`: `rings` are `(fraction along
    /// the axis beyond a, radius)` pairs from the `a` end to the `b` end,
    /// closed by a pole at each end. `sides` facets around.
    fn lathe(a: V3, b: V3, pole_a: V3, pole_b: V3, rings: &[(V3, f64)], sides: usize) -> Shape {
        let w = normalize(sub(b, a));
        let (u, v) = frame(w);
        let mut verts = vec![pole_a];
        for &(c, r) in rings {
            for k in 0..sides {
                let th = TAU * k as f64 / sides as f64;
                verts.push(add(c, add(scale(u, r * th.cos()), scale(v, r * th.sin()))));
            }
        }
        verts.push(pole_b);
        let top = verts.len() - 1;
        let at = |ring: usize, k: usize| 1 + ring * sides + k % sides;
        let mut faces = Vec::new();
        for k in 0..sides {
            faces.push(vec![0, at(0, k + 1), at(0, k)]);
            faces.push(vec![
                top,
                at(rings.len() - 1, k),
                at(rings.len() - 1, k + 1),
            ]);
        }
        for ring in 0..rings.len() - 1 {
            for k in 0..sides {
                faces.push(vec![
                    at(ring, k),
                    at(ring, k + 1),
                    at(ring + 1, k + 1),
                    at(ring + 1, k),
                ]);
            }
        }
        Shape { verts, faces }.oriented()
    }

    /// A capsule from `a` (radius `ra`) to `b` (radius `rb`): a
    /// hemisphere of `bands` latitude bands at each end and cone rings
    /// about one radius apart, so faces stay short for the depth sort.
    pub fn capsule(a: V3, b: V3, ra: f64, rb: f64, sides: usize, bands: usize) -> Shape {
        let w = normalize(sub(b, a));
        let mut rings = Vec::new();
        for i in 1..=bands {
            let lat = -FRAC_PI_2 + FRAC_PI_2 * i as f64 / bands as f64;
            rings.push((add(a, scale(w, ra * lat.sin())), ra * lat.cos()));
        }
        let len = dot(sub(b, a), sub(b, a)).sqrt();
        let mid = (len / (1.2 * ra.max(rb))).floor() as usize;
        for j in 1..=mid {
            let f = j as f64 / (mid + 1) as f64;
            rings.push((lerp(a, b, f), ra + (rb - ra) * f));
        }
        for i in 0..bands {
            let lat = FRAC_PI_2 * i as f64 / bands as f64;
            rings.push((add(b, scale(w, rb * lat.sin())), rb * lat.cos()));
        }
        Shape::lathe(
            a,
            b,
            sub(a, scale(w, ra)),
            add(b, scale(w, rb)),
            &rings,
            sides,
        )
    }

    /// A low-poly sphere about `c`, its axis vertical: `bands` latitude
    /// bands from pole to pole.
    pub fn sphere(c: V3, r: f64, sides: usize, bands: usize) -> Shape {
        let up = [0.0, 1.0, 0.0];
        let rings: Vec<(V3, f64)> = (1..bands)
            .map(|i| {
                let lat = -FRAC_PI_2 + std::f64::consts::PI * i as f64 / bands as f64;
                (add(c, scale(up, r * lat.sin())), r * lat.cos())
            })
            .collect();
        Shape::lathe(
            c,
            add(c, up),
            sub(c, scale(up, r)),
            add(c, scale(up, r)),
            &rings,
            sides,
        )
    }

    /// A closed prism from `a` to `b`, circumradius `r` (6 sides: a hex
    /// dumbbell head).
    pub fn prism(a: V3, b: V3, r: f64, sides: usize) -> Shape {
        let w = normalize(sub(b, a));
        let (u, v) = frame(w);
        let mut verts = Vec::new();
        for end in [a, b] {
            for k in 0..sides {
                let th = TAU * k as f64 / sides as f64;
                verts.push(add(
                    end,
                    add(scale(u, r * th.cos()), scale(v, r * th.sin())),
                ));
            }
        }
        let mut faces = vec![(0..sides).collect::<Vec<_>>(), (sides..2 * sides).collect()];
        for k in 0..sides {
            let n = (k + 1) % sides;
            faces.push(vec![k, n, sides + n, sides + k]);
        }
        Shape { verts, faces }.oriented()
    }

    /// [`Shape::prism`] without its caps: an open band (a headband).
    pub fn band(a: V3, b: V3, r: f64, sides: usize) -> Shape {
        let mut s = Shape::prism(a, b, r, sides);
        s.faces.drain(..2);
        s
    }

    /// An axis-aligned box from `lo` to `hi`.
    pub fn cuboid(lo: V3, hi: V3) -> Shape {
        let verts = (0..8)
            .map(|i| {
                [
                    if i & 1 == 0 { lo[0] } else { hi[0] },
                    if i & 2 == 0 { lo[1] } else { hi[1] },
                    if i & 4 == 0 { lo[2] } else { hi[2] },
                ]
            })
            .collect();
        let faces = [
            [0, 1, 3, 2],
            [4, 6, 7, 5],
            [0, 4, 5, 1],
            [2, 3, 7, 6],
            [0, 2, 6, 4],
            [1, 5, 7, 3],
        ]
        .iter()
        .map(|q| q.to_vec())
        .collect();
        Shape { verts, faces }.oriented()
    }

    /// Scaled by `s` per axis about `about` (an affine map: faces stay
    /// planar and convex).
    pub fn scaled(mut self, s: V3, about: V3) -> Shape {
        for p in &mut self.verts {
            let d = sub(*p, about);
            *p = add(about, [d[0] * s[0], d[1] * s[1], d[2] * s[2]]);
        }
        self
    }

    /// Every face turned inside out: the inverted hull.
    pub fn hull(mut self) -> Shape {
        for f in &mut self.faces {
            f.reverse();
        }
        self
    }

    /// The validated mesh, every face of `material`.
    pub fn mesh(self, material: Material) -> Mesh {
        let vertices = self
            .verts
            .iter()
            .map(|p| pga::Point::new(p[0], p[1], p[2]))
            .collect();
        let faces = self
            .faces
            .into_iter()
            .map(|f| MeshFace::new(f, 0))
            .collect();
        Mesh::new(vertices, faces, vec![material])
            .unwrap_or_else(|e| panic!("a kit mesh is valid: {e}"))
    }

    /// Faces turned outward (see the module docs).
    fn oriented(mut self) -> Shape {
        let centre = scale(
            self.verts.iter().fold([0.0; 3], |acc, p| add(acc, *p)),
            1.0 / self.verts.len() as f64,
        );
        for f in &mut self.faces {
            let (p0, p1, p2) = (self.verts[f[0]], self.verts[f[1]], self.verts[f[2]]);
            let n = cross(sub(p1, p0), sub(p2, p0));
            if dot(n, sub(p0, centre)) < 0.0 {
                f.reverse();
            }
        }
        self
    }
}
