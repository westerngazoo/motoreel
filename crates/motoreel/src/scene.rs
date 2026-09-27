//! Scene composition and the evaluation loop.

use garust::{pga, Motor3};

use crate::camera::{Camera, Projection};
use crate::label::Label;
use crate::object::{Object, Shape};
use crate::path::{bernstein, split_cubic, Seg, Subpath};
use crate::prim::{Prim2, Pt2, Style};

/// Pinhole path tolerance, as a fraction of the view's shorter side
/// (SPEC-0012 §2.4): 0.054 px at 1080 × 1920, and under 0.1 px on any
/// frame whose short side is at most 2000 px. It scales with resolution.
const PINHOLE_REL_TOL: f64 = 5e-5;

/// Pinhole subdivision depth limit: at most 2¹² = 4096 pieces per authored
/// cubic. A piece still over tolerance at the limit is emitted anyway
/// (SPEC-0012 §2.4).
const PINHOLE_MAX_DEPTH: u32 = 12;

/// A view-space point in Euclidean form: what pinhole subdivision splits.
type Xyz = (f64, f64, f64);

/// Opaque handle to an object in a scene (stable insertion index) — the
/// seam R-0007's derived shapes will reference; inert until then.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjectId(usize);

impl ObjectId {
    /// The insertion index this id stands for. Ids are not scene-scoped, so
    /// a consumer must treat an out-of-range index as a cull, never a panic.
    pub(crate) fn index(self) -> usize {
        self.0
    }
}

/// A renderable description: objects in draw order, one camera, and a
/// duration in seconds (consumed by SPEC-0003's frame walk, not by
/// [`Scene::eval`]).
#[derive(Clone, Debug)]
pub struct Scene {
    /// Draw order = insertion order; later objects paint over earlier.
    pub objects: Vec<Object>,
    /// The eye (SPEC-0002 §2.4).
    pub camera: Camera,
    /// Scene length in seconds. Contract: finite and ≥ 0.
    pub duration: f64,
    /// Labels, painted after every object, in insertion order (R-0007).
    pub labels: Vec<Label>,
    /// The centred image-space view window screen anchors are derived from.
    /// Contract: both finite and > 0. Must match the sink's own window —
    /// [`Scene::render`] rejects a disagreement before any frame is written.
    pub view: (f64, f64),
}

impl Scene {
    /// An empty scene of the given duration with the default camera.
    pub fn new(duration: f64) -> Self {
        Scene {
            objects: Vec::new(),
            camera: Camera::default(),
            duration,
            labels: Vec::new(),
            view: (3.2, 1.8),
        }
    }

    /// Append an object; returns its stable id (draw order = insertion).
    pub fn add(&mut self, object: Object) -> ObjectId {
        self.objects.push(object);
        ObjectId(self.objects.len() - 1)
    }

    /// Append a label. Labels paint after every object, in insertion order.
    pub fn add_label(&mut self, label: Label) {
        self.labels.push(label);
    }

    /// Evaluate at time `t`: every track becomes a pose, geometry rides
    /// one composed motor into view space, surviving primitives are
    /// emitted in draw order. Total and deterministic; culled primitives
    /// are simply absent (SPEC-0002 §2.5).
    pub fn eval(&self, t: f64) -> Vec<Prim2> {
        let view = self.camera.pose.inverse();
        let pinhole_tol = PINHOLE_REL_TOL * self.view.0.min(self.view.1);
        let mut prims = Vec::with_capacity(self.objects.len() + self.labels.len());
        for obj in &self.objects {
            let to_view = view.compose(&obj.track.eval(t)); // pose, then view
            let projected = project_shape(
                &obj.shape,
                &to_view,
                self.camera.projection,
                pinhole_tol,
                obj.style,
            );
            if let Some(p) = projected {
                prims.push(p);
            }
        }

        // Phase 2 — labels, after every object (R-0007 §2.1). Phase 1 above
        // is byte-for-byte what it was; nothing in it was reordered.
        for label in &self.labels {
            let Some(anchor) =
                label
                    .anchor
                    .resolve(&self.objects, &view, self.camera.projection, self.view, t)
            else {
                continue; // culled whole — never a NaN position
            };
            let at = Pt2 {
                x: anchor.x + label.offset.x,
                y: anchor.y + label.offset.y,
            };
            // `offset` is author data; this guard keeps SPEC-0002 §2.2's
            // finite-coordinate invariant unconditional.
            if !(at.x.is_finite() && at.y.is_finite()) {
                continue;
            }
            prims.push(Prim2::Text {
                at,
                // Verbatim. The substitution that used to happen here is
                // what R-0009 removed: it ran upstream of both sinks, so
                // even the one that could write Spanish never got to.
                text: label.text.clone(),
                size: label.size,
                align: label.align,
                face: label.face,
                style: label.style,
            });
        }
        prims
    }
}

/// One shape through the camera; `None` culls the whole primitive
/// (SPEC-0002 §2.5: every vertex must pass, or nothing is emitted).
fn project_shape(
    shape: &Shape,
    to_view: &Motor3,
    projection: Projection,
    pinhole_tol: f64,
    style: Style,
) -> Option<Prim2> {
    let pt = |p: &pga::Point| projection.project(&p.transform(to_view));
    Some(match shape {
        Shape::Point(p) => Prim2::Point { at: pt(p)?, style },
        Shape::Segment(a, b) => Prim2::Segment {
            a: pt(a)?,
            b: pt(b)?,
            style,
        },
        Shape::Polyline(ps) => {
            let points = ps.iter().map(pt).collect::<Option<Vec<Pt2>>>()?;
            Prim2::Polyline { points, style }
        }
        Shape::Edges(es) => {
            let segments = es
                .iter()
                .map(|(a, b)| Some((pt(a)?, pt(b)?)))
                .collect::<Option<Vec<(Pt2, Pt2)>>>()?;
            Prim2::Edges { segments, style }
        }
        Shape::Path(subpaths) => Prim2::Path {
            subpaths: project_path(subpaths, to_view, projection, pinhole_tol)?,
            style,
        },
    })
}

/// A model-space path in image space (SPEC-0012 §2.3–2.4), or `None` to
/// cull it whole.
///
/// Every control point must project, handles and empty subpaths' starts
/// included: a curve lies in the convex hull of its control points, so if
/// they are all in front of the camera, so is the curve. Empty subpaths are
/// then dropped, and a path with nothing left is not emitted.
fn project_path(
    subpaths: &[Subpath<pga::Point>],
    to_view: &Motor3,
    projection: Projection,
    pinhole_tol: f64,
) -> Option<Vec<Subpath<Pt2>>> {
    let mut out = Vec::with_capacity(subpaths.len());
    for sub in subpaths {
        let view = sub.map(|p| p.transform(to_view));
        let image = project_subpath(&view, projection, pinhole_tol)?;
        if !image.segs.is_empty() {
            out.push(image);
        }
    }
    (!out.is_empty()).then_some(out)
}

/// One view-space subpath in image space, or `None` to cull.
///
/// Lines project their end points under either camera: a projective map
/// sends lines to lines. Under the orthographic camera, which is affine, a
/// cubic is exactly the cubic of its projected control points. Under the
/// pinhole camera it is subdivided until each piece is within tolerance.
fn project_subpath(
    view: &Subpath<pga::Point>,
    projection: Projection,
    pinhole_tol: f64,
) -> Option<Subpath<Pt2>> {
    let pt = |p: &pga::Point| projection.project(p);
    let mut out = Subpath::new(pt(&view.start)?);
    let mut from = view.start;
    for seg in &view.segs {
        from = match (*seg, projection) {
            (Seg::Line(end), _) => {
                out.segs.push(Seg::Line(pt(&end)?));
                end
            }
            (Seg::Cubic(h1, h2, end), Projection::Orthographic) => {
                out.segs.push(Seg::Cubic(pt(&h1)?, pt(&h2)?, pt(&end)?));
                end
            }
            (Seg::Cubic(h1, h2, end), Projection::Pinhole { .. }) => {
                let q = [from, h1, h2, end].map(|p| p.to_euclidean());
                subdivide(q, projection, pinhole_tol, 0, &mut out.segs)?;
                end
            }
        };
    }
    out.closed = view.closed;
    Some(out)
}

/// Emit the pinhole image of the view-space cubic `q` as image-space
/// cubic pieces (SPEC-0012 §2.4).
///
/// A piece is emitted when its projected control polygon stays within
/// `tol` of the true perspective image at `t = ¼, ½, ¾`, or at depth
/// [`PINHOLE_MAX_DEPTH`]; otherwise it is halved by de Casteljau and both
/// halves recurse, left first. Only `+ − × ÷` and a correctly rounded
/// `sqrt` enter, so the pieces are a pure function of the view-space
/// control points. `None` if any point fails to project, which culls the
/// whole primitive.
fn subdivide(
    q: [Xyz; 4],
    projection: Projection,
    tol: f64,
    depth: u32,
    out: &mut Vec<Seg<Pt2>>,
) -> Option<()> {
    let [a, b, c, d] = q.map(|v| projection.project_euclidean(v));
    let image = [a?, b?, c?, d?];
    if depth == PINHOLE_MAX_DEPTH || probe_error(q, image, projection)? <= tol {
        out.push(Seg::Cubic(image[1], image[2], image[3]));
        return Some(());
    }
    let (left, right) = split_cubic(q, |u, v| {
        ((u.0 + v.0) / 2.0, (u.1 + v.1) / 2.0, (u.2 + v.2) / 2.0)
    });
    subdivide(left, projection, tol, depth + 1, out)?;
    subdivide(right, projection, tol, depth + 1, out)
}

/// The largest distance, over `t = ¼, ½, ¾`, between the true image
/// `project(Q(t))` and the image-space cubic through the projected
/// control points at the same `t` (SPEC-0012 §2.4).
///
/// A probe-point estimate, not a bound between probes. It also penalises
/// the reparametrisation perspective introduces, so it over-subdivides:
/// that costs pieces, not accuracy.
fn probe_error(q: [Xyz; 4], image: [Pt2; 4], projection: Projection) -> Option<f64> {
    let mut worst = 0.0_f64;
    for t in [0.25, 0.5, 0.75] {
        let w = bernstein(t);
        let on_curve = projection.project_euclidean((
            w[0] * q[0].0 + w[1] * q[1].0 + w[2] * q[2].0 + w[3] * q[3].0,
            w[0] * q[0].1 + w[1] * q[1].1 + w[2] * q[2].1 + w[3] * q[3].1,
            w[0] * q[0].2 + w[1] * q[1].2 + w[2] * q[2].2 + w[3] * q[3].2,
        ))?;
        let (dx, dy) = (
            on_curve.x
                - (w[0] * image[0].x + w[1] * image[1].x + w[2] * image[2].x + w[3] * image[3].x),
            on_curve.y
                - (w[0] * image[0].y + w[1] * image[1].y + w[2] * image[2].y + w[3] * image[3].y),
        );
        worst = worst.max((dx * dx + dy * dy).sqrt());
    }
    Some(worst)
}

#[cfg(test)]
mod tests {
    use super::Scene;
    use crate::object::Object;
    use garust::pga;

    /// Draw order is insertion order; ids are distinct stable indices.
    #[test]
    fn add_preserves_insertion_order_with_distinct_ids() {
        let mut scene = Scene::new(1.0);
        let a = scene.add(Object::point(pga::Point::new(0.0, 0.0, 0.0)));
        let b = scene.add(Object::point(pga::Point::new(1.0, 0.0, 0.0)));
        assert_ne!(a, b);
        assert_eq!(scene.objects.len(), 2);
    }
}
