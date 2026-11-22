//! Scene composition and the evaluation loop.

use garust::{pga, Motor3};

use crate::camera::{Camera, Projection};
use crate::finish::{Finish, FrameCtx, Light, Surface};
use crate::label::Label;
use crate::object::{Object, Shape};
use crate::path::{bernstein, split_cubic, Seg, Subpath};
use crate::prim::{Fill, Paint, Prim2, Pt2, Style};
use crate::solid::{self, Posed};

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

/// How [`Scene::eval_with`] paints mesh objects: one finish for every
/// depth group, a light in view space, and the seed a finish may vary by
/// (R-0017 AC4).
///
/// Chosen per render rather than stored on an object or the scene: both
/// are plain-data structs that callers build field by field, so a new
/// field would break them (SPEC-0018 §2.5).
#[derive(Clone, Copy)]
pub struct Look<'a> {
    /// The finish.
    pub finish: &'a dyn Finish,
    /// The light, in view space ([`Camera::view_direction`] turns a world
    /// direction into one).
    pub light: Light,
    /// Handed to the finish in its [`FrameCtx`].
    pub seed: u64,
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
    ///
    /// Mesh objects (R-0018) need a finish and a light, which this call
    /// does not have: they emit nothing here. Paint them with
    /// [`Scene::eval_with`].
    pub fn eval(&self, t: f64) -> Vec<Prim2> {
        self.eval_looked(t, None)
    }

    /// [`Scene::eval`], with mesh objects painted through `look`
    /// (R-0018 AC5, SPEC-0018 §2.5).
    ///
    /// A maximal run of consecutive mesh objects is one **depth group**: its
    /// faces are ordered together ([`Scene::surfaces`]) and painted as one
    /// batch, at the draw-order slot of the run's first object. Every other
    /// object keeps its insertion-order slot, exactly as in `eval`.
    pub fn eval_with(&self, t: f64, look: &Look) -> Vec<Prim2> {
        self.eval_looked(t, Some(look))
    }

    /// The surfaces of each depth group at `t`, far to near, groups in draw
    /// order (SPEC-0018 §2.4). Ids are `(object index << 32) | face index`.
    pub fn surfaces(&self, t: f64) -> Vec<Vec<Surface>> {
        let view = self.camera.pose.inverse();
        let mut out = Vec::new();
        let mut i = 0;
        while i < self.objects.len() {
            let end = self.mesh_run_end(i);
            if end > i {
                out.push(self.group_surfaces(i, end, &view, t));
                i = end;
            } else {
                i += 1;
            }
        }
        out
    }

    /// The end of the run of consecutive mesh objects starting at `i`
    /// (`i` itself when object `i` is not a mesh).
    fn mesh_run_end(&self, i: usize) -> usize {
        i + self.objects[i..]
            .iter()
            .take_while(|o| matches!(o.shape, Shape::Mesh(_)))
            .count()
    }

    /// The ordered surfaces of mesh objects `start..end`.
    fn group_surfaces(&self, start: usize, end: usize, view: &Motor3, t: f64) -> Vec<Surface> {
        let posed: Vec<Posed> = (start..end)
            .filter_map(|index| {
                // `if let`, not a `match` with a catch-all: R-0012 AC1 keeps
                // every `Shape` match in this file exhaustive by name.
                let obj = &self.objects[index];
                if let Shape::Mesh(mesh) = &obj.shape {
                    Some(Posed {
                        index,
                        mesh,
                        to_view: view.compose(&obj.track.eval(t)),
                    })
                } else {
                    None
                }
            })
            .collect();
        solid::surfaces(&posed, self.camera.projection)
    }

    /// The one evaluation walk; `look` is `None` for [`Scene::eval`].
    fn eval_looked(&self, t: f64, look: Option<&Look>) -> Vec<Prim2> {
        let view = self.camera.pose.inverse();
        let pinhole_tol = PINHOLE_REL_TOL * self.view.0.min(self.view.1);
        let mut prims = Vec::with_capacity(self.objects.len() + self.labels.len());
        let mut i = 0;
        while i < self.objects.len() {
            let end = self.mesh_run_end(i);
            if end > i {
                if let Some(look) = look {
                    let batch = self.group_surfaces(i, end, &view, t);
                    let ctx = FrameCtx { t, seed: look.seed };
                    prims.extend(look.finish.paint(&batch, &look.light, &ctx));
                }
                i = end;
                continue;
            }
            let obj = &self.objects[i];
            i += 1;
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
            style: pose_fill(style, |q| pt(&pga::Point::new(q.x, q.y, 0.0)))?,
        },
        // Painted through a finish by `eval_with`, never one at a time.
        Shape::Mesh(_) => return None,
    })
}

/// A path's style with its gradient, if it has one, carried into image
/// space by `map`: the model plane `z = 0` through the pose and the camera
/// (SPEC-0016 §2.6). `None` culls the primitive whole when a frame point
/// fails to project, the rule control points follow.
///
/// Everything else is verbatim, a flat fill included.
fn pose_fill(style: Style, map: impl Fn(Pt2) -> Option<Pt2>) -> Option<Style> {
    let Some(Fill {
        paint: Paint::Gradient(gradient),
        alpha,
    }) = style.fill
    else {
        return Some(style);
    };
    Some(Style {
        fill: Some(Fill::gradient(gradient.map_frame(map)?, alpha)),
        ..style
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
