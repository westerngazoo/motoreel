//! «Barrido»: two finishes split by a vertical line in the image, the
//! house's x-ray sweep. Left of the line one finish, right of it the other.
//!
//! Each face's image polygon is clipped to each half-plane
//! (Sutherland–Hodgman, one plane, so the convex case is the whole
//! algorithm). Only finishes that paint [`Face::image`] clip cleanly:
//! [`crate::Facetas`] and [`crate::RayosX`]. A painted face's strokes are
//! placed from the 3D triangle and would ignore the line.

use motoreel::{Prim2, Pt2};

use crate::acabado::Acabado;
use crate::face::Face;

/// `left` where `x < line`, `right` where `x ≥ line` (image space).
pub struct Barrido<'a> {
    /// The finish left of the line.
    pub left: &'a dyn Acabado,
    /// The finish right of the line.
    pub right: &'a dyn Acabado,
    /// The line's image-space x.
    pub line: f64,
}

impl Acabado for Barrido<'_> {
    fn paint(&self, face: &Face, out: &mut Vec<Prim2>) {
        for (finish, keep_left) in [(self.left, true), (self.right, false)] {
            let image = clip(&face.image, self.line, keep_left);
            if image.len() >= 3 {
                let part = Face {
                    image,
                    ..face.clone()
                };
                finish.paint(&part, out);
            }
        }
    }
}

/// The part of polygon `pts` on one side of the vertical line `x = line`.
#[must_use]
pub fn clip(pts: &[Pt2], line: f64, keep_left: bool) -> Vec<Pt2> {
    let inside = |p: &Pt2| if keep_left { p.x < line } else { p.x >= line };
    let mut out = Vec::with_capacity(pts.len() + 2);
    for (i, p) in pts.iter().enumerate() {
        let q = &pts[(i + 1) % pts.len()];
        if inside(p) {
            out.push(*p);
        }
        if inside(p) != inside(q) {
            let t = (line - p.x) / (q.x - p.x);
            out.push(Pt2 {
                x: line,
                y: p.y + (q.y - p.y) * t,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_triangle_splits_into_two_pieces() {
        let tri = [
            Pt2 { x: 0.0, y: 0.0 },
            Pt2 { x: 2.0, y: 0.0 },
            Pt2 { x: 0.0, y: 2.0 },
        ];
        let l = clip(&tri, 1.0, true);
        let r = clip(&tri, 1.0, false);
        assert_eq!(l.len(), 4);
        assert_eq!(r.len(), 3);
        assert!(clip(&tri, -1.0, true).is_empty());
    }
}
