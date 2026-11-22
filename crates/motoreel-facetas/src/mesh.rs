//! Low-poly meshes as triangle lists: the shapes the toy is built from.
//!
//! Every builder makes a **convex** closed solid (a capsule, a sphere, a
//! prism, a box), which lets one rule orient every triangle outward: its
//! normal points away from the solid's vertex centroid. The facet count is
//! the look, so it is the caller's choice and small on purpose.

use std::f64::consts::{FRAC_PI_2, TAU};

use crate::vec3::{add, cross, dot, lerp, normalize, scale, sub, V3};

/// A closed triangle mesh, counter-clockwise seen from outside.
#[derive(Clone, Debug, PartialEq)]
pub struct Mesh {
    /// Vertex positions, model space.
    pub verts: Vec<V3>,
    /// Vertex indices, three per triangle, oriented outward.
    pub tris: Vec<[usize; 3]>,
}

/// Two unit vectors perpendicular to the unit `w` and to each other.
fn frame(w: V3) -> (V3, V3) {
    let helper = if w[1].abs() < 0.9 {
        [0.0, 1.0, 0.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    // `w` is unit and `helper` is chosen non-parallel, so both are non-zero.
    let u = normalize(cross(w, helper)).unwrap_or([1.0, 0.0, 0.0]);
    (u, cross(w, u))
}

impl Mesh {
    /// Cone rings of a capsule are spaced about this many radii apart.
    pub const CONE_STEP: f64 = 1.2;

    /// A capsule from `a` (radius `ra`) to `b` (radius `rb`): a hemisphere
    /// at each end joined by a cone, `sides` facets around and `rings`
    /// latitude bands per hemisphere. `a == b` degenerates to a sphere of
    /// radius `rb` around an arbitrary axis.
    #[must_use]
    pub fn capsule(a: V3, b: V3, ra: f64, rb: f64, sides: usize, rings: usize) -> Mesh {
        let sides = sides.max(3);
        let rings = rings.max(1);
        let w = normalize(sub(b, a)).unwrap_or([0.0, 1.0, 0.0]);
        let (u, v) = frame(w);
        let ring = |c: V3, r: f64, lat: f64, twist: f64| -> Vec<V3> {
            (0..sides)
                .map(|k| {
                    let th = TAU * (k as f64 + twist) / sides as f64;
                    let radial = add(scale(u, th.cos()), scale(v, th.sin()));
                    add(
                        c,
                        add(scale(radial, r * lat.cos()), scale(w, r * lat.sin())),
                    )
                })
                .collect()
        };
        // Latitudes from the `a` pole (exclusive) to the `b` pole (exclusive).
        let mut bands: Vec<Vec<V3>> = Vec::new();
        for i in 1..=rings {
            let lat = -FRAC_PI_2 + FRAC_PI_2 * i as f64 / rings as f64;
            bands.push(ring(a, ra, lat, 0.0));
        }
        // Rings along the cone too, about one radius apart: a long thin
        // face sorts by a centroid far from where it overlaps its
        // neighbours, which is the painter's sort's worst case.
        let len = crate::vec3::length(sub(b, a));
        let mid = (len / (Self::CONE_STEP * ra.max(rb))).floor() as usize;
        for j in 1..=mid {
            let f = j as f64 / (mid + 1) as f64;
            bands.push(ring(lerp(a, b, f), ra + (rb - ra) * f, 0.0, 0.0));
        }
        for i in 0..rings {
            let lat = FRAC_PI_2 * i as f64 / rings as f64;
            bands.push(ring(b, rb, lat, 0.0));
        }
        let mut verts = vec![sub(a, scale(w, ra))];
        for band in &bands {
            verts.extend_from_slice(band);
        }
        verts.push(add(b, scale(w, rb)));
        let top = verts.len() - 1;
        let at = |band: usize, k: usize| 1 + band * sides + k % sides;
        let mut tris = Vec::new();
        for k in 0..sides {
            tris.push([0, at(0, k + 1), at(0, k)]);
            tris.push([top, at(bands.len() - 1, k), at(bands.len() - 1, k + 1)]);
        }
        for band in 0..bands.len() - 1 {
            for k in 0..sides {
                tris.push([at(band, k), at(band, k + 1), at(band + 1, k + 1)]);
                tris.push([at(band, k), at(band + 1, k + 1), at(band + 1, k)]);
            }
        }
        Mesh { verts, tris }.oriented()
    }

    /// A low-poly sphere: a capsule of zero length.
    #[must_use]
    pub fn sphere(c: V3, r: f64, sides: usize, rings: usize) -> Mesh {
        Mesh::capsule(c, add(c, [0.0, 1e-9, 0.0]), r, r, sides, rings)
    }

    /// A closed prism (`sides` = 6 is a hex dumbbell head) from `a` to `b`
    /// of circumradius `r`.
    #[must_use]
    pub fn prism(a: V3, b: V3, r: f64, sides: usize) -> Mesh {
        let sides = sides.max(3);
        let w = normalize(sub(b, a)).unwrap_or([0.0, 1.0, 0.0]);
        let (u, v) = frame(w);
        let mut verts = vec![a, b];
        for end in [a, b] {
            for k in 0..sides {
                let th = TAU * k as f64 / sides as f64;
                verts.push(add(
                    end,
                    add(scale(u, r * th.cos()), scale(v, r * th.sin())),
                ));
            }
        }
        let at = |end: usize, k: usize| 2 + end * sides + k % sides;
        let mut tris = Vec::new();
        for k in 0..sides {
            tris.push([0, at(0, k + 1), at(0, k)]);
            tris.push([1, at(1, k), at(1, k + 1)]);
            tris.push([at(0, k), at(0, k + 1), at(1, k + 1)]);
            tris.push([at(0, k), at(1, k + 1), at(1, k)]);
        }
        Mesh { verts, tris }.oriented()
    }

    /// [`Mesh::prism`] without its two caps: an open band (a headband).
    #[must_use]
    pub fn band(a: V3, b: V3, r: f64, sides: usize) -> Mesh {
        let mut m = Mesh::prism(a, b, r, sides);
        // Vertices 0 and 1 are the cap centres; every cap triangle uses one.
        m.tris.retain(|t| t.iter().all(|&i| i > 1));
        m
    }

    /// An axis-aligned box between corners `lo` and `hi`.
    #[must_use]
    pub fn cuboid(lo: V3, hi: V3) -> Mesh {
        let verts: Vec<V3> = (0..8)
            .map(|i| {
                [
                    if i & 1 == 0 { lo[0] } else { hi[0] },
                    if i & 2 == 0 { lo[1] } else { hi[1] },
                    if i & 4 == 0 { lo[2] } else { hi[2] },
                ]
            })
            .collect();
        let quads = [
            [0, 1, 3, 2],
            [4, 6, 7, 5],
            [0, 4, 5, 1],
            [2, 3, 7, 6],
            [0, 2, 6, 4],
            [1, 5, 7, 3],
        ];
        let tris = quads
            .iter()
            .flat_map(|q| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]])
            .collect();
        Mesh { verts, tris }.oriented()
    }

    /// The mesh scaled by `s` per axis about `about` (a chunky torso is a
    /// capsule widened sideways).
    #[must_use]
    pub fn scaled(mut self, s: V3, about: V3) -> Mesh {
        for p in &mut self.verts {
            let d = sub(*p, about);
            *p = add(about, [d[0] * s[0], d[1] * s[1], d[2] * s[2]]);
        }
        self
    }

    /// Flips every triangle whose normal points toward the vertex centroid,
    /// and drops degenerate ones. Valid because every builder is convex.
    fn oriented(mut self) -> Mesh {
        let n = self.verts.len() as f64;
        let centre = scale(
            self.verts.iter().fold([0.0; 3], |acc, p| add(acc, *p)),
            1.0 / n,
        );
        let verts = &self.verts;
        self.tris.retain_mut(|t| {
            let (p0, p1, p2) = (verts[t[0]], verts[t[1]], verts[t[2]]);
            let Some(normal) = normalize(cross(sub(p1, p0), sub(p2, p0))) else {
                return false;
            };
            let mid = lerp(p0, lerp(p1, p2, 0.5), 2.0 / 3.0);
            if dot(normal, sub(mid, centre)) < 0.0 {
                t.swap(1, 2);
            }
            true
        });
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every triangle of every builder faces away from the centre.
    fn outward(m: &Mesh) -> bool {
        let n = m.verts.len() as f64;
        let c = scale(m.verts.iter().fold([0.0; 3], |a, p| add(a, *p)), 1.0 / n);
        m.tris.iter().all(|t| {
            let (p0, p1, p2) = (m.verts[t[0]], m.verts[t[1]], m.verts[t[2]]);
            dot(cross(sub(p1, p0), sub(p2, p0)), sub(p0, c)) > 0.0
        })
    }

    #[test]
    fn builders_are_closed_and_outward() {
        let cap = Mesh::capsule([0.0; 3], [0.0, 1.0, 0.0], 0.2, 0.1, 8, 2);
        // 2 + 2 cap bands and floor(1 / (1.2 · 0.2)) = 4 cone rings: 8 bands.
        assert_eq!(cap.tris.len(), 2 * 8 + 2 * 8 * 7);
        assert!(outward(&cap));
        assert!(outward(&Mesh::prism([0.0; 3], [0.0, 0.0, 1.0], 0.5, 6)));
        let b = Mesh::cuboid([0.0; 3], [1.0, 2.0, 3.0]);
        assert_eq!(b.tris.len(), 12);
        assert!(outward(&b));
        assert!(outward(&Mesh::sphere([1.0, 2.0, 3.0], 0.5, 10, 3)));
    }

    #[test]
    fn a_flat_box_loses_its_degenerate_faces() {
        assert!(Mesh::cuboid([0.0; 3], [1.0, 0.0, 1.0]).tris.len() < 12);
    }
}
