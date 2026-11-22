//! Plain 3-vectors: the arithmetic the mesh builders and the pipeline share.
//!
//! Poses are motors (garust); these are only the points and directions a
//! motor's matrix is applied to. Plain `+ − ×`, no `mul_add`, so the same
//! input gives the same bits on every run.

/// A point or direction in 3D, `[x, y, z]`, y up.
pub type V3 = [f64; 3];

/// `a + b`.
#[must_use]
pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// `a − b`.
#[must_use]
pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// `a · s`.
#[must_use]
pub fn scale(a: V3, s: f64) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

/// The dot product.
#[must_use]
pub fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The cross product `a × b` (right-handed).
#[must_use]
pub fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// The Euclidean length.
#[must_use]
pub fn length(a: V3) -> f64 {
    dot(a, a).sqrt()
}

/// `a` scaled to unit length; `None` for a zero or non-finite vector.
#[must_use]
pub fn normalize(a: V3) -> Option<V3> {
    let n = length(a);
    (n.is_finite() && n > 0.0).then(|| scale(a, 1.0 / n))
}

/// The point `a + (b − a)·t`.
#[must_use]
pub fn lerp(a: V3, b: V3, t: f64) -> V3 {
    add(a, scale(sub(b, a), t))
}

/// Applies a column-major homogeneous matrix (garust `Motor::to_matrix`)
/// to a point.
#[must_use]
pub fn transform(m: &[[f64; 4]; 4], p: V3) -> V3 {
    let mut out = [0.0; 3];
    for (row, o) in out.iter_mut().enumerate() {
        *o = m[0][row] * p[0] + m[1][row] * p[1] + m[2][row] * p[2] + m[3][row];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cross_is_right_handed() {
        assert_eq!(cross([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]), [0.0, 0.0, 1.0]);
    }

    #[test]
    fn normalize_rejects_zero() {
        assert_eq!(normalize([0.0; 3]), None);
        let n = normalize([0.0, 3.0, 4.0]).unwrap();
        assert!((n[1] - 0.6).abs() < 1e-15 && (n[2] - 0.8).abs() < 1e-15);
    }
}
