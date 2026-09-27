//! Shape generators: pure functions from a few numbers to a path
//! (SPEC-0012 §2.9).
//!
//! Each returns `Vec<Subpath<Pt2>>` in planar model coordinates, ready for
//! [`crate::Object::planar`] — and an empty `Vec` for degenerate input
//! rather than a panic: a non-finite argument, a radius, width or height
//! `≤ 0`, a zero sweep, or fewer than three distinct polygon vertices.
//!
//! **Orientation** (owner decision, 2026-09-27): [`circle`], [`arc`],
//! [`sector`] and [`rounded_rect`] are always counter-clockwise in y-up
//! coordinates — a negative sweep describes the same region and is
//! traversed counter-clockwise — so [`Subpath::reversed`] on any of them
//! reliably makes a hole under the nonzero rule. [`polygon`] keeps the
//! caller's vertex order, the natural way to author a hole directly.
//!
//! **No zero-length sides.** A side that vanishes (a rounded rectangle's
//! straight side at the largest corner radius, a repeated polygon vertex)
//! is omitted, not emitted at zero length.
//!
//! [`circle`] and [`rounded_rect`] are trig-free: their corners are
//! quarter-circle cubics with the handle constant `4(√2 − 1)/3`, and `√`
//! is correctly rounded, so golden fixtures built from them are
//! bit-portable. [`arc`] and [`sector`] call `cos`, `sin` and `tan` at
//! scene construction — trig lives in the scene, R-0003's caveat.

use std::f64::consts::{SQRT_2, TAU};

use crate::path::{Seg, Subpath};
use crate::prim::Pt2;

/// The quarter-circle cubic's handle length per unit radius,
/// `4(√2 − 1)/3 ≈ 0.5522847498307936`. Evaluated at compile time from the
/// correctly rounded `SQRT_2`; four such quarters stray at most
/// `2.73 × 10⁻⁴ R` outside the true circle, and never inside it.
const K: f64 = 4.0 * (SQRT_2 - 1.0) / 3.0;

/// A circle of radius `r` about `c`: one closed subpath of four
/// quarter-circle cubics, starting at `(c.x + r, c.y)`. Trig-free.
pub fn circle(c: Pt2, r: f64) -> Vec<Subpath<Pt2>> {
    if !(finite(c) && r.is_finite() && r > 0.0) {
        return Vec::new();
    }
    let k = K * r;
    let east = at(c, r, 0.0);
    vec![Subpath::new(east)
        .cubic_to(at(c, r, k), at(c, k, r), at(c, 0.0, r))
        .cubic_to(at(c, -k, r), at(c, -r, k), at(c, -r, 0.0))
        .cubic_to(at(c, -r, -k), at(c, -k, -r), at(c, 0.0, -r))
        .cubic_to(at(c, k, -r), at(c, r, -k), east)
        .close()]
}

/// An open arc of radius `r` about `c`, covering the angles from `start`
/// to `start + sweep` (radians, counter-clockwise from +x).
///
/// One cubic per started quarter turn, each with handle
/// `4/3 · tan(θ/4) · r`; end points lie on the circle up to the rounding
/// of `cos` and `sin`. `|sweep|` is clamped to one full turn, and a
/// negative sweep is traversed counter-clockwise, from `start + sweep` to
/// `start`.
pub fn arc(c: Pt2, r: f64, start: f64, sweep: f64) -> Vec<Subpath<Pt2>> {
    let valid = finite(c) && r.is_finite() && r > 0.0 && start.is_finite() && sweep.is_finite();
    if !valid || sweep == 0.0 {
        return Vec::new();
    }
    let sweep = sweep.clamp(-TAU, TAU);
    // Counter-clockwise from the lower angle to the upper one.
    let (from, to) = if sweep > 0.0 {
        (start, start + sweep)
    } else {
        (start + sweep, start)
    };
    // At least one piece, even for a sweep too small to register.
    let pieces = (sweep.abs() / (TAU / 4.0)).ceil().max(1.0);
    let theta = sweep.abs() / pieces;
    let handle = 4.0 / 3.0 * (theta / 4.0).tan() * r;
    let on_circle = |angle: f64| at(c, r * angle.cos(), r * angle.sin());
    // The tangent at `angle`, scaled to the handle length.
    let tangent = |angle: f64| (-handle * angle.sin(), handle * angle.cos());

    let mut sub = Subpath::new(on_circle(from));
    let count = pieces as u32; // 1 to 4
    for i in 0..count {
        let a0 = from + theta * f64::from(i);
        // The last piece ends exactly on `to`, not on an accumulated angle.
        let a1 = if i + 1 == count {
            to
        } else {
            from + theta * f64::from(i + 1)
        };
        let (p0, p1) = (on_circle(a0), on_circle(a1));
        let (t0, t1) = (tangent(a0), tangent(a1));
        sub = sub.cubic_to(
            Pt2 {
                x: p0.x + t0.0,
                y: p0.y + t0.1,
            },
            Pt2 {
                x: p1.x - t1.0,
                y: p1.y - t1.1,
            },
            p1,
        );
    }
    vec![sub]
}

/// A closed pie slice: from the centre `c`, a line to the start of
/// [`arc`]`(c, r, start, sweep)`, that arc, and the implicit close back to
/// `c`. Counter-clockwise for either sign of `sweep`.
pub fn sector(c: Pt2, r: f64, start: f64, sweep: f64) -> Vec<Subpath<Pt2>> {
    let Some(rim) = arc(c, r, start, sweep).pop() else {
        return Vec::new();
    };
    let mut segs = Vec::with_capacity(rim.segs.len() + 1);
    segs.push(Seg::Line(rim.start));
    segs.extend(rim.segs);
    vec![Subpath {
        start: c,
        segs,
        closed: true,
    }]
}

/// A `w × h` rectangle centred on `c` with corners rounded to radius
/// `rad`, clamped to `[0, min(w, h)/2]`. Trig-free.
///
/// Every variant starts at the bottom of the left side and leaves that
/// side to the implicit close. At `rad = 0` it is a plain rectangle —
/// exactly three lines and the close. Otherwise each corner is a
/// quarter-circle cubic with handle `K·rad`, and straight sides that
/// vanish at the largest radius are omitted.
pub fn rounded_rect(c: Pt2, w: f64, h: f64, rad: f64) -> Vec<Subpath<Pt2>> {
    let valid = finite(c) && w.is_finite() && h.is_finite() && rad.is_finite();
    if !valid || w <= 0.0 || h <= 0.0 {
        return Vec::new();
    }
    let (xo, yo) = (w / 2.0, h / 2.0); // the outline's half-extents
    let rad = rad.clamp(0.0, xo.min(yo));
    if rad == 0.0 {
        return vec![Subpath::new(at(c, -xo, -yo))
            .line_to(at(c, xo, -yo))
            .line_to(at(c, xo, yo))
            .line_to(at(c, -xo, yo))
            .close()];
    }
    // The straight sides' half-lengths. At the largest radius one of them
    // is exactly 0 (`xo − xo`), so those sides vanish exactly.
    let (x, y, k) = (xo - rad, yo - rad, K * rad);
    let mut sub = Subpath::new(at(c, -xo, -y)).cubic_to(
        at(c, -xo, -y - k),
        at(c, -x - k, -yo),
        at(c, -x, -yo),
    );
    if x > 0.0 {
        sub = sub.line_to(at(c, x, -yo));
    }
    sub = sub.cubic_to(at(c, x + k, -yo), at(c, xo, -y - k), at(c, xo, -y));
    if y > 0.0 {
        sub = sub.line_to(at(c, xo, y));
    }
    sub = sub.cubic_to(at(c, xo, y + k), at(c, x + k, yo), at(c, x, yo));
    if x > 0.0 {
        sub = sub.line_to(at(c, -x, yo));
    }
    sub = sub.cubic_to(at(c, -x - k, yo), at(c, -xo, y + k), at(c, -xo, y));
    vec![sub.close()]
}

/// A closed polygon through `pts` in the caller's order, with the closing
/// side implicit.
///
/// Consecutive duplicate vertices — the last repeating the first included
/// — are dropped rather than emitted as zero-length sides; fewer than
/// three vertices left, or any non-finite one, gives an empty `Vec`.
pub fn polygon(pts: &[Pt2]) -> Vec<Subpath<Pt2>> {
    if !pts.iter().all(|&p| finite(p)) {
        return Vec::new();
    }
    let mut corners = pts.to_vec();
    corners.dedup();
    if corners.len() > 1 && corners.first() == corners.last() {
        corners.pop();
    }
    if corners.len() < 3 {
        return Vec::new();
    }
    let mut sub = Subpath::new(corners[0]);
    for &p in &corners[1..] {
        sub = sub.line_to(p);
    }
    vec![sub.close()]
}

/// `c` offset by `(dx, dy)`.
fn at(c: Pt2, dx: f64, dy: f64) -> Pt2 {
    Pt2 {
        x: c.x + dx,
        y: c.y + dy,
    }
}

/// Both coordinates finite.
fn finite(p: Pt2) -> bool {
    p.x.is_finite() && p.y.is_finite()
}
